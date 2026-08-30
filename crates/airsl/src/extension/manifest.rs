//! The `extension.toml` manifest: what an extension asks for.
//!
//! Its own module because the manifest is the only artefact in this crate that a third party
//! writes, so its grammar is a public contract and its error messages are documentation. The
//! shape deserialised from TOML is private and deliberately strict — an unknown key is an
//! error, because a misspelled `fs.raed` that was silently ignored would be a capability the
//! author believed they had requested.
//!
//! Parsing is two stages: `toml` into the raw shape, then [`Manifest::validate`] into the typed
//! form with newtypes, expanded absolute paths and a checked api version. Both stages are pure.
//! [`RawManifest`] is public only so [`Manifest::parse`] and [`Manifest::validate`] can be called
//! separately; it has no accessors, and `toml`/`serde` are an internal detail this module does
//! not re-export. A tool that wants to tell a parse failure from a validation failure does so by
//! which [`Error`] variant it gets back, never by inspecting the raw value.
//!
//! `[capabilities]` reserves the keys `fs`, `proc`, `env` and `optional` for its own tables, so
//! `env = true` is a shape mismatch caught at parse time, not a module presence assertion.
//!
//! Responsibilities: [`Manifest`], [`CapabilityRequest`], [`LimitRequest`], and
//! [`Manifest::from_dir`].
//!
//! Non-responsibilities: whether the request is acceptable. That is negotiation, against a
//! ceiling this module never sees.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::extension::api_version::ApiVersion;
use crate::extension::memory_size::parse_memory_size;
use crate::extension::variables::Variables;
use crate::paths::containment::is_within;
use crate::paths::rules::native::to_script_string;
use crate::sandbox::{InstructionLimit, MemoryLimit};
use crate::types::{ExtensionName, ModuleName};

/// File name a manifest must have inside its extension directory.
pub const MANIFEST_FILE: &str = "extension.toml";

/// A validated manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    name: ExtensionName,
    version: String,
    entry: PathBuf,
    api: ApiVersion,
    required: CapabilityRequest,
    optional: CapabilityRequest,
    limits: LimitRequest,
}

/// One block of capability requests — `[capabilities]` or `[capabilities.optional]`.
///
/// Paths are absolute and already expanded. Module names are presence assertions
/// (`regex = true`) and carry no parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilityRequest {
    fs_read: Vec<PathBuf>,
    fs_write: Vec<PathBuf>,
    proc_run: BTreeSet<String>,
    env_read: BTreeSet<String>,
    modules: BTreeSet<ModuleName>,
}

impl CapabilityRequest {
    /// Requested read roots, in manifest order.
    #[must_use]
    pub fn fs_read(&self) -> &[PathBuf] {
        &self.fs_read
    }

    /// Requested write roots, in manifest order.
    #[must_use]
    pub fn fs_write(&self) -> &[PathBuf] {
        &self.fs_write
    }

    /// Requested executables, sorted.
    pub fn proc_run(&self) -> impl Iterator<Item = &str> {
        self.proc_run.iter().map(String::as_str)
    }

    /// Requested environment variable names, sorted.
    pub fn env_read(&self) -> impl Iterator<Item = &str> {
        self.env_read.iter().map(String::as_str)
    }

    /// Modules whose presence the extension asserts, sorted.
    pub fn modules(&self) -> impl Iterator<Item = &ModuleName> {
        self.modules.iter()
    }

    /// Whether nothing at all is requested.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fs_read.is_empty()
            && self.fs_write.is_empty()
            && self.proc_run.is_empty()
            && self.env_read.is_empty()
            && self.modules.is_empty()
    }
}

/// The `[limits]` block. An absent limit means "whatever the ceiling says".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LimitRequest {
    memory: Option<MemoryLimit>,
    instructions: Option<InstructionLimit>,
}

