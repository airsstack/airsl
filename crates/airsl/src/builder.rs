//! Type-state construction of an [`Engine`].
//!
//! A builder rather than a constructor because building an engine has one decision that must not
//! be defaulted silently — the policy — and several that should be. The type-state parameter makes
//! the required one a compile error rather than a runtime check: there is no `build` method until
//! [`EngineBuilder::policy`] has been called.
//!
//! Responsibilities:
//!
//! - [`EngineBuilder`] and the [`Missing`] / [`Present`] state markers.
//! - Applying a policy to a fresh [`mlua::Lua`] and installing the host modules.
//!
//! Non-responsibilities: running scripts. That is [`Engine`]'s job.

use mlua::Table;

use crate::engine::Engine;
use crate::error::{Error, Result};
use crate::modules::{InstallContext, ModuleSet, stdlib};
use crate::sandbox::Policy;
use crate::sandbox::language_surface::{
    CHUNK_LOADERS, MINIMAL_RANDOM_SEED, MINIMAL_WITHHELD_GLOBALS, UNSAFE_OS_FUNCTIONS,
};
use crate::types::RootTable;

/// Type-state marker: the policy has not been chosen yet.
#[derive(Debug)]
pub struct Missing;

/// Type-state marker: the policy has been chosen.
#[derive(Debug)]
pub struct Present;

/// Configures and builds an [`Engine`].
#[derive(Debug)]
pub struct EngineBuilder<S> {
    policy: Policy,
    modules: Option<ModuleSet>,
    root: RootTable,
    state: core::marker::PhantomData<S>,
}

impl EngineBuilder<Missing> {
    /// Starts with no policy chosen.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            policy: Policy::confined(),
            modules: None,
            root: RootTable::default(),
            state: core::marker::PhantomData,
        }
    }

    /// Chooses what scripts may reach and what they may consume.
    ///
    /// This is the one required step; [`EngineBuilder::build`] does not exist until it is called.
    #[must_use]
    pub fn policy(self, policy: Policy) -> EngineBuilder<Present> {
        EngineBuilder {
            policy,
            modules: self.modules,
            root: self.root,
            state: core::marker::PhantomData,
        }
    }
}

impl EngineBuilder<Present> {
    /// Replaces the default host-module set.
    ///
    /// Without this the engine installs [`stdlib()`]. Pass a set built from [`ModuleSet::new`] to
    /// install only specific modules, or extend [`stdlib()`] to add to them.
    #[must_use]
    pub fn stdlib(mut self, modules: ModuleSet) -> Self {
        self.modules = Some(modules);
        self
    }

    /// Replaces the global the host modules are installed under.
    ///
    /// Defaults to `airsstack`. An embedding host should set its own, so that a module contributed
    /// by a third party does not land in a namespace named after somebody else's system.
    #[must_use]
    pub fn root_table(mut self, root: RootTable) -> Self {
        self.root = root;
        self
    }

    /// Builds the engine: creates the Lua state, applies the policy, and installs the modules.
    ///
    /// The resource ceilings are armed before the modules are installed, so no caller can ever
    /// hold an engine whose limits are not yet in force.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EngineSetup`] when the state cannot be created or capped, and
    /// [`Error::ModuleInstall`] when the root table cannot be created or a host module fails to
    /// install.
    pub fn build(self) -> Result<Engine> {
        let lua = mlua::Lua::new_with(
            self.policy.language().libraries(),
            mlua::LuaOptions::default(),
        )
        .map_err(|source| Error::EngineSetup {
            stage: "creating the state",
            source: Box::new(source),
        })?;

        if self.policy.language().withholds_unsafe_globals() {
            withhold_unsafe_globals(&lua)?;
            protect_string_metatable(&lua)?;
        }
        if self.policy.language().closes_ambient_channels() {
            withhold_minimal_globals(&lua)?;
            seed_minimal_random(&lua)?;
        }
        // Seeding must stay ahead of `apply`, so a script is never charged for the host's
        // set-up. No test can observe a move: the instruction hook does not count C calls made
        // from Rust, so a seeding moved below `apply` would still leave every test green.
        let budget = self.policy.limits().apply(&lua)?;

        let modules = match self.modules {
            Some(set) => set,
            None => stdlib()?,
        };
        install_modules(&lua, &modules, &self.policy, &self.root)?;

        Ok(Engine::from_parts(
            lua,
            modules,
            self.policy,
            budget,
            self.root,
        ))
    }
}

