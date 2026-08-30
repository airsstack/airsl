//! The `require` a confined script gets, resolved against its own directory.
//!
//! Built rather than narrowed. Under every surface below [`LanguageSurface::Full`] Lua's own
//! `require`, `package` and chunk loaders are all absent, so there is nothing to constrain — this
//! installs a Rust function that resolves, checks containment, and loads, with the VM never
//! holding a path or a file.
//!
//! Responsibilities:
//!
//! - [`RequireDisposition`], which of the three possible `require` globals a script should get.
//! - [`RequireLoader`], which installs and removes the `require` global for one script root.
//! - Resolution, containment, caching and cycle detection.
//!
//! Non-responsibilities: applying the disposition. [`crate::Engine`] holds the state and does that.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::{Path, PathBuf};

use mlua::Value;

use crate::error::{Error, Result};
use crate::paths::containment::is_within;
use crate::paths::rules::native::{strip_verbatim, to_script_string};
use crate::sandbox::LanguageSurface;
use crate::types::RequireTarget;

/// Registry key the table of already-loaded modules is stored under.
const LOADED_KEY: &str = "airsl.require.loaded";

/// The global a script calls.
const REQUIRE: &str = "require";

/// Which `require` a script should be given, before an engine goes and installs it.
///
/// An enum rather than a predicate because the answer has three values, not two. Asking only
/// "should this script get a *confined* `require`" collapses [`LanguageSurface::Full`] and
/// [`LanguageSurface::Minimal`] onto one `false`, and those two must not share an answer: `Full`
/// keeps the loader `package` installed, `Minimal` is entitled to none. Naming the third case is
/// what stops a `Full` engine from clearing the global its own surface promises to provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequireDisposition<'a> {
    /// Leave Lua's own `require` exactly as `luaopen_package` installed it.
    Native,

    /// Install a `require` resolving under this directory and nowhere else.
    Confined(&'a Path),

    /// Clear `require`. Either the surface withholds it or the script has no directory to
    /// resolve against.
    Absent,
}

impl<'a> RequireDisposition<'a> {
    /// The disposition for a script with `root`, running on `surface`.
    ///
    /// `Full` keeps Lua's own, which resolves through `package.path` and is deliberately left
    /// alone — a first-party script may depend on it, and the surface documents `require` as part
    /// of what it grants. `Restricted` gets the confined loader, but only where there is a
    /// directory to confine it to: a script built from source has none. `Minimal` gets nothing,
    /// because that surface exists for evaluating expressions and generated snippets, and a loader
    /// that opens files would contradict the one configuration whose guarantees are meant to be
    /// easiest to state.
    ///
    /// The match is exhaustive on purpose. A fourth surface should not be able to acquire a
    /// default here by falling into a wildcard; it should fail to compile until someone decides.
    pub(crate) const fn decide(surface: LanguageSurface, root: Option<&'a Path>) -> Self {
        match (surface, root) {
            (LanguageSurface::Full, _) => Self::Native,
            (LanguageSurface::Restricted, Some(root)) => Self::Confined(root),
            (LanguageSurface::Restricted | LanguageSurface::Minimal, _) => Self::Absent,
        }
    }
}

/// Installs a `require` confined to one directory.
#[derive(Debug, Clone)]
pub(crate) struct RequireLoader {
    root: PathBuf,
}

impl RequireLoader {
    /// A loader confined to `root`.
    pub(crate) fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Installs `require` into the globals table.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EngineSetup`] when the function or its cache cannot be created.
    pub(crate) fn install(&self, lua: &mlua::Lua) -> Result<()> {
        let fail = |source: mlua::Error| Error::EngineSetup {
            stage: "confined require",
            source: Box::new(source),
        };

        // Created once and kept, matching what `package.loaded` does for Lua's own `require`.
        // Recreating it per evaluation made a reused engine re-run every module it required,
        // which is the opposite of what a dispatch path wants and is invisible in a CLI that
        // runs one script per process.
        if !lua
            .named_registry_value::<Value>(LOADED_KEY)
            .is_ok_and(|v| v.is_table())
        {
            let loaded = lua.create_table().map_err(fail)?;
            lua.set_named_registry_value(LOADED_KEY, loaded)
                .map_err(fail)?;
        }

        let root = self.root.clone();
        let require = lua
            .create_function(move |lua, target: mlua::LuaString| {
                let target = RequireTarget::new(target.to_str()?.to_owned())?;
                load(lua, &root, &target).map_err(mlua::Error::from)
            })
            .map_err(fail)?;

        lua.globals().set(REQUIRE, require).map_err(fail)
    }

    /// Removes `require`, for a script that is not entitled to one.
    ///
    /// The cache is left in place. It is unreachable without `require` to consult it, and keeping
    /// it means an engine that alternates between scripts from source and scripts on disk does not
    /// re-run the modules the latter share.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EngineSetup`] when the global cannot be cleared.
    pub(crate) fn remove(lua: &mlua::Lua) -> Result<()> {
        lua.globals()
            .set(REQUIRE, Value::Nil)
            .map_err(|source| Error::EngineSetup {
                stage: "confined require",
                source: Box::new(source),
            })
    }
}

