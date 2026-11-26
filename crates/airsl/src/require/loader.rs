//! The `require` a confined script gets, resolved against its own directory.
//!
//! Built rather than narrowed. Under every surface below [`LanguageSurface::Full`] Lua's own
//! `require`, `package` and chunk loaders are all absent, so there is nothing to constrain — this
//! installs a Rust function that resolves, checks containment, and loads, with the VM never
//! holding a path or a file. Where the grant set does not hold [`HostAccess::Require`] it installs
//! a function that refuses every name but the root table's instead.
//!
//! Its own module because installing and running a `require` is the part that touches the VM and
//! the filesystem, where [`super::RequireDisposition`] only chooses between outcomes.
//!
//! Responsibilities:
//!
//! - [`RequireLoader`], which installs the confined `require` for one script root, installs the
//!   `require` that refuses every name but the root table's, and installs the `require` that
//!   answers only the root.
//! - Resolution, containment, caching and cycle detection.
//! - Running a module: read as an expression first, then called with its name and path, the two
//!   arguments Lua's own `require` passes.
//!
//! Non-responsibilities: choosing which `require` a script gets, which is
//! [`super::RequireDisposition`], and applying that choice, which [`crate::Engine`] does because it
//! holds the state.
//!
//! [`LanguageSurface::Full`]: crate::sandbox::LanguageSurface::Full
//! [`HostAccess::Require`]: crate::sandbox::HostAccess::Require
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::{Path, PathBuf};

use mlua::Value;

use crate::error::{Error, Result};
use crate::paths::containment::is_within;
use crate::paths::rules::native::{strip_verbatim, to_script_string};
use crate::require::root;
use crate::types::{RequireTarget, RootTable};

/// Registry key the table of already-loaded modules is stored under.
const LOADED_KEY: &str = "airsl.require.loaded";

/// The global a script calls.
const REQUIRE: &str = "require";

/// Installs a `require` confined to one directory.
#[derive(Debug, Clone)]
pub(crate) struct RequireLoader {
    dir: PathBuf,
    root: RootTable,
}

impl RequireLoader {
    /// A loader confined to `dir`, answering the engine's own `root` table by name before it looks
    /// for a file.
    pub(crate) fn new(dir: impl Into<PathBuf>, root: RootTable) -> Self {
        Self {
            dir: dir.into(),
            root,
        }
    }

    /// Installs `require` into the globals table.
    ///
    /// The argument is converted to a Lua string first, so `nil`, a table or no argument at all
    /// raise the conversion error. A string equal to the root's name returns the root table; any
    /// other is validated and resolved under the directory.
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

        let dir = self.dir.clone();
        let root = self.root.clone();
        let require = lua
            .create_function(move |lua, target: mlua::LuaString| {
                if let Some(table) = root::answer(lua, &root, &target.as_bytes())? {
                    return Ok(Value::Table(table));
                }
                let target = RequireTarget::new(target.to_str()?.to_owned())?;
                // Lua's own `require` also returns the module's path as a second value; this one
                // returns only the module. A second value spills into a multi-value position (the
                // last field of a table constructor, the last argument of a call) and breaks a
                // script that relies on one result. A module reads its own path from `...` instead.
                load(lua, &dir, &target).map_err(mlua::Error::from)
            })
            .map_err(fail)?;

