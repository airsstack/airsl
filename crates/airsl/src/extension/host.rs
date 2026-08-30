//! The registry that turns a directory of extensions into a set of running engines and fans
//! events out to them.
//!
//! Its own module because the registry is policy the unit does not have: one name loads once, a
//! directory loads in a deterministic order, a broadcast visits every extension and never
//! short-circuits. Those are decisions about *many* extensions, and [`super::loaded`] is
//! deliberately about one. The builder is a type-state on the ceiling — there is no `build` until
//! `ceiling` has been given — for the same reason [`crate::Engine::builder`] is a type-state on
//! the policy: the bound is the one input a host must not be able to forget.
//!
//! Responsibilities: [`HostBuilder`], [`ExtensionHost`] — `load`, `load_dir`, `broadcast`, lookup.
//!
//! Non-responsibilities: the load sequence itself ([`super::loaded`]); the intersection
//! ([`mod@super::negotiate`]); deciding policy ([`super::approver`]).

use std::collections::BTreeSet;
use std::path::Path;

use crate::error::{Error, Result};
use crate::extension::{
    Approver, Ceiling, Dispatch, Extension, LoadContext, LoadReport, MANIFEST_FILE,
    ManifestApprover, Variables,
};
use crate::modules::ModuleSet;
use crate::paths::rules::native::to_script_string;
use crate::types::{EventName, ExtensionName, RootTable};

/// Builds a [`ModuleSet`] for each load. The default is [`crate::modules::stdlib()`].
///
/// A trait rather than a bare closure parameter so a host that needs no customisation names no
/// type at all — [`Stdlib`] is the default `F` on both [`HostBuilder`] and [`ExtensionHost`].
pub trait ModuleFactory: Send + Sync {
    /// Assembles the set a fresh load starts from.
    ///
    /// Called once per [`ExtensionHost::load`], because the set an engine ends up owning must not
    /// be shared between two extensions.
    ///
    /// # Errors
    ///
    /// Whatever assembling the set raises — [`Error::DuplicateModule`] for a doubled name.
    fn modules(&self) -> Result<ModuleSet>;
}

impl<F> ModuleFactory for F
where
    F: Fn() -> Result<ModuleSet> + Send + Sync,
{
    fn modules(&self) -> Result<ModuleSet> {
        self()
    }
}

/// The shipped default: [`crate::modules::stdlib()`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Stdlib;

impl ModuleFactory for Stdlib {
    fn modules(&self) -> Result<ModuleSet> {
        crate::modules::stdlib()
    }
}

/// Type-state marker: [`HostBuilder::ceiling`] has not been called yet.
///
/// [`HostBuilder::build`] does not exist for this state — the ceiling is the one input a host
/// must not be able to forget, so there is nothing to build until it has one.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCeiling;

/// Type-state marker: a [`Ceiling`] has been given.
///
/// Carries the ceiling by value, rather than merely proving one was supplied, because
/// [`HostBuilder::build`] needs somewhere to move it out of; [`Ceiling`] is not `Copy`.
#[derive(Debug, Clone)]
pub struct WithCeiling(Ceiling);

/// Configures and builds an [`ExtensionHost`].
///
/// Type-state only on the ceiling. `A` and `F` already default to the shipped choices
/// ([`ManifestApprover`], [`Stdlib`]), and swapping either is opt-in rather than a step that can
/// be skipped by accident — only the ceiling can be forgotten silently, which is what `C` exists
/// to prevent.
#[derive(Debug)]
pub struct HostBuilder<C, A = ManifestApprover, F = Stdlib> {
    ceiling: C,
    events: BTreeSet<EventName>,
    variables: Variables,
    approver: A,
    root_table: RootTable,
    factory: F,
}

