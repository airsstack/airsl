//! The `require` a confined script gets, resolved against its own directory.
//!
//! Built rather than narrowed. Under every surface below [`LanguageSurface::Full`] Lua's own
//! `require`, `package` and chunk loaders are all absent, so there is nothing to constrain — this
//! installs a Rust function that resolves, checks containment, and loads, with the VM never
//! holding a path or a file.
//!
//! Responsibilities:
//!
//! - [`RequireLoader`], which installs and removes the `require` global for one script root.
//! - Resolution, containment, caching and cycle detection.
//!
//! Non-responsibilities: deciding whether a script gets `require` at all. That follows from the
//! policy and from whether the script has a root, and [`crate::Engine`] decides it.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::{Path, PathBuf};

use mlua::Value;

use crate::error::{Error, Result};
use crate::sandbox::LanguageSurface;
use crate::types::RequireTarget;

/// Registry key the table of already-loaded modules is stored under.
const LOADED_KEY: &str = "airsl.require.loaded";

/// The global a script calls.
const REQUIRE: &str = "require";

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

    /// Whether a script on `surface` should get a confined `require` at all.
    ///
    /// `Full` keeps Lua's own, which resolves through `package.path` and is deliberately left
    /// alone — a first-party script may depend on it. `Minimal` gets none: that surface exists for
    /// evaluating expressions and generated snippets, and a loader that opens files would
    /// contradict the one configuration whose guarantees are meant to be easiest to state.
    pub(crate) const fn applies_to(surface: LanguageSurface) -> bool {
        matches!(surface, LanguageSurface::Restricted)
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
    let key = path.display().to_string();
    let loaded: mlua::Table = lua.named_registry_value(LOADED_KEY).map_err(fail)?;

    match loaded.get::<Value>(key.as_str()).map_err(fail)? {
        // A module still loading has required itself, directly or through a chain. Lua's own
        // `require` detects this; without the check the recursion ends in a stack overflow, which
        // aborts the process rather than raising something a script can catch.
        Value::LightUserData(_) => {
            return Err(Error::RequireCycle {
                module: target.to_string(),
                root: root.display().to_string(),
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
        path: path.display().to_string(),
        source,
    })?;

    lua.load(&source)
        .set_name(format!("@{}", path.display()))
        .eval::<Value>()
        .map_err(|source| Error::lua(target.as_str(), source))
}

/// Finds the file `target` names under `root`, refusing anything outside it.
fn resolve(root: &Path, target: &RequireTarget) -> Result<PathBuf> {
    let root = root.canonicalize().map_err(|_| Error::RequireNotFound {
        module: target.to_string(),
        root: root.display().to_string(),
    })?;

    for candidate in target.candidates() {
        let Ok(path) = root.join(&candidate).canonicalize() else {
            continue;
        };
        // Compared as paths rather than as strings: `Path::starts_with` matches whole components,
        // so a sibling directory whose name merely begins with the root's is not a match. Both
        // sides are canonical, so a symlink pointing out of the root is caught here — the one
        // escape a validated target cannot rule out on its own.
        if !path.starts_with(&root) {
            return Err(Error::RequireEscape {
                module: target.to_string(),
                root: root.display().to_string(),
            });
        }
        return Ok(path);
    }

    Err(Error::RequireNotFound {
        module: target.to_string(),
        root: root.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::RequireLoader;
    use crate::sandbox::LanguageSurface;
    use crate::types::RequireTarget;
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
        assert!(RequireLoader::applies_to(LanguageSurface::Restricted));
        assert!(!RequireLoader::applies_to(LanguageSurface::Full));
        assert!(!RequireLoader::applies_to(LanguageSurface::Minimal));
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
        std::os::unix::fs::symlink(outside.path().join("secrets.lua"), dir.path().join("s.lua"))
            .unwrap();

        let err = resolve(dir.path(), "s").unwrap_err();
        assert!(err.to_string().contains("outside"), "{err}");
    }

    #[test]
    fn a_sibling_directory_sharing_a_name_prefix_is_not_inside_the_root() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("app");
        let decoy = parent.path().join("app-extra");
        std::fs::create_dir_all(&root).unwrap();
        write(&decoy, "m.lua", "return 1");

        std::os::unix::fs::symlink(decoy.join("m.lua"), root.join("m.lua")).unwrap();
        let err = resolve(&root, "m").unwrap_err();
        assert!(err.to_string().contains("outside"), "{err}");
    }
}