/// Resolves `target` under `root`, loads it once, and returns what it returned.
fn load(lua: &mlua::Lua, root: &Path, target: &RequireTarget) -> Result<Value> {
    let fail = |source: mlua::Error| Error::lua(target.as_str(), source);

    let path = resolve(root, target)?;
    // Not `to_script_string`: this key is an internal lookup, never shown to a script, and
    // re-spelling it to fix a display problem would change what it is keyed on instead.
    let key = path.display().to_string();
    let loaded: mlua::Table = lua.named_registry_value(LOADED_KEY).map_err(fail)?;

    match loaded.get::<Value>(key.as_str()).map_err(fail)? {
        // A module still loading has required itself, directly or through a chain. Lua's own
        // `require` detects this; without the check the recursion ends in a stack overflow, which
        // aborts the process rather than raising something a script can catch.
        Value::LightUserData(_) => {
            return Err(Error::RequireCycle {
                module: target.to_string(),
                root: to_script_string(root),
            });
        }
        Value::Nil => {}
        cached => return Ok(cached),
    }

    let in_progress = Value::LightUserData(mlua::LightUserData(std::ptr::null_mut()));
    loaded.set(key.as_str(), in_progress).map_err(fail)?;

    // The marker must not survive a failure. The cache outlives the evaluation that filled it, so
    // a module that raised once would afterwards be reported as a cycle — a wrong diagnosis of a
    // real error, and a permanent one for as long as the engine lives.
    match run(lua, &path, target) {
        Ok(value) => {
            // A module that returns nothing is still loaded; record that, matching Lua's own
            // convention, so a second require does not run it again.
            let recorded = if matches!(value, Value::Nil) {
                Value::Boolean(true)
            } else {
                value.clone()
            };
            loaded.set(key.as_str(), recorded).map_err(fail)?;
            Ok(value)
        }
        Err(error) => {
            loaded.set(key.as_str(), Value::Nil).map_err(fail)?;
            Err(error)
        }
    }
}

/// Reads and evaluates the module at `path`.
fn run(lua: &mlua::Lua, path: &Path, target: &RequireTarget) -> Result<Value> {
    let source = std::fs::read_to_string(path).map_err(|source| Error::ScriptRead {
        path: to_script_string(path),
        source,
    })?;

    lua.load(&source)
        .set_name(format!("@{}", to_script_string(path)))
        .eval::<Value>()
        .map_err(|source| Error::lua(target.as_str(), source))
}

