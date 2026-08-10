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
use crate::sandbox::language_surface::{CHUNK_LOADERS, UNSAFE_OS_FUNCTIONS};
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