        lua.globals().set(REQUIRE, require).map_err(fail)
    }

    /// Installs a `require` that answers the root table by name and refuses every other call with
    /// [`Error::RequireDenied`] carrying `detail`.
    ///
    /// The first argument is read without being converted, so a call with no argument, a table or
    /// a number is refused the same way as a call with another name; only a Lua string equal to
    /// the root's name is answered. The module cache is not touched: it is unreachable without a
    /// loader to consult it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EngineSetup`] when the function cannot be created or installed.
    pub(crate) fn install_refusal(lua: &mlua::Lua, root: RootTable, detail: String) -> Result<()> {
        let fail = |source: mlua::Error| Error::EngineSetup {
            stage: "refusing require",
            source: Box::new(source),
        };

        let refusal = lua
            .create_function(move |lua, args: mlua::MultiValue| {
                if let Some(Value::String(requested)) = args.front()
                    && let Some(table) = root::answer(lua, &root, &requested.as_bytes())?
                {
                    return Ok(Value::Table(table));
                }
                Err(mlua::Error::from(Error::RequireDenied {
                    detail: detail.clone(),
                }))
            })
            .map_err(fail)?;

        lua.globals().set(REQUIRE, refusal).map_err(fail)
    }

    /// Installs a `require` for a script that has nothing to resolve against: it answers the root
    /// table's name and refuses every other module with [`Error::RequireNoDirectory`].
    ///
    /// The argument is converted and validated exactly as [`RequireLoader::install`] does, so a
    /// non-string raises the same conversion error and a malformed name the same
    /// [`Error::InvalidName`]; only a well-formed name that would have been looked up on disk is
    /// refused here instead. The module cache is not touched, and keeping it means an engine that
    /// alternates between scripts from source and scripts on disk does not re-run the modules the
    /// latter share.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EngineSetup`] when the function cannot be created or installed.
    pub(crate) fn install_root_only(lua: &mlua::Lua, root: RootTable) -> Result<()> {
        let fail = |source: mlua::Error| Error::EngineSetup {
            stage: "root-only require",
            source: Box::new(source),
        };

        let require = lua
            .create_function(move |lua, target: mlua::LuaString| {
                if let Some(table) = root::answer(lua, &root, &target.as_bytes())? {
                    return Ok(Value::Table(table));
                }
                let target = RequireTarget::new(target.to_str()?.to_owned())?;
                Err(mlua::Error::from(Error::RequireNoDirectory {
                    module: target.to_string(),
                }))
            })
            .map_err(fail)?;

        lua.globals().set(REQUIRE, require).map_err(fail)
    }
}

/// Resolves `target` under `root`, loads it once, and returns what it returned
/// (`true` when it returned nothing).
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
            // convention, so a second require does not run it again. The caller receives the
            // recorded value too, so the first require and every later one agree.
            let recorded = if matches!(value, Value::Nil) {
                Value::Boolean(true)
            } else {
                value
            };
            loaded.set(key.as_str(), recorded.clone()).map_err(fail)?;
            Ok(recorded)
        }
        Err(error) => {
            loaded.set(key.as_str(), Value::Nil).map_err(fail)?;
            Err(error)
        }
    }
}

/// Reads the module at `path` and calls it with `target` and its own path, returning its first
/// result.
fn run(lua: &mlua::Lua, path: &Path, target: &RequireTarget) -> Result<Value> {
    // One string for every use, so the chunk name and the path the module is handed cannot differ.
    let script_path = to_script_string(path);

    let source = std::fs::read_to_string(path).map_err(|source| Error::ScriptRead {
        path: script_path.clone(),
        source,
    })?;

    let chunk_name = format!("@{script_path}");

    compile(lua, &source, &chunk_name)
        .and_then(|module| module.call::<Value>((target.as_str(), script_path)))
        .map_err(|source| Error::lua(target.as_str(), source))
}