/// Finds the file `target` names under `root`, refusing anything outside it.
fn resolve(root: &Path, target: &RequireTarget) -> Result<PathBuf> {
    let root = root.canonicalize().map_err(|_| Error::RequireNotFound {
        module: target.to_string(),
        // `root` here is still the caller-supplied path: the shadowed, canonicalised binding
        // this `let` introduces has not taken effect inside its own initialiser.
        root: to_script_string(root),
    })?;

    for candidate in target.candidates() {
        let Ok(path) = root.join(&candidate).canonicalize() else {
            continue;
        };
        // Compared component-wise via the shared containment predicate, never as strings: a
        // sibling directory whose name merely begins with the root's is not a match. Both sides
        // are canonical, so a symlink pointing out of the root is caught here — the one escape a
        // validated target cannot rule out on its own.
        if !is_within(&path, &root) {
            return Err(Error::RequireEscape {
                module: target.to_string(),
                // `root` here is the canonicalised binding, so on Windows it would otherwise carry
                // a verbatim `\\?\` prefix into a message a script reads. `to_script_string` strips
                // it and re-spells the separator in the same call.
                root: to_script_string(&root),
            });
        }
        // Stripped before it crosses back out of this function: this value becomes both the
        // module-cache key and the chunk name a Lua traceback shows, and nothing verbatim may ever
        // reach either.
        return Ok(strip_verbatim(path));
    }

    Err(Error::RequireNotFound {
        module: target.to_string(),
        // Same canonicalised binding as the escape arm above, and the same reason it converts.
        root: to_script_string(&root),
    })
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::RequireDisposition;
    use crate::sandbox::LanguageSurface;
    use crate::types::RequireTarget;
    use crate::{Engine, Policy, Script};
    use std::io::Write as _;
    use std::path::Path;

    fn write(dir: &Path, name: &str, body: &str) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(body.as_bytes()).unwrap();
    }

    fn resolve(root: &Path, target: &str) -> crate::Result<std::path::PathBuf> {
        super::resolve(root, &RequireTarget::new(target).unwrap())
    }

    #[test]
    fn only_the_restricted_surface_gets_a_confined_require() {
        let root = Path::new("/scripts");
        assert_eq!(
            RequireDisposition::decide(LanguageSurface::Restricted, Some(root)),
            RequireDisposition::Confined(root)
        );
        assert_eq!(
            RequireDisposition::decide(LanguageSurface::Full, Some(root)),
            RequireDisposition::Native
        );
        assert_eq!(
            RequireDisposition::decide(LanguageSurface::Minimal, Some(root)),
            RequireDisposition::Absent
        );
    }

    #[test]
    fn the_full_surface_keeps_lua_s_own_require_whether_or_not_the_script_has_a_root() {
        // The distinction this enum exists for: `Full` and `Minimal` both decline the confined
        // loader, but only one of them wants the global cleared.
        for root in [None, Some(Path::new("/scripts"))] {
            assert_eq!(
                RequireDisposition::decide(LanguageSurface::Full, root),
                RequireDisposition::Native,
                "{root:?}"
            );
        }
    }

    #[test]
    fn a_script_with_no_root_has_nothing_to_resolve_against_and_gets_no_require() {
        for surface in [LanguageSurface::Restricted, LanguageSurface::Minimal] {
            assert_eq!(
                RequireDisposition::decide(surface, None),
                RequireDisposition::Absent,
                "{surface}"
            );
        }
    }

    #[test]
    fn a_sibling_module_resolves() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "index.lua", "return 1");
        assert!(resolve(dir.path(), "index").is_ok());
    }

    #[test]
    fn a_nested_module_resolves_through_its_dotted_name() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "lib/index.lua", "return 1");
        assert!(resolve(dir.path(), "lib.index").is_ok());
    }

    #[test]
    fn a_directory_module_resolves_through_init() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "lib/init.lua", "return 1");
        assert!(resolve(dir.path(), "lib").is_ok());
    }

    #[test]
    fn a_missing_module_names_the_directory_it_searched() {
        let dir = tempfile::tempdir().unwrap();
        let err = resolve(dir.path(), "absent").unwrap_err();
        assert!(err.to_string().contains("not found"), "{err}");
    }

    #[test]
    fn a_symlink_out_of_the_root_is_refused() {
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "secrets.lua", "return 'leaked'");

        let dir = tempfile::tempdir().unwrap();
        crate::test_support::link_file(
            &outside.path().join("secrets.lua"),
            &dir.path().join("s.lua"),
        )
        .unwrap();

        let err = resolve(dir.path(), "s").unwrap_err();
        assert!(err.to_string().contains("outside"), "{err}");
    }

    #[test]
    fn a_module_loaded_as_a_file_and_the_same_module_reached_through_require_report_the_same_traceback_name()
     {
        // `ChunkName::from_path` (the file route) and this module's own `run` (the `require`
        // route) each render the same physical file's path independently. If only one of them
        // converted through the script vocabulary, the two routes would spell the same file two
        // ways in a traceback.
        //
        // Both errors wrap the location a different distance from the top — the file route's
        // outer wrapper names the whole script, the require route's inner wrapper names the
        // module by its dotted target rather than a path — so the one place both share the exact
        // same rendering of `nested.lua`'s own path is the stack-traceback frame Lua emits for it.
        // That frame is what this test compares, rather than a precomputed expected string,
        // because Lua also truncates a long chunk id to fit its fixed-size buffer: the truncation
        // is deterministic on the underlying name, so two byte-identical names still truncate to
        // byte-identical frames, but a bare `.contains(expected)` would spuriously fail once a
        // temp-directory path is long enough to trigger it — as it is on this host.
        //
        // On unix `to_script_string` is the identity, so the two frames compared below would be
        // byte-identical whether or not either route actually converts — this test cannot
        // distinguish "both converted", "neither converted" and "only one converted" on this
        // host. Only a Windows host, where the two routes' inputs diverge (one is canonicalised
        // through `resolve`, the other is not), can fail this for the reason it exists to catch.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        write(&root, "lib/nested.lua", "error('boom')");
        write(&root, "main.lua", "return require('lib.nested')");

        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();

        let direct = Script::from_file(root.join("lib/nested.lua")).unwrap();
        let direct_err = engine.eval(&direct).unwrap_err().to_string();
        let direct_frame = direct_err
            .lines()
            .find(|line| line.contains("nested.lua:1: in main chunk"))
            .unwrap();

        let main = Script::from_file(root.join("main.lua")).unwrap();
        let required_err = engine.eval(&main).unwrap_err().to_string();
        let required_frame = required_err
            .lines()
            .find(|line| line.contains("nested.lua:1: in main chunk"))
            .unwrap();

        assert_eq!(
            direct_frame.trim(),
            required_frame.trim(),
            "direct: {direct_err}\nrequired: {required_err}"
        );
    }

    #[test]
    fn a_sibling_directory_sharing_a_name_prefix_is_not_inside_the_root() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("app");
        let decoy = parent.path().join("app-extra");
        std::fs::create_dir_all(&root).unwrap();
        write(&decoy, "m.lua", "return 1");

        crate::test_support::link_file(&decoy.join("m.lua"), &root.join("m.lua")).unwrap();
        let err = resolve(&root, "m").unwrap_err();
        assert!(err.to_string().contains("outside"), "{err}");
    }
}