impl LimitRequest {
    /// Requested memory ceiling.
    #[must_use]
    pub const fn memory(&self) -> Option<MemoryLimit> {
        self.memory
    }

    /// Requested instruction ceiling.
    #[must_use]
    pub const fn instructions(&self) -> Option<InstructionLimit> {
        self.instructions
    }
}

impl Manifest {
    /// The extension's name.
    #[must_use]
    pub const fn name(&self) -> &ExtensionName {
        &self.name
    }

    /// The version string, uninterpreted.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The entry script, relative to the extension directory.
    #[must_use]
    pub fn entry(&self) -> &Path {
        &self.entry
    }

    /// The api version the manifest pins.
    #[must_use]
    pub const fn api(&self) -> ApiVersion {
        self.api
    }

    /// Capabilities the extension cannot load without.
    #[must_use]
    pub const fn required(&self) -> &CapabilityRequest {
        &self.required
    }

    /// Capabilities the extension can live without.
    #[must_use]
    pub const fn optional(&self) -> &CapabilityRequest {
        &self.optional
    }

    /// Requested ceilings.
    #[must_use]
    pub const fn limits(&self) -> &LimitRequest {
        &self.limits
    }

    /// Parses manifest text without validating it; see [`Manifest::validate`].
    ///
    /// Public so a tool can report parse errors separately from validation errors.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManifestParse`] with the parser's message when `text` is not valid TOML
    /// or contains an unknown key; `path` is only used in the message.
    pub fn parse(text: &str, path: &Path) -> Result<RawManifest> {
        toml::from_str(text).map_err(|error| Error::ManifestParse {
            path: to_script_string(path),
            reason: error.to_string(),
        })
    }

    /// Reads and validates `<dir>/extension.toml`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManifestRead`] when the file cannot be read, [`Error::ManifestParse`] when
    /// it is not the expected shape, and whatever [`Manifest::validate`] returns.
    pub fn from_dir(dir: impl AsRef<Path>, variables: &Variables) -> Result<Self> {
        let dir = dir.as_ref();
        let path = dir.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&path).map_err(|source| Error::ManifestRead {
            path: to_script_string(&path),
            source,
        })?;
        let raw = Self::parse(&text, &path)?;
        Self::validate(raw, dir, variables)
    }

    /// Turns a parsed manifest into a validated one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] for a malformed extension name,
    /// [`Error::UnsupportedApi`] for an api this runtime does not implement,
    /// [`Error::ManifestVariable`] for an unknown `$VAR`, and [`Error::ManifestInvalid`] for an
    /// entry that leaves `dir` or does not exist, a non-absolute path after expansion, a module
    /// key that is malformed or whose value is not `true`, an unparseable memory size, or a zero
    /// instruction limit.
    pub fn validate(raw: RawManifest, dir: &Path, variables: &Variables) -> Result<Self> {
        let name = ExtensionName::new(raw.extension.name)?;
        let api = ApiVersion::supported(raw.extension.api)?;
        let entry = validate_entry(&raw.extension.entry, dir)?;

        let required = CapabilityRequest::from_raw(
            raw.capabilities.fs,
            raw.capabilities.proc,
            raw.capabilities.env,
            raw.capabilities.modules,
            variables,
        )?;
        let optional = match raw.capabilities.optional {
            Some(block) => CapabilityRequest::from_raw(
                block.fs,
                block.proc,
                block.env,
                block.modules,
                variables,
            )?
            .without(&required),
            None => CapabilityRequest::default(),
        };

        let limits = LimitRequest {
            memory: raw
                .limits
                .memory
                .as_deref()
                .map(parse_memory_size)
                .transpose()?,
            instructions: raw
                .limits
                .instructions
                .map(validate_instruction_count)
                .transpose()?,
        };

        Ok(Self {
            name,
            version: raw.extension.version,
            entry,
            api,
            required,
            optional,
            limits,
        })
    }
}