/// Removes the chunk loaders and the unsafe `os` functions from the globals table.
fn withhold_unsafe_globals(lua: &mlua::Lua) -> Result<()> {
    let fail = |source: mlua::Error| Error::EngineSetup {
        stage: "withholding unsafe globals",
        source: Box::new(source),
    };

    let globals = lua.globals();
    for name in CHUNK_LOADERS {
        globals.set(name, mlua::Value::Nil).map_err(fail)?;
    }

    if let Ok(os) = globals.get::<Table>("os") {
        for name in UNSAFE_OS_FUNCTIONS {
            os.set(name, mlua::Value::Nil).map_err(fail)?;
        }
    }

    Ok(())
}

/// Removes the globals only the minimal surface withholds, such as `print`.
fn withhold_minimal_globals(lua: &mlua::Lua) -> Result<()> {
    let globals = lua.globals();
    for name in MINIMAL_WITHHELD_GLOBALS {
        globals
            .set(name, mlua::Value::Nil)
            .map_err(|source| Error::EngineSetup {
                stage: "withholding minimal-surface globals",
                source: Box::new(source),
            })?;
    }
    Ok(())
}

/// Makes `math.random` repeatable and stops a script re-seeding it from the clock.
///
/// Lua seeds `math.random` from the clock when the state is created, and `math.randomseed()` with
/// no argument re-seeds from it and returns it. Both are a read of the host's clock, so the state
/// is seeded with [`MINIMAL_RANDOM_SEED`] through Lua's own `math.randomseed`, and the global is
/// replaced by a function that holds the original and refuses only the no-argument form. Any
/// other call goes to the original unchanged, so a script that seeds itself keeps working. This
/// runs before the instruction ceiling is armed, so a script is not charged for the host's set-up.
fn seed_minimal_random(lua: &mlua::Lua) -> Result<()> {
    let fail = |source: mlua::Error| Error::EngineSetup {
        stage: "seeding math.random",
        source: Box::new(source),
    };

    let math: Table = lua.globals().get("math").map_err(fail)?;
    let original: mlua::Function = math.get("randomseed").map_err(fail)?;
    original.call::<()>(MINIMAL_RANDOM_SEED).map_err(fail)?;

    let guarded = lua
        .create_function(move |_, args: mlua::MultiValue| {
            if args.is_empty() {
                return Err(mlua::Error::from(Error::Denied {
                    module: "math",
                    operation: "randomseed",
                    detail: "a seed is required — the no-argument form reads the clock".into(),
                }));
            }
            original.call::<mlua::MultiValue>(args)
        })
        .map_err(fail)?;
    math.set("randomseed", guarded).map_err(fail)
}

/// Hides the string metatable behind `__metatable`, so a script cannot reach it.
///
/// Every Lua string shares one metatable whose `__index` is the `string` library. A script that
/// reaches it can replace a method for every string in the state — including for scripts that run
/// afterwards on the same engine, which never touch a global and would have no way to notice:
///
/// ```lua
/// getmetatable('').__index.upper = function() return 'PWNED' end
/// ```
///
/// Setting `__metatable` makes `getmetatable('')` return that value instead of the real table and
/// makes `setmetatable` on a string raise. Method calls are unaffected — `('x'):upper()` resolves
/// through the metatable directly rather than through `getmetatable`.
///
/// Not applied on [`LanguageSurface::Full`](crate::LanguageSurface::Full): a first-party script
/// there already has `io` and `os`, so withholding this would cost it a legitimate technique and
/// deny it nothing it could not do anyway.
fn protect_string_metatable(lua: &mlua::Lua) -> Result<()> {
    let fail = |source: mlua::Error| Error::EngineSetup {
        stage: "protecting the string metatable",
        source: Box::new(source),
    };

    // `lua.load` is the Rust-side loader and is unaffected by withholding the `load` global.
    lua.load("local mt = getmetatable(''); if mt then mt.__metatable = false end")
        .exec()
        .map_err(fail)
}