/// Compiles a module's source into a function, reading it as an expression first.
///
/// When the source is not an expression, its own compile error is the one returned, never the
/// expression attempt's, so a syntax error points at what the author wrote.
fn compile(lua: &mlua::Lua, source: &str, chunk_name: &str) -> mlua::Result<mlua::Function> {
    // This is the order of `mlua::Chunk::eval`: expression first, statements second. It is
    // rebuilt here because `eval` calls the chunk with no arguments and the expression step is
    // private to mlua, so a function that can be given arguments has to be compiled by hand.
    if let Ok(function) = lua
        .load(format!("return {source}"))
        .set_name(chunk_name)
        .into_function()
    {
        return Ok(function);
    }

    lua.load(source).set_name(chunk_name).into_function()
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
        // Stripped before it crosses back out of this function: this value becomes the
        // module-cache key, the chunk name a Lua traceback shows and the path the module is
        // handed, and nothing verbatim may ever reach any of them.
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

    use crate::paths::rules::native::{strip_verbatim, to_script_string};
    use crate::sandbox::{GrantSet, HostAccess};
    use crate::types::{RequireTarget, RootTable};
    use crate::{Engine, Policy, Script};
    use mlua::Value;
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
    fn the_refusing_require_refuses_every_name_but_the_root() {
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        let dir = tempfile::tempdir().unwrap();
        for call in ["require()", "require({})", "require('x')", "require(1)"] {
            write(dir.path(), "main.lua", &format!("return {call}"));
            let script = Script::from_file(dir.path().join("main.lua")).unwrap();
            let err = engine.eval(&script).unwrap_err();
            assert!(
                matches!(
                    crate::test_support::raised(&err),
                    Some(crate::Error::RequireDenied { .. })
                ),
                "{call}: {err}"
            );
            assert!(
                err.to_string()
                    .contains("require denied: `require` is not granted — no access is granted"),
                "{call}: {err}"
            );
        }

        write(
            dir.path(),
            "main.lua",
            "return rawequal(require('airsstack'), airsstack)",
        );
        let script = Script::from_file(dir.path().join("main.lua")).unwrap();
        assert!(engine.eval_to::<bool>(&script).unwrap());
    }

    #[test]
    fn a_confined_require_answers_the_root_before_looking_for_a_file() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "airsstack.lua", "BESIDE_RAN = true return {}");
        write(
            dir.path(),
            "airsstack/init.lua",
            "FOLDER_RAN = true return {}",
        );
        write(
            dir.path(),
            "main.lua",
            "return tostring(rawequal(require('airsstack'), airsstack)) .. '|' \
             .. tostring(BESIDE_RAN) .. '|' .. tostring(FOLDER_RAN)",
        );
        assert_eq!(run_entry(dir.path(), "main.lua").unwrap(), "true|nil|nil");
    }

    /// One script under each of the three installers that answer the root: a file under
    /// `confined` (the directory loader), a source under `pure` (the refusal) and a source under
    /// `confined` (the root-only `require`). Returns the string each evaluates to.
    fn under_each_installer(source: &str) -> [(&'static str, String); 3] {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "main.lua", source);
        let confined = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();
        let pure = Engine::builder().policy(Policy::pure()).build().unwrap();
        let from_source = |engine: &Engine| {
            engine
                .eval_to::<String>(&Script::from_source(source, "probe").unwrap())
                .unwrap()
        };
        [
            (
                "Confined",
                confined
                    .eval_to::<String>(&Script::from_file(dir.path().join("main.lua")).unwrap())
                    .unwrap(),
            ),
            ("Refused", from_source(&pure)),
            ("RootOnly", from_source(&confined)),
        ]
    }

    #[test]
    fn the_root_only_require_answers_the_root_and_refuses_other_names_for_want_of_a_directory() {
        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();
        let script = |source: &str| Script::from_source(source, "probe").unwrap();

        assert!(
            engine
                .eval_to::<bool>(&script("return rawequal(require('airsstack'), airsstack)"))
                .unwrap()
        );

        for (call, module) in [("require('lib')", "lib"), ("require(1)", "1")] {
            let err = engine.eval(&script(&format!("return {call}"))).unwrap_err();
            assert!(
                matches!(
                    crate::test_support::raised(&err),
                    Some(crate::Error::RequireNoDirectory { .. })
                ),
                "{call}: {err}"
            );
            assert!(
                err.to_string().contains(&format!(
                    "module `{module}` cannot be loaded: the script has no directory to load modules from"
                )),
                "{call}: {err}"
            );
        }
    }

    #[test]
    fn the_root_only_require_names_its_stage_when_the_state_cannot_hold_it() {
        let lua = mlua::Lua::new();
        // Collect twice so nothing reclaimable is left, then cap the state at exactly what it
        // uses: the installer's first allocation has no headroom and must be refused. Filling the
        // state up to a margin instead would pass or fail with the margin, because whether the
        // installer's allocations still fit depends on how the filler happened to land.
        lua.gc_collect().unwrap();
        lua.gc_collect().unwrap();
        lua.set_memory_limit(lua.used_memory()).unwrap();

        let err = super::RequireLoader::install_root_only(&lua, RootTable::default()).unwrap_err();

        assert!(
            matches!(
                err,
                crate::Error::EngineSetup {
                    stage: "root-only require",
                    ..
                }
            ),
            "{err}"
        );
        assert!(
            err.to_string()
                .contains("cannot configure the Lua state (root-only require)"),
            "{err}"
        );
    }

    /// The message a failed `require` raised, without the traceback, whose last frame names the
    /// script and so differs between a file and a source.
    fn message_of(raised: &str) -> &str {
        raised
            .split_once("\nstack traceback:")
            .map_or(raised, |(message, _)| message)
    }

    #[test]
    fn the_root_only_require_rejects_an_invalid_target_as_the_confined_one_does() {
        let found = under_each_installer("return tostring(select(2, pcall(require, '../x')))");
        let [(_, confined), _, (_, root_only)] = found;
        assert!(confined.contains("require target"), "{confined}");
        assert!(
            message_of(&confined).contains("require target"),
            "{confined}"
        );
        assert_eq!(message_of(&root_only), message_of(&confined));
    }

    #[test]
    fn the_root_only_require_rejects_a_non_string_as_the_confined_one_does() {
        for call in [
            "pcall(require)",
            "pcall(require, nil)",
            "pcall(require, {})",
        ] {
            let found = under_each_installer(&format!("return tostring(select(2, {call}))"));
            let [(_, confined), _, (_, root_only)] = found;
            assert!(!message_of(&confined).is_empty(), "{call}");
            assert_eq!(message_of(&root_only), message_of(&confined), "{call}");
        }
    }

    #[test]
    fn a_root_require_returns_exactly_one_value() {
        for (label, found) in
            under_each_installer("return tostring(select('#', require('airsstack')))")
        {
            assert_eq!(found, "1", "{label}");
        }
    }

    #[test]
    fn a_root_require_ignores_arguments_after_the_first() {
        for (label, found) in
            under_each_installer("return tostring(rawequal(require('airsstack', 'x'), airsstack))")
        {
            assert_eq!(found, "true", "{label}");
        }
    }

    /// A `pure` engine holding only the `require` member, which is the smallest policy that
    /// still loads modules.
    fn pure_with_require() -> Engine {
        Engine::builder()
            .policy(Policy::pure().with_grants(GrantSet::sealed().with_access(HostAccess::Require)))
            .build()
            .unwrap()
    }

    #[test]
    fn a_pure_script_with_require_granted_loads_its_siblings() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "lib.lua", "return 42");
        write(dir.path(), "main.lua", "return require('lib')");
        let script = Script::from_file(dir.path().join("main.lua")).unwrap();
        assert_eq!(pure_with_require().eval_to::<i64>(&script).unwrap(), 42);
    }

    #[test]
    fn a_pure_script_with_require_granted_cannot_load_outside_its_folder() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "main.lua", "return require('../secrets')");
        let script = Script::from_file(dir.path().join("main.lua")).unwrap();
        let err = pure_with_require().eval(&script).unwrap_err();
        assert!(err.to_string().contains("require target"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn a_pure_script_with_require_granted_cannot_follow_a_symlink_out_of_its_folder() {
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "secrets.lua", "return 'leaked'");

        let dir = tempfile::tempdir().unwrap();
        crate::test_support::link_file(
            &outside.path().join("secrets.lua"),
            &dir.path().join("s.lua"),
        )
        .unwrap();
        write(dir.path(), "main.lua", "return require('s')");

        let script = Script::from_file(dir.path().join("main.lua")).unwrap();
        let err = pure_with_require().eval(&script).unwrap_err();
        assert!(
            matches!(
                crate::test_support::raised(&err),
                Some(crate::Error::RequireEscape { .. })
            ),
            "{err}"
        );
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

    fn compile(lua: &mlua::Lua, source: &str) -> mlua::Result<mlua::Function> {
        super::compile(lua, source, "@/abs/m.lua")
    }

    const NAME: &str = "pkg.mod";
    const PATH: &str = "/abs/m.lua";

    #[test]
    fn compile_runs_a_block_that_returns_its_first_argument() {
        let lua = mlua::Lua::new();
        let f = compile(&lua, "return (...)").unwrap();
        assert_eq!(f.call::<String>((NAME, PATH)).unwrap(), NAME);
    }

    #[test]
    fn compile_passes_both_arguments() {
        let lua = mlua::Lua::new();
        let f = compile(&lua, "return select('#', ...)").unwrap();
        assert_eq!(f.call::<i64>((NAME, PATH)).unwrap(), 2);
    }

    #[test]
    fn compile_runs_a_bare_vararg_expression() {
        let lua = mlua::Lua::new();
        let f = compile(&lua, "...").unwrap();
        assert_eq!(f.call::<String>((NAME, PATH)).unwrap(), NAME);
    }

    #[test]
    fn compile_runs_a_bare_table_expression() {
        let lua = mlua::Lua::new();
        let f = compile(&lua, "{ v = 'bare' }").unwrap();
        let table = f.call::<mlua::Table>((NAME, PATH)).unwrap();
        assert_eq!(table.get::<String>("v").unwrap(), "bare");
    }

    #[test]
    fn compile_prefers_the_expression_reading() {
        let lua = mlua::Lua::new();
        let f = compile(&lua, "tostring(1)").unwrap();
        assert_eq!(
            f.call::<Option<String>>((NAME, PATH)).unwrap().unwrap(),
            "1"
        );
    }

    #[test]
    fn compile_reports_the_statement_parse_error() {
        let lua = mlua::Lua::new();
        let err = compile(&lua, "local x = \n").unwrap_err();
        assert!(
            matches!(err, mlua::Error::SyntaxError { .. }),
            "not a syntax error: {err}"
        );
        assert!(
            err.to_string().contains(":2: unexpected symbol near <eof>"),
            "{err}"
        );
        // The chunk name is set on the statement attempt too, so the error names the module's file.
        assert!(err.to_string().contains("/abs/m.lua:2:"), "{err}");
    }

    #[test]
    fn compile_keeps_only_the_first_of_several_return_values() {
        let lua = mlua::Lua::new();
        let f = compile(&lua, "return { v = 1 }, 'extra'").unwrap();
        let value = f.call::<Value>((NAME, PATH)).unwrap();
        let table = value.as_table().unwrap();
        assert_eq!(table.get::<i64>("v").unwrap(), 1);
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

    /// Runs `entry` under `root` on a fresh engine and returns the string it returns.
    fn run_entry(root: &Path, entry: &str) -> crate::Result<String> {
        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();
        let script = Script::from_file(root.join(entry)).unwrap();
        engine.eval_to::<String>(&script)
    }

    #[test]
    fn a_required_module_receives_its_name_and_path_as_its_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        write(
            &root,
            "lib/m.lua",
            "return { n = select('#', ...), name = (...), path = select(2, ...) }",
        );
        write(
            &root,
            "main.lua",
            "local m = require('lib.m')\nreturn m.n .. '|' .. m.name .. '|' .. m.path",
        );

        let expected = to_script_string(&strip_verbatim(
            root.join("lib/m.lua").canonicalize().unwrap(),
        ));
        assert_eq!(
            run_entry(&root, "main.lua").unwrap(),
            format!("2|lib.m|{expected}")
        );
    }

    #[test]
    fn a_module_reaches_its_sibling_through_its_own_name_from_either_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "main.lua",
            "local f = require(\"core.hooks.enforce\")\nreturn f()\n",
        );
        write(
            root,
            "core/hooks/enforce.lua",
            "local base = (...):match(\"^(.-)[^.]*$\")\nlocal lib = require(base .. \"lib.x\")\nreturn function() return \"lib=\" .. lib.v .. \" base=[\" .. base .. \"]\" end\n",
        );
        write(root, "core/hooks/lib/x.lua", "return { v = \"ok\" }\n");
        write(
            root,
            "core/hooks/from_hooks.lua",
            "local f = require(\"enforce\")\nreturn f()\n",
        );

        // A fresh engine per root: the module cache keeps the name a file was first reached under.
        assert_eq!(
            run_entry(root, "main.lua").unwrap(),
            "lib=ok base=[core.hooks.]"
        );
        assert_eq!(
            run_entry(root, "core/hooks/from_hooks.lua").unwrap(),
            "lib=ok base=[]"
        );
    }

    #[test]
    fn a_module_written_as_a_bare_expression_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "m.lua", "{ v = 'bare' }\n");
        write(dir.path(), "main.lua", "return require('m').v");
        assert_eq!(run_entry(dir.path(), "main.lua").unwrap(), "bare");
    }

    #[test]
    fn a_module_that_is_both_an_expression_and_a_statement_returns_the_expression() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "m.lua", "tostring(1)\n");
        write(dir.path(), "main.lua", "return tostring(require('m'))");
        assert_eq!(run_entry(dir.path(), "main.lua").unwrap(), "1");
    }

    #[test]
    fn a_syntax_error_in_a_module_reports_the_statement_parse() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "m.lua", "local x = \n");
        write(dir.path(), "main.lua", "return require('m')");
        let err = run_entry(dir.path(), "main.lua").unwrap_err();
        assert!(matches!(err, crate::Error::Lua { .. }), "{err}");
        assert!(
            err.to_string().contains(":2: unexpected symbol near <eof>"),
            "{err}"
        );
    }

    #[test]
    fn a_module_returning_several_values_yields_only_the_first() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "m.lua", "return { v = 1 }, 'extra'");
        write(
            dir.path(),
            "main.lua",
            "local a, b = require('m'); return tostring(b) .. ':' .. a.v",
        );
        assert_eq!(run_entry(dir.path(), "main.lua").unwrap(), "nil:1");
    }

    #[test]
    fn require_returns_exactly_one_value_on_the_first_load_and_on_a_cache_hit() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "m.lua", "return { v = 1 }");
        write(
            dir.path(),
            "main.lua",
            "local function pack(...) return select('#', ...), ... end\n\
             local first_count, first = pack(require('m'))\n\
             local cached_count, cached = pack(require('m'))\n\
             return first_count .. '|' .. cached_count .. '|' .. tostring(first == cached)",
        );
        assert_eq!(run_entry(dir.path(), "main.lua").unwrap(), "1|1|true");
    }

    #[test]
    fn a_require_in_a_multi_value_position_contributes_one_value() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.lua", "return { v = 'a' }");
        write(dir.path(), "b.lua", "return { v = 'b' }");
        write(
            dir.path(),
            "main.lua",
            "local fields = { require('a'), require('b') }\n\
             local inserted = pcall(table.insert, {}, require('a'))\n\
             return #fields .. '|' .. tostring(inserted)",
        );
        assert_eq!(run_entry(dir.path(), "main.lua").unwrap(), "2|true");
    }

    #[test]
    fn a_module_returning_false_yields_false_and_runs_once() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "m.lua", "runs = (runs or 0) + 1\nreturn false");
        write(
            dir.path(),
            "main.lua",
            "local first = require('m')\n\
             local second = require('m')\n\
             return tostring(first) .. '|' .. tostring(second) .. '|' .. runs",
        );
        assert_eq!(run_entry(dir.path(), "main.lua").unwrap(), "false|false|1");
    }

    #[test]
    fn a_module_returning_nothing_yields_true_on_its_first_require() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "m.lua", "runs = (runs or 0) + 1");
        write(
            dir.path(),
            "main.lua",
            "local first = { require('m') }\n\
             local second = { require('m') }\n\
             return tostring(first[1]) .. '|' .. #first .. '|' .. tostring(second[1]) .. '|' \
             .. #second .. '|' .. select('#', require('m')) .. '|' .. runs",
        );
        assert_eq!(
            run_entry(dir.path(), "main.lua").unwrap(),
            "true|1|true|1|1|1"
        );
    }

    #[test]
    fn a_file_reached_under_two_names_runs_once_and_sees_the_first_name() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "lib/init.lua",
            "runs = (runs or 0) + 1\nreturn { name = (...) }",
        );
        write(
            dir.path(),
            "main.lua",
            "local a = require('lib')\n\
             local b = require('lib.init')\n\
             return tostring(a == b) .. '|' .. a.name .. '|' .. runs",
        );
        assert_eq!(run_entry(dir.path(), "main.lua").unwrap(), "true|lib|1");
    }
}