/// Checks that `entry` is relative, climbs nowhere, and names an existing file under `dir`.
fn validate_entry(entry: &str, dir: &Path) -> Result<PathBuf> {
    let invalid = |reason: String| Error::ManifestInvalid {
        field: "extension.entry",
        reason,
    };
    let relative = PathBuf::from(entry);
    if relative.is_absolute() {
        return Err(invalid(format!(
            "`{entry}` must be relative to the extension directory"
        )));
    }
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid(format!(
            "`{entry}` must not contain `..` or `.` components"
        )));
    }
    let root = dir
        .canonicalize()
        .map_err(|e| invalid(format!("cannot resolve the extension directory: {e}")))?;
    let resolved = root
        .join(&relative)
        .canonicalize()
        .map_err(|e| invalid(format!("`{entry}` cannot be resolved: {e}")))?;
    if !is_within(&resolved, &root) {
        return Err(invalid(format!(
            "`{entry}` resolves outside the extension directory"
        )));
    }
    if !resolved.is_file() {
        return Err(invalid(format!("`{entry}` is not a file")));
    }
    // `relative`, not `resolved`, is what this function returns: it was never canonicalised, only
    // checked, so unlike `require_loader::resolve` and `loaded::recheck_entry` there is nothing
    // verbatim in it to strip.
    Ok(relative)
}

impl CapabilityRequest {
    fn from_raw(
        fs: RawFs,
        proc: RawProc,
        env: RawEnv,
        modules: BTreeMap<String, toml::Value>,
        variables: &Variables,
    ) -> Result<Self> {
        let fs_read = expand_paths(fs.read, "capabilities.fs.read", variables)?;
        let fs_write = expand_paths(fs.write, "capabilities.fs.write", variables)?;

        let mut asserted = BTreeSet::new();
        for (key, value) in modules {
            if !matches!(value, toml::Value::Boolean(true)) {
                return Err(Error::ManifestInvalid {
                    field: "capabilities",
                    reason: format!(
                        "`{key}` must be `true` (a module presence assertion) or one of the fs/proc/env tables"
                    ),
                });
            }
            let name = ModuleName::new(key.clone()).map_err(|source| Error::ManifestInvalid {
                field: "capabilities",
                reason: format!("`{key}` is not a valid module name: {source}"),
            })?;
            asserted.insert(name);
        }

        Ok(Self {
            fs_read,
            fs_write,
            proc_run: proc.run.into_iter().collect(),
            env_read: env.read.into_iter().collect(),
            modules: asserted,
        })
    }

    /// Drops every entry that `required` already carries.
    fn without(mut self, required: &Self) -> Self {
        self.fs_read.retain(|p| !required.fs_read.contains(p));
        self.fs_write.retain(|p| !required.fs_write.contains(p));
        self.proc_run = &self.proc_run - &required.proc_run;
        self.env_read = &self.env_read - &required.env_read;
        self.modules = &self.modules - &required.modules;
        self
    }
}

/// Expands and checks one list of manifest paths.
fn expand_paths(
    raw: Vec<String>,
    field: &'static str,
    variables: &Variables,
) -> Result<Vec<PathBuf>> {
    let mut out = Vec::with_capacity(raw.len());
    for text in raw {
        let expanded = variables.expand(&text)?;
        let path = PathBuf::from(&expanded);
        if !path.is_absolute() {
            return Err(Error::ManifestInvalid {
                field,
                reason: format!("`{expanded}` is not absolute after expansion"),
            });
        }
        // Same rule `validate_entry` applies to `extension.entry`: a `..` component lets a path
        // spelled inside the ceiling resolve lexically outside it, before negotiation ever
        // canonicalises anything. `Path::components()` only ever yields `CurDir` for a leading
        // `.`, which an absolute path can never have, so `..` is the one case worth checking here.
        if path.components().any(|c| c == Component::ParentDir) {
            return Err(Error::ManifestInvalid {
                field,
                reason: format!("`{expanded}` must not contain `..` components"),
            });
        }
        out.push(path);
    }
    Ok(out)
}