impl HostBuilder<NoCeiling> {
    /// Starts with no ceiling, no declared events, no variables, and the default approver and
    /// module factory.
    pub(crate) fn new() -> Self {
        Self {
            ceiling: NoCeiling,
            events: BTreeSet::new(),
            variables: Variables::none(),
            approver: ManifestApprover,
            root_table: RootTable::default(),
            factory: Stdlib,
        }
    }
}

impl<C, A: Approver, F: ModuleFactory> HostBuilder<C, A, F> {
    /// Sets the host's maximum authority. Every load is negotiated against it.
    #[must_use]
    pub fn ceiling(self, ceiling: Ceiling) -> HostBuilder<WithCeiling, A, F> {
        HostBuilder {
            ceiling: WithCeiling(ceiling),
            events: self.events,
            variables: self.variables,
            approver: self.approver,
            root_table: self.root_table,
            factory: self.factory,
        }
    }

    /// Declares the events an extension loaded by this host may subscribe to with `ext.on`.
    ///
    /// Adds to any events already declared by an earlier call, rather than replacing them. Stored
    /// sorted and deduplicated, so [`ExtensionHost::events`] iterates in a stable order and the
    /// declared-events list an [`Error::UnknownEvent`] names is reproducible.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] for any name [`EventName::new`] refuses.
    pub fn events(mut self, events: impl IntoIterator<Item = impl AsRef<str>>) -> Result<Self> {
        for name in events {
            self.events.insert(EventName::new(name.as_ref())?);
        }
        Ok(self)
    }

    /// Adds or replaces host-supplied `$VAR` values a manifest's paths may expand.
    #[must_use]
    pub fn variables(
        mut self,
        vars: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        for (name, value) in vars {
            self.variables = self.variables.with(name, value);
        }
        self
    }

    /// Replaces the approver. Defaults to [`ManifestApprover`].
    #[must_use]
    pub fn approver<B: Approver>(self, approver: B) -> HostBuilder<C, B, F> {
        HostBuilder {
            ceiling: self.ceiling,
            events: self.events,
            variables: self.variables,
            approver,
            root_table: self.root_table,
            factory: self.factory,
        }
    }

    /// Replaces the global every loaded extension's host modules are installed under. Defaults to
    /// `airsstack`.
    #[must_use]
    pub fn root_table(mut self, root: RootTable) -> Self {
        self.root_table = root;
        self
    }

    /// Replaces the module factory. Defaults to [`Stdlib`].
    #[must_use]
    pub fn modules<G: ModuleFactory>(self, factory: G) -> HostBuilder<C, A, G> {
        HostBuilder {
            ceiling: self.ceiling,
            events: self.events,
            variables: self.variables,
            approver: self.approver,
            root_table: self.root_table,
            factory,
        }
    }
}

impl<A: Approver, F: ModuleFactory> HostBuilder<WithCeiling, A, F> {
    /// Builds the host.
    ///
    /// # Errors
    ///
    /// None today; the return type is [`Result`] so a later version can probe the module factory
    /// once here without a signature change.
    pub fn build(self) -> Result<ExtensionHost<A, F>> {
        Ok(ExtensionHost {
            ceiling: self.ceiling.0,
            events: self.events,
            variables: self.variables,
            approver: self.approver,
            root_table: self.root_table,
            factory: self.factory,
            extensions: Vec::new(),
        })
    }
}

/// A registry of loaded extensions, sharing one ceiling, event list and module factory.
///
/// `A` and `F` are the approver and module factory every load through this host uses; both
/// default to the shipped choices ([`ManifestApprover`], [`Stdlib`]), so a host that needs
/// neither swapped out never names either parameter.
#[derive(Debug)]
pub struct ExtensionHost<A: Approver = ManifestApprover, F: ModuleFactory = Stdlib> {
    ceiling: Ceiling,
    events: BTreeSet<EventName>,
    variables: Variables,
    approver: A,
    root_table: RootTable,
    factory: F,
    extensions: Vec<Extension>,
}