/// Creates the root table and installs every module into it.
///
/// Every module is told the policy it is being installed under, so a module that guards an
/// operation reads its authority from the same place [`crate::Engine::policy`] reports it from.
fn install_modules(
    lua: &mlua::Lua,
    modules: &ModuleSet,
    policy: &Policy,
    root_table: &RootTable,
) -> Result<()> {
    let fail = |e: mlua::Error| Error::ModuleInstall {
        module: root_table.to_string(),
        reason: e.to_string(),
    };

    let context = InstallContext::new(policy, root_table);
    let root = lua.create_table().map_err(fail)?;
    for module in modules.iter() {
        let table = lua.create_table().map_err(fail)?;
        module.install(lua, &table, &context)?;
        root.set(module.name().as_str(), table).map_err(fail)?;
    }
    lua.globals().set(root_table.as_str(), root).map_err(fail)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use crate::modules::ModuleSet;
    use crate::{Engine, LanguageSurface, Policy, Script};

    fn probe(policy: Policy, source: &str) -> String {
        let engine = Engine::builder().policy(policy).build().unwrap();
        engine
            .eval_to::<String>(&Script::from_source(source, "probe").unwrap())
            .unwrap()
    }

    fn pure_engine() -> Engine {
        Engine::builder().policy(Policy::pure()).build().unwrap()
    }

    fn eval_i64(engine: &Engine, source: &str) -> i64 {
        engine
            .eval_to::<i64>(&Script::from_source(source, "probe").unwrap())
            .unwrap()
    }

    #[test]
    fn a_confined_engine_withholds_the_chunk_loaders() {
        for name in ["load", "loadstring", "dofile", "loadfile"] {
            let found = probe(Policy::confined(), &format!("return type({name})"));
            assert_eq!(found, "nil", "{name} should be withheld");
        }
    }

    #[test]
    fn a_confined_engine_withholds_io_and_debug_entirely() {
        for name in ["io", "debug", "package"] {
            assert_eq!(
                probe(Policy::confined(), &format!("return type({name})")),
                "nil"
            );
        }
    }

    #[test]
    fn a_confined_engine_withholds_the_unsafe_os_functions() {
        for name in [
            "execute",
            "exit",
            "getenv",
            "remove",
            "rename",
            "tmpname",
            "setlocale",
        ] {
            let found = probe(Policy::confined(), &format!("return type(os.{name})"));
            assert_eq!(found, "nil", "os.{name} should be withheld");
        }
    }

    #[test]
    fn a_confined_engine_keeps_the_pure_os_functions() {
        for name in ["time", "date", "clock", "difftime"] {
            let found = probe(Policy::confined(), &format!("return type(os.{name})"));
            assert_eq!(found, "function", "os.{name} should be available");
        }
    }

    #[test]
    fn a_confined_engine_keeps_string_table_and_math() {
        for name in ["string", "table", "math"] {
            assert_eq!(
                probe(Policy::confined(), &format!("return type({name})")),
                "table"
            );
        }
    }

    #[test]
    fn a_trusted_engine_exposes_io() {
        assert_eq!(probe(Policy::trusted(), "return type(io)"), "table");
    }

    #[test]
    fn a_pure_engine_withholds_print() {
        assert_eq!(probe(Policy::pure(), "return type(print)"), "nil");

        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        let err = engine
            .eval(&Script::from_source("print('x')", "probe").unwrap())
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("attempt to call a nil value (global 'print')"),
            "unexpected error text: {err}"
        );
    }

    #[test]
    fn restricted_keeps_print_and_lua_seeding() {
        for policy in [Policy::confined(), Policy::trusted()] {
            assert_eq!(probe(policy, "return type(print)"), "function");
        }

        let found = probe(
            Policy::confined(),
            "local a, b = math.randomseed()
             return math.type(a) .. '/' .. math.type(b)",
        );
        assert_eq!(found, "integer/integer");
    }

    #[test]
    fn minimal_random_is_the_same_sequence_on_every_fresh_engine() {
        for _ in 0..2 {
            assert_eq!(
                eval_i64(&pure_engine(), "return math.random(1, 1000000)"),
                275_394
            );
        }
    }

    #[test]
    fn minimal_random_continues_across_evaluations_on_one_engine() {
        let engine = pure_engine();
        assert_eq!(eval_i64(&engine, "return math.random(1, 1000000)"), 275_394);
        assert_eq!(eval_i64(&engine, "return math.random(1, 1000000)"), 909_833);
    }

    #[test]
    fn minimal_refuses_randomseed_without_a_seed() {
        let err = pure_engine()
            .eval(&Script::from_source("math.randomseed()", "probe").unwrap())
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(
                "math.randomseed denied: a seed is required — the no-argument form reads the clock"
            ),
            "unexpected error text: {err}"
        );
    }

    #[test]
    fn minimal_randomseed_with_a_seed_repeats() {
        let engine = pure_engine();
        let found = engine
            .eval_to::<String>(
                &Script::from_source(
                    "local a, b = math.randomseed(42)
                     local first = math.random(1, 1000000)
                     math.randomseed(42)
                     local second = math.random(1, 1000000)
                     return a .. ',' .. b .. ',' .. tostring(first == second)",
                    "probe",
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(found, "42,0,true");
    }

    #[test]
    fn minimal_randomseed_passes_lua_s_own_argument_error_through() {
        let err = pure_engine()
            .eval(&Script::from_source("math.randomseed(nil)", "probe").unwrap())
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("bad argument #1 to '?' (number expected, got nil)"),
            "unexpected error text: {err}"
        );
    }

    #[test]
    fn a_pure_engine_withholds_os_and_coroutine_as_well() {
        for name in ["os", "coroutine"] {
            assert_eq!(
                probe(Policy::pure(), &format!("return type({name})")),
                "nil"
            );
        }
    }

    #[test]
    fn every_preset_can_count_characters_rather_than_only_bytes() {
        // Without `utf8` a script has `#s`, which counts bytes: "café" is 5 bytes and 4
        // characters, and truncating on the byte count cuts through the middle of one.
        for policy in [Policy::trusted(), Policy::confined(), Policy::pure()] {
            let found = probe(policy, "return utf8.len('café') .. '/' .. #'café'");
            assert_eq!(found, "4/5");
        }
    }

    #[test]
    fn utf8_is_reachable_on_every_surface() {
        for policy in [Policy::trusted(), Policy::confined(), Policy::pure()] {
            assert_eq!(probe(policy, "return type(utf8)"), "table");
        }
    }

    #[test]
    fn a_confined_engine_can_iterate_codepoints() {
        let found = probe(
            Policy::confined(),
            "local out = {}
             for _, c in utf8.codes('café') do out[#out+1] = c end
             return table.concat(out, ',')",
        );
        assert_eq!(found, "99,97,102,233");
    }

    #[test]
    fn a_pure_engine_still_has_the_pure_computation_libraries() {
        for name in ["string", "table", "math"] {
            assert_eq!(
                probe(Policy::pure(), &format!("return type({name})")),
                "table"
            );
        }
    }

    #[test]
    fn a_confined_engine_hides_the_string_metatable() {
        assert_eq!(
            probe(Policy::confined(), "return type(getmetatable(''))"),
            "boolean"
        );
    }

    #[test]
    fn a_confined_script_cannot_replace_a_string_method_for_the_next_script() {
        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();

        let attack = Script::from_source(
            "return pcall(function() getmetatable('').__index.upper = function() return 'PWNED' end end)",
            "attacker",
        )
        .unwrap();
        assert!(
            !engine.eval_to::<bool>(&attack).unwrap(),
            "the script reached the string metatable"
        );

        // The victim reads no global and calls no host module — it would have no way to notice.
        let victim = Script::from_source("return ('hello'):upper()", "victim").unwrap();
        assert_eq!(engine.eval_to::<String>(&victim).unwrap(), "HELLO");
    }

    #[test]
    fn method_calls_on_strings_still_work_under_a_hidden_metatable() {
        assert_eq!(probe(Policy::confined(), "return ('ab'):rep(2)"), "abab");
    }

    #[test]
    fn a_trusted_engine_leaves_the_string_metatable_reachable() {
        assert_eq!(
            probe(Policy::trusted(), "return type(getmetatable(''))"),
            "table"
        );
    }

    #[test]
    fn a_memory_ceiling_is_armed_before_any_script_runs() {
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        let script =
            Script::from_source("local t = {} for i = 1, 1e9 do t[i] = i end", "greedy").unwrap();
        assert!(engine.eval(&script).is_err());
    }

    #[test]
    fn an_empty_module_set_still_creates_the_root_table() {
        let engine = Engine::builder()
            .policy(Policy::confined())
            .stdlib(ModuleSet::new())
            .build()
            .unwrap();
        let found = engine
            .eval_to::<String>(&Script::from_source("return type(airsstack)", "probe").unwrap())
            .unwrap();
        assert_eq!(found, "table");
        assert!(engine.module_names().is_empty());
    }

    #[test]
    fn the_engine_keeps_the_policy_it_was_built_with() {
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        assert_eq!(engine.policy().language(), LanguageSurface::Minimal);
    }
}