/// Rejects `instructions = 0`; anything else becomes a ceiling.
///
/// [`InstructionLimit::count`] silently clamps `0` to `1` so a caller building a ceiling
/// programmatically can never construct an unenforceable limit, but a manifest author who wrote
/// `0` almost certainly meant "no limit", not "one instruction" — the same author mistake
/// [`parse_memory_size`] already refuses for `memory = "0"`.
fn validate_instruction_count(raw: u64) -> Result<InstructionLimit> {
    if raw == 0 {
        return Err(Error::ManifestInvalid {
            field: "limits.instructions",
            reason: format!("`{raw}` is zero"),
        });
    }
    Ok(InstructionLimit::count(raw))
}

/// A manifest as deserialised, before validation.
///
/// Every field is the TOML type, not the domain type; unknown keys at any level are rejected
/// by serde. Opaque on purpose: it has no accessors, so nothing outside this module can inspect
/// or act on unvalidated data. Validate with [`Manifest::validate`].
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RawManifest {
    extension: RawExtension,
    #[serde(default)]
    capabilities: RawCapabilities,
    #[serde(default)]
    limits: RawLimits,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawExtension {
    name: String,
    version: String,
    entry: String,
    api: u32,
}

/// `[capabilities]`: the three parameterised tables, the optional block, and any number of
/// `<module> = true` keys collected by `flatten`.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
struct RawCapabilities {
    #[serde(default)]
    fs: RawFs,
    #[serde(default)]
    proc: RawProc,
    #[serde(default)]
    env: RawEnv,
    #[serde(default)]
    optional: Option<RawOptionalCapabilities>,
    #[serde(flatten)]
    modules: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
struct RawOptionalCapabilities {
    #[serde(default)]
    fs: RawFs,
    #[serde(default)]
    proc: RawProc,
    #[serde(default)]
    env: RawEnv,
    #[serde(flatten)]
    modules: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawFs {
    #[serde(default)]
    read: Vec<String>,
    #[serde(default)]
    write: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawProc {
    #[serde(default)]
    run: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawEnv {
    #[serde(default)]
    read: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct RawLimits {
    memory: Option<String>,
    instructions: Option<u64>,
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use std::path::Path;

    use super::{CapabilityRequest, MANIFEST_FILE, Manifest};
    use crate::extension::variables::Variables;
    use crate::types::ModuleName;

    fn vars() -> Variables {
        Variables::none().with("APP_HOME", "/home/x/app")
    }

    fn write_manifest(dir: &Path, text: &str) {
        std::fs::write(dir.join(MANIFEST_FILE), text).unwrap();
        std::fs::write(dir.join("main.lua"), "return 1").unwrap();
    }

    const FULL: &str = r#"
[extension]
name    = "journal-indexer"
version = "0.2.0"
entry   = "main.lua"
api     = 1

[capabilities]
fs.read   = ["$APP_HOME/journal"]
fs.write  = ["$APP_HOME/journal/.index"]
proc.run  = ["git"]
env.read  = ["APP_HOME", "HOME"]
regex     = true

[capabilities.optional]
proc.run  = ["tar"]

[limits]
memory       = "64MB"
instructions = 50_000_000
"#;

    fn here() -> &'static Path {
        Path::new("/x/extension.toml")
    }

    #[test]
    fn the_full_example_parses() {
        let raw = Manifest::parse(FULL, here()).unwrap();
        assert_eq!(raw.extension.name, "journal-indexer");
        assert_eq!(raw.capabilities.fs.read, ["$APP_HOME/journal"]);
        assert_eq!(
            raw.capabilities.optional.as_ref().unwrap().proc.run,
            ["tar"]
        );
        assert_eq!(raw.limits.memory.as_deref(), Some("64MB"));
        assert_eq!(raw.limits.instructions, Some(50_000_000));
        assert!(raw.capabilities.modules.contains_key("regex"));
    }

    #[test]
    fn a_manifest_with_only_the_extension_table_parses() {
        let raw = Manifest::parse(
            "[extension]\nname='a'\nversion='1'\nentry='main.lua'\napi=1\n",
            here(),
        )
        .unwrap();
        assert!(raw.capabilities.fs.read.is_empty());
        assert!(raw.limits.memory.is_none());
    }

    #[test]
    fn an_unknown_key_is_a_parse_error_naming_the_path() {
        for text in [
            "[extension]\nname='a'\nversion='1'\nentry='m.lua'\napi=1\ncolour='red'\n",
            "[extension]\nname='a'\nversion='1'\nentry='m.lua'\napi=1\n[capabilities]\nfs.raed=['/']\n",
            "[extension]\nname='a'\nversion='1'\nentry='m.lua'\napi=1\n[limitz]\nmemory='1MB'\n",
        ] {
            let err = Manifest::parse(text, here()).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains("/x/extension.toml"), "{msg}");
        }
    }

    #[test]
    fn a_missing_required_field_is_a_parse_error() {
        let err = Manifest::parse("[extension]\nname='a'\n", here()).unwrap_err();
        assert!(err.to_string().contains("missing field"), "{err}");
    }

    #[test]
    fn invalid_toml_is_a_parse_error() {
        assert!(Manifest::parse("this is = = not toml", here()).is_err());
    }

    #[test]
    fn the_full_example_validates_with_expanded_absolute_paths() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), FULL);
        let m = Manifest::from_dir(dir.path(), &vars()).unwrap();

        assert_eq!(m.name().as_str(), "journal-indexer");
        assert_eq!(m.version(), "0.2.0");
        assert_eq!(m.entry(), Path::new("main.lua"));
        assert_eq!(m.api().get(), 1);
        assert_eq!(m.required().fs_read(), [Path::new("/home/x/app/journal")]);
        assert_eq!(
            m.required().fs_write(),
            [Path::new("/home/x/app/journal/.index")]
        );
        assert_eq!(m.required().proc_run().collect::<Vec<_>>(), ["git"]);
        assert_eq!(
            m.required().env_read().collect::<Vec<_>>(),
            ["APP_HOME", "HOME"]
        );
        assert_eq!(
            m.required().modules().collect::<Vec<_>>(),
            [&ModuleName::new("regex").unwrap()]
        );
        assert_eq!(m.optional().proc_run().collect::<Vec<_>>(), ["tar"]);
        assert_eq!(m.limits().memory().unwrap().get(), 64 * 1024 * 1024);
        assert_eq!(m.limits().instructions().unwrap().get(), 50_000_000);
    }

    #[test]
    fn a_missing_manifest_is_a_read_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = Manifest::from_dir(dir.path(), &vars()).unwrap_err();
        assert!(err.to_string().starts_with("cannot read manifest"), "{err}");
    }

    #[test]
    fn an_unsupported_api_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            "[extension]\nname='a'\nversion='1'\nentry='main.lua'\napi=2\n",
        );
        let err = Manifest::from_dir(dir.path(), &vars()).unwrap_err();
        assert!(err.to_string().contains("api 2"), "{err}");
    }

    #[test]
    fn an_unknown_variable_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            "[extension]\nname='a'\nversion='1'\nentry='main.lua'\napi=1\n[capabilities]\nfs.read=['$NOPE/x']\n",
        );
        let err = Manifest::from_dir(dir.path(), &vars()).unwrap_err();
        assert!(err.to_string().contains("`$NOPE`"), "{err}");
    }

    #[test]
    fn a_relative_path_after_expansion_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            "[extension]\nname='a'\nversion='1'\nentry='main.lua'\napi=1\n[capabilities]\nfs.read=['journal']\n",
        );
        let err = Manifest::from_dir(dir.path(), &vars()).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("capabilities.fs.read") && msg.contains("absolute"),
            "{msg}"
        );
    }

    #[test]
    fn fs_paths_containing_a_parent_dir_component_are_refused() {
        // Mirrors `validate_entry`'s rule: a request that resolves lexically outside its own
        // written root must not reach negotiation looking like a plain absolute path, even though
        // it is refused at runtime by the canonicalising guard regardless.
        let dir = tempfile::tempdir().unwrap();
        for (body, field) in [
            (
                "[capabilities]\nfs.read=['$APP_HOME/../secret']\n",
                "capabilities.fs.read",
            ),
            (
                "[capabilities]\nfs.write=['$APP_HOME/sub/../../escape']\n",
                "capabilities.fs.write",
            ),
        ] {
            write_manifest(
                dir.path(),
                &format!("[extension]\nname='a'\nversion='1'\nentry='main.lua'\napi=1\n{body}"),
            );
            let err = Manifest::from_dir(dir.path(), &vars()).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains(field) && msg.contains(".."), "{field}: {msg}");
        }
    }

    #[test]
    fn a_module_named_after_a_reserved_capability_key_is_a_parse_error() {
        // `env`, `fs`, `proc` and `optional` are `[capabilities]`'s own table names, so a module
        // presence assertion that collides with one (`env = true`) is a shape mismatch at parse
        // time, not the flattened catch-all `ManifestInvalid` a genuinely unknown key gets.
        let err = Manifest::parse(
            "[extension]\nname='a'\nversion='1'\nentry='main.lua'\napi=1\n[capabilities]\nenv=true\n",
            here(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("/x/extension.toml"), "{err}");
    }

    #[test]
    fn an_instructions_limit_of_zero_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            "[extension]\nname='a'\nversion='1'\nentry='main.lua'\napi=1\n[limits]\ninstructions=0\n",
        );
        let err = Manifest::from_dir(dir.path(), &vars()).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("limits.instructions") && msg.contains("zero"),
            "{msg}"
        );
    }

    #[test]
    fn an_entry_outside_the_directory_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        for entry in ["../main.lua", "/etc/passwd", "lib/../../main.lua"] {
            write_manifest(
                dir.path(),
                &format!("[extension]\nname='a'\nversion='1'\nentry='{entry}'\napi=1\n"),
            );
            let err = Manifest::from_dir(dir.path(), &vars()).unwrap_err();
            assert!(
                err.to_string().contains("extension.entry"),
                "{entry}: {err}"
            );
        }
    }

    #[test]
    fn a_missing_entry_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(MANIFEST_FILE),
            "[extension]\nname='a'\nversion='1'\nentry='nope.lua'\napi=1\n",
        )
        .unwrap();
        let err = Manifest::from_dir(dir.path(), &vars()).unwrap_err();
        assert!(err.to_string().contains("extension.entry"), "{err}");
    }

    #[test]
    fn a_module_assertion_must_be_true_and_well_named() {
        let dir = tempfile::tempdir().unwrap();
        for (body, case) in [
            ("regex = false", "value not true"),
            ("regex = 'yes'", "value not boolean"),
            ("Re-Gex = true", "malformed module name"),
        ] {
            write_manifest(
                dir.path(),
                &format!(
                    "[extension]\nname='a'\nversion='1'\nentry='main.lua'\napi=1\n[capabilities]\n{body}\n"
                ),
            );
            let err = Manifest::from_dir(dir.path(), &vars()).unwrap_err();
            assert!(err.to_string().contains("capabilities"), "{case}: {err}");
        }
    }

    #[test]
    fn a_capability_in_both_blocks_is_required_and_dropped_from_optional() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            "[extension]\nname='a'\nversion='1'\nentry='main.lua'\napi=1\n\
             [capabilities]\nproc.run=['git']\nenv.read=['A']\n\
             [capabilities.optional]\nproc.run=['git','tar']\nenv.read=['A']\n",
        );
        let m = Manifest::from_dir(dir.path(), &vars()).unwrap();
        assert_eq!(m.required().proc_run().collect::<Vec<_>>(), ["git"]);
        assert_eq!(m.optional().proc_run().collect::<Vec<_>>(), ["tar"]);
        assert!(m.optional().env_read().next().is_none());
    }

    #[test]
    fn a_bad_name_is_reported_as_an_invalid_name() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            "[extension]\nname='Bad Name'\nversion='1'\nentry='main.lua'\napi=1\n",
        );
        let err = Manifest::from_dir(dir.path(), &vars()).unwrap_err();
        assert!(err.to_string().contains("extension name"), "{err}");
    }

    #[test]
    fn an_empty_capability_request_reports_empty() {
        assert!(CapabilityRequest::default().is_empty());
    }

    #[test]
    fn an_unrecognised_top_level_capability_key_parses_and_is_refused_at_validation() {
        // Unlike a misspelled key nested under a known table (`fs.raed`, caught by that table's
        // own `deny_unknown_fields` at parse time), a key that is not `fs`/`proc`/`env`/`optional`
        // at the top level of `[capabilities]` has nowhere else to land: `RawCapabilities` has no
        // `deny_unknown_fields` of its own because the `#[serde(flatten)]` `modules` map is what
        // *is* its catch-all. So `wat = ["a"]` parses successfully into `modules["wat"]` as a TOML
        // array, and only fails once `CapabilityRequest::from_raw` checks that every flattened
        // value is `true`.
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            "[extension]\nname='a'\nversion='1'\nentry='main.lua'\napi=1\n[capabilities]\nwat=['a']\n",
        );
        let raw = Manifest::parse(
            &std::fs::read_to_string(dir.path().join(MANIFEST_FILE)).unwrap(),
            Path::new("x"),
        )
        .unwrap();
        assert!(raw.capabilities.modules.contains_key("wat"));

        let err = Manifest::from_dir(dir.path(), &vars()).unwrap_err();
        assert!(err.to_string().contains("capabilities"), "{err}");
    }

    #[test]
    fn a_capability_in_both_blocks_is_dropped_from_optional_fs_paths_too() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            "[extension]\nname='a'\nversion='1'\nentry='main.lua'\napi=1\n\
             [capabilities]\nfs.read=['$APP_HOME/journal']\n\
             [capabilities.optional]\nfs.read=['$APP_HOME/journal', '$APP_HOME/other']\n",
        );
        let m = Manifest::from_dir(dir.path(), &vars()).unwrap();
        assert_eq!(m.required().fs_read(), [Path::new("/home/x/app/journal")]);
        assert_eq!(m.optional().fs_read(), [Path::new("/home/x/app/other")]);
    }

    #[test]
    fn a_symlinked_entry_pointing_outside_the_directory_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("secret.lua");
        std::fs::write(&target, "return 1").unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join("main.lua")).unwrap();

        std::fs::write(
            dir.path().join(MANIFEST_FILE),
            "[extension]\nname='a'\nversion='1'\nentry='main.lua'\napi=1\n",
        )
        .unwrap();
        let err = Manifest::from_dir(dir.path(), &vars()).unwrap_err();
        assert!(err.to_string().contains("extension.entry"), "{err}");
    }

    #[test]
    fn a_tempdir_root_under_var_still_canonicalises_to_match_the_entry() {
        // On macOS `tempfile::tempdir()` returns a path under `/var`, itself a symlink to
        // `/private/var`. `validate_entry` canonicalises both the directory and the joined entry
        // path before comparing, so the two sides land on the same `/private/var/...` form and the
        // `starts_with` containment check still succeeds.
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), FULL);
        assert!(Manifest::from_dir(dir.path(), &vars()).is_ok());
    }
}