impl ExtensionHost {
    /// Starts building a host. There is no [`HostBuilder::build`] until
    /// [`HostBuilder::ceiling`] has been called.
    #[must_use]
    pub fn builder() -> HostBuilder<NoCeiling> {
        HostBuilder::new()
    }
}

impl<A: Approver, F: ModuleFactory> ExtensionHost<A, F> {
    /// Draws a fresh module set from the factory and bundles it with the host's shared inputs.
    ///
    /// Fresh on every call: the `ext` module in the set is about to be replaced with one that
    /// knows this host's events, and [`super::loaded::Approved::start`] then hands the whole set
    /// to a new engine, which takes ownership of it — two loads must never share one.
    fn context(&self) -> Result<LoadContext<'_, A>> {
        Ok(LoadContext::new(
            &self.ceiling,
            &self.events,
            &self.variables,
            &self.approver,
            &self.root_table,
            self.factory.modules()?,
        ))
    }

    /// Loads the extension at `dir` and registers it under the name its manifest declares.
    ///
    /// # Errors
    ///
    /// Everything [`Extension::approve`] and [`super::loaded::Approved::start`] raise, plus
    /// [`Error::DuplicateExtension`] when an extension with the same name is already loaded — the
    /// duplicate is refused before `start` runs, so its entry script never executes.
    pub fn load(&mut self, dir: impl AsRef<Path>) -> Result<&Extension> {
        let approved = Extension::approve(dir, self.context()?)?;
        if self.get(approved.name()).is_some() {
            return Err(Error::DuplicateExtension {
                extension: approved.name().to_string(),
            });
        }
        let extension = approved.start()?;
        self.extensions.push(extension);
        // Indexing at `len - 1` right after the push is the honest shape here: `unwrap`/`expect`
        // are denied by the workspace lints, no `Error` variant describes "the vector is empty
        // immediately after we pushed to it", and `indexing_slicing` is not among the lints this
        // crate enables. The invariant the index depends on is stated two lines above it.
        let index = self.extensions.len() - 1;
        Ok(&self.extensions[index])
    }

    /// Loads every subdirectory of `root` that contains an `extension.toml`, in directory-name
    /// order.
    ///
    /// Never short-circuits: one extension failing to load does not stop the rest. Every outcome
    /// lands in the returned [`LoadReport`] rather than in an `Err`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] (`operation: "read_dir"`) when `root` itself cannot be listed.
    /// Per-extension failures are never an `Err`; they are recorded in the report.
    pub fn load_dir(&mut self, root: impl AsRef<Path>) -> Result<LoadReport> {
        let root = root.as_ref();
        let io = |source: std::io::Error| Error::Io {
            operation: "read_dir",
            path: to_script_string(root),
            source,
        };

        let mut candidates = Vec::new();
        for entry in std::fs::read_dir(root).map_err(io)? {
            let path = entry.map_err(io)?.path();
            if path.is_dir() && path.join(MANIFEST_FILE).is_file() {
                candidates.push(path);
            }
        }
        candidates.sort();

        let mut report = LoadReport::default();
        for dir in candidates {
            match self.load(&dir) {
                Ok(extension) => report.record_loaded(extension.name().clone()),
                Err(error) => report.record_failed(dir, error),
            }
        }
        Ok(report)
    }

    /// Calls every loaded extension's handler for `event`, in load order.
    ///
    /// Never short-circuits: one extension's handler raising does not stop the rest from being
    /// called. Each extension's outcome, including the error if any, lands in its own
    /// [`Dispatch`]. Serialised per engine by the lock [`crate::Engine::dispatch`] already holds
    /// for the duration of an evaluation — no lock is added here.
    #[must_use]
    pub fn broadcast(&self, event: &EventName, payload: &serde_json::Value) -> Vec<Dispatch> {
        self.extensions
            .iter()
            .map(|extension| {
                Dispatch::new(extension.name().clone(), extension.call(event, payload))
            })
            .collect()
    }

    /// The loaded extension named `name`, if any.
    #[must_use]
    pub fn get(&self, name: &ExtensionName) -> Option<&Extension> {
        self.extensions
            .iter()
            .find(|extension| extension.name() == name)
    }

    /// Every loaded extension, in load order.
    pub fn extensions(&self) -> impl Iterator<Item = &Extension> {
        self.extensions.iter()
    }

    /// The events this host declared, in sorted order.
    pub fn events(&self) -> impl Iterator<Item = &EventName> {
        self.events.iter()
    }

    /// The host's maximum authority.
    #[must_use]
    pub const fn ceiling(&self) -> &Ceiling {
        &self.ceiling
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use std::fs;

    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::extension::approver::{ApprovalRequest, Decision};
    use crate::modules::{HostModule, InstallContext, stdlib};
    use crate::sandbox::{GrantSet, Policy};
    use crate::test_support::abs;
    use crate::types::ModuleName;

    /// Writes `extension.toml` + `main.lua` into a fresh directory and returns it.
    fn fixture(name: &str, manifest_extra: &str, lua: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("extension.toml"),
            format!(
                "[extension]\nname = \"{name}\"\nversion = \"0.1.0\"\nentry = \"main.lua\"\napi = 1\n{manifest_extra}"
            ),
        )
        .unwrap();
        fs::write(dir.path().join("main.lua"), lua).unwrap();
        dir
    }

    fn host() -> ExtensionHost<ManifestApprover> {
        ExtensionHost::builder()
            .ceiling(Ceiling::new(Policy::confined()).unwrap())
            .events(["ping", "pong"])
            .unwrap()
            .build()
            .unwrap()
    }

    const ECHO: &str = r#"
        airsstack.ext.on("ping", function(p) return { got = p.n } end)
    "#;

    #[test]
    fn build_is_unreachable_without_a_ceiling() {
        // No `build` method exists on this state; the assignment only compiles because the type
        // returned by `builder()` really is `HostBuilder<NoCeiling>`, which is the property this
        // test proves in the absence of a compile-fail harness.
        let _: HostBuilder<NoCeiling> = ExtensionHost::builder();
    }

    #[test]
    fn events_rejects_an_invalid_name() {
        let err = ExtensionHost::builder().events(["not valid!"]).unwrap_err();
        assert!(matches!(err, Error::InvalidName { .. }), "{err}");
    }

    #[test]
    fn events_are_sorted_deduplicated_and_union_across_calls() {
        let host = ExtensionHost::builder()
            .ceiling(Ceiling::new(Policy::confined()).unwrap())
            .events(["pong", "ping", "ping"])
            .unwrap()
            .events(["alpha"])
            .unwrap()
            .build()
            .unwrap();

        let events: Vec<_> = host.events().map(EventName::as_str).collect();
        assert_eq!(events, ["alpha", "ping", "pong"]);
    }

    #[test]
    fn ceiling_returns_the_bound_the_host_was_built_with() {
        let ceiling = Ceiling::new(Policy::confined()).unwrap();
        let host = ExtensionHost::builder()
            .ceiling(ceiling.clone())
            .build()
            .unwrap();

        assert_eq!(host.ceiling(), &ceiling);
    }

    #[test]
    fn extensions_iterates_every_loaded_extension_in_load_order() {
        let first = fixture("first", "", "return 1");
        let second = fixture("second", "", "return 1");

        let mut host = host();
        host.load(first.path()).unwrap();
        host.load(second.path()).unwrap();

        let names: Vec<_> = host
            .extensions()
            .map(|extension| extension.name().as_str())
            .collect();
        assert_eq!(names, ["first", "second"]);
    }

    #[test]
    fn load_registers_the_extension_and_get_finds_it() {
        let dir = fixture("echo", "", ECHO);
        let mut host = host();

        host.load(dir.path()).unwrap();

        assert!(host.get(&ExtensionName::new("echo").unwrap()).is_some());
    }

    #[test]
    fn loading_the_same_name_twice_is_a_duplicate_and_runs_no_code() {
        let first = fixture("dup", "", "return 1");

        let second = TempDir::new().unwrap();
        let marker = second.path().join("marker");
        fs::write(
            second.path().join("extension.toml"),
            format!(
                "[extension]\nname = \"dup\"\nversion = \"0.1.0\"\nentry = \"main.lua\"\napi = 1\n\
                 [capabilities]\nfs.write = [\"{}\"]\n",
                to_script_string(second.path())
            ),
        )
        .unwrap();
        fs::write(
            second.path().join("main.lua"),
            format!(
                "airsstack.fs.write('{}', 'ran')\n",
                to_script_string(&marker)
            ),
        )
        .unwrap();

        let mut host = ExtensionHost::builder()
            .ceiling(
                Ceiling::new(
                    Policy::confined()
                        .with_grants(GrantSet::declared().with_fs(|fs| fs.write(second.path()))),
                )
                .unwrap(),
            )
            .build()
            .unwrap();

        host.load(first.path()).unwrap();
        let err = host.load(second.path()).unwrap_err();

        assert!(
            matches!(&err, Error::DuplicateExtension { extension } if extension == "dup"),
            "{err}"
        );
        assert!(
            !marker.exists(),
            "the second extension's entry must not have run"
        );
    }

    #[test]
    fn load_dir_loads_in_directory_order_and_reports_failures() {
        let root = TempDir::new().unwrap();
        for name in ["b-good", "a-good"] {
            let dir = root.path().join(name);
            fs::create_dir(&dir).unwrap();
            fs::write(
                dir.join("extension.toml"),
                format!("[extension]\nname = \"{name}\"\nversion = \"0.1.0\"\nentry = \"main.lua\"\napi = 1\n"),
            )
            .unwrap();
            fs::write(dir.join("main.lua"), "return 1").unwrap();
        }
        let broken = root.path().join("c-broken");
        fs::create_dir(&broken).unwrap();
        // `abs("/")`, not a bare `"/"`: the manifest validator requires an absolute path before
        // negotiation ever runs, and a driveless `/` fails that check on Windows instead of
        // reaching the ceiling denial this test means to exercise.
        fs::write(
            broken.join("extension.toml"),
            format!(
                "[extension]\nname = \"c-broken\"\nversion = \"0.1.0\"\nentry = \"main.lua\"\napi = 1\n\
                 [capabilities]\nfs.read = [\"{}\"]\n",
                abs("/")
            ),
        )
        .unwrap();
        fs::write(broken.join("main.lua"), "return 1").unwrap();

        let mut host = host();
        let report = host.load_dir(root.path()).unwrap();

        let loaded: Vec<_> = report.loaded().iter().map(ExtensionName::as_str).collect();
        assert_eq!(loaded, ["a-good", "b-good"]);
        assert_eq!(report.failed().len(), 1);
        let (dir, err) = &report.failed()[0];
        assert_eq!(*dir, broken);
        assert!(matches!(err, Error::ExtensionDenied { .. }), "{err}");
    }

    #[test]
    fn load_dir_skips_directories_without_a_manifest() {
        let root = TempDir::new().unwrap();
        let no_manifest = root.path().join("no-manifest");
        fs::create_dir(&no_manifest).unwrap();
        fs::write(no_manifest.join("main.lua"), "return 1").unwrap();

        let mut host = host();
        let report = host.load_dir(root.path()).unwrap();

        assert!(report.is_clean());
        assert!(report.loaded().is_empty());
    }

    #[test]
    fn load_dir_on_a_missing_root_is_io() {
        let mut host = host();
        let err = host.load_dir("/does/not/exist/for/this/test").unwrap_err();
        assert!(
            matches!(
                &err,
                Error::Io {
                    operation: "read_dir",
                    ..
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn broadcast_visits_every_extension_in_load_order_and_isolates_a_failure() {
        let echo = fixture("echo", "", ECHO);
        let erroring = fixture(
            "erroring",
            "",
            r#"airsstack.ext.on("ping", function() error("boom") end)"#,
        );
        let quiet = fixture("quiet", "", "return 1");

        let mut host = host();
        host.load(echo.path()).unwrap();
        host.load(erroring.path()).unwrap();
        host.load(quiet.path()).unwrap();

        let results = host.broadcast(&EventName::new("ping").unwrap(), &json!({"n": 3}));

        assert_eq!(results.len(), 3);
        assert!(
            matches!(results[0].result(), Ok(Some(_))),
            "{:?}",
            results[0].result()
        );
        assert!(
            matches!(results[1].result(), Err(Error::Lua { .. })),
            "{:?}",
            results[1].result()
        );
        assert!(
            matches!(results[2].result(), Ok(None)),
            "{:?}",
            results[2].result()
        );
    }

    struct DenyXPrefixed;

    impl Approver for DenyXPrefixed {
        fn decide(&self, request: &ApprovalRequest<'_>) -> Decision {
            if request.manifest().name().as_str().starts_with('x') {
                Decision::Deny(String::from("names starting with `x` are refused"))
            } else {
                Decision::Approve
            }
        }
    }

    #[test]
    fn a_custom_approver_can_narrow_what_loads() {
        let dir = fixture("x-blocked", "", "return 1");
        let mut host = ExtensionHost::builder()
            .ceiling(Ceiling::new(Policy::confined()).unwrap())
            .approver(DenyXPrefixed)
            .build()
            .unwrap();

        let err = host.load(dir.path()).unwrap_err();

        assert!(
            matches!(&err, Error::ExtensionDenied { detail, .. } if detail.contains('x')),
            "{err}"
        );
    }

    #[test]
    fn variables_reach_the_manifest() {
        let home = TempDir::new().unwrap();
        let data = home.path().canonicalize().unwrap().join("data");
        fs::create_dir(&data).unwrap();
        let dir = fixture(
            "var-ext",
            "[capabilities]\nfs.read = [\"$HOME_DIR/data\"]\n",
            "return 1",
        );

        let mut host = ExtensionHost::builder()
            .ceiling(
                Ceiling::new(
                    Policy::confined()
                        .with_grants(GrantSet::declared().with_fs(|fs| fs.read(&data))),
                )
                .unwrap(),
            )
            .variables([("HOME_DIR", home.path().to_str().unwrap())])
            .build()
            .unwrap();

        let extension = host.load(dir.path()).unwrap();

        assert!(
            extension
                .granted()
                .grants()
                .fs()
                .allows_read(&data.join("notes.txt"))
        );
    }

    struct Probe(ModuleName);

    impl Probe {
        fn new() -> Self {
            Self(ModuleName::new("probe").unwrap())
        }
    }

    impl HostModule for Probe {
        fn name(&self) -> &ModuleName {
            &self.0
        }

        fn install(
            &self,
            _lua: &mlua::Lua,
            _table: &mlua::Table,
            _context: &InstallContext<'_>,
        ) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_custom_module_factory_is_used() {
        let dir = fixture("probe-ext", "[capabilities]\nprobe = true\n", "return 1");

        let mut host = ExtensionHost::builder()
            .ceiling(Ceiling::new(Policy::confined()).unwrap())
            .modules(|| {
                let mut modules = stdlib()?;
                modules.insert(Box::new(Probe::new()))?;
                Ok(modules)
            })
            .build()
            .unwrap();

        assert!(host.load(dir.path()).is_ok());
    }

    #[test]
    fn host_is_send_and_sync() {
        const fn assert<T: Send + Sync>() {}
        assert::<ExtensionHost>();
    }
}
