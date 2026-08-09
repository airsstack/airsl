//! Type-state construction of an [`Engine`].
//!
//! A builder rather than a constructor because building an engine has one decision that must not
//! be defaulted silently — the sandbox policy — and several that should be. The type-state
//! parameter makes the required one a compile error rather than a runtime check: there is no
//! `build` method until [`EngineBuilder::sandbox`] has been called.
//!
//! Responsibilities:
//!
//! - [`EngineBuilder`] and the [`Missing`] / [`Present`] state markers.
//! - Applying the sandbox to a fresh [`mlua::Lua`] and installing the host modules.
//!
//! Non-responsibilities: running scripts. That is [`Engine`]'s job.

use mlua::{StdLib, Table};

use crate::engine::{Engine, ROOT_TABLE};
use crate::error::{Error, Result};
use crate::modules::{ModuleSet, stdlib};
use crate::policy::Sandbox;

/// Type-state marker: the sandbox policy has not been chosen yet.
#[derive(Debug)]
pub struct Missing;

/// Type-state marker: the sandbox policy has been chosen.
#[derive(Debug)]
pub struct Present;

/// Base-library globals that hand a script a second way to load code, bypassing the sandbox and
/// the confined `require`. Removed under [`Sandbox::Restricted`].
const CHUNK_LOADERS: [&str; 4] = ["load", "loadstring", "dofile", "loadfile"];

/// `os` functions that reach outside the process or change how the process behaves. Every
/// capability they provide is available through a host module instead, where the host controls it.
///
/// `setlocale` is on the list for a subtler reason than the rest: Lua compares strings with
/// `strcoll`, so a script that changes the locale changes the sort order of every subsequent
/// `table.sort` — which would silently break output the tooling asserts byte-for-byte.
const UNSAFE_OS_FUNCTIONS: [&str; 7] = [
    "execute",
    "exit",
    "getenv",
    "remove",
    "rename",
    "tmpname",
    "setlocale",
];

/// Configures and builds an [`Engine`].
#[derive(Debug)]
pub struct EngineBuilder<S> {
    sandbox: Sandbox,
    modules: Option<ModuleSet>,
    state: core::marker::PhantomData<S>,
}

impl EngineBuilder<Missing> {
    /// Starts with no sandbox policy chosen.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            sandbox: Sandbox::Restricted,
            modules: None,
            state: core::marker::PhantomData,
        }
    }

    /// Chooses what Lua standard libraries scripts may reach.
    ///
    /// This is the one required step; [`EngineBuilder::build`] does not exist until it is called.
    #[must_use]
    pub fn sandbox(self, policy: Sandbox) -> EngineBuilder<Present> {
        EngineBuilder {
            sandbox: policy,
            modules: self.modules,
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

    /// Builds the engine: creates the Lua state, applies the sandbox, and installs the modules.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ModuleInstall`] when the root table cannot be created or a host module
    /// fails to install.
    pub fn build(self) -> Result<Engine> {
        let libs = match self.sandbox {
            Sandbox::Restricted => {
                StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::COROUTINE | StdLib::OS
            }
            Sandbox::Full => StdLib::ALL_SAFE,
        };
        let lua = mlua::Lua::new_with(libs, mlua::LuaOptions::default()).map_err(|e| {
            Error::ModuleInstall {
                module: String::from("<lua state>"),
                reason: e.to_string(),
            }
        })?;

        if self.sandbox.is_restricted() {
            restrict_globals(&lua)?;
        }

        let modules = match self.modules {
            Some(set) => set,
            None => stdlib()?,
        };
        install_modules(&lua, &modules)?;

        Ok(Engine::from_parts(lua, modules))
    }
}

/// Removes the chunk loaders and the unsafe `os` functions from the globals table.
fn restrict_globals(lua: &mlua::Lua) -> Result<()> {
    let fail = |e: mlua::Error| Error::ModuleInstall {
        module: String::from("<sandbox>"),
        reason: e.to_string(),
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

/// Creates the `airsstack` root table and installs every module into it.
fn install_modules(lua: &mlua::Lua, modules: &ModuleSet) -> Result<()> {
    let fail = |e: mlua::Error| Error::ModuleInstall {
        module: String::from(ROOT_TABLE),
        reason: e.to_string(),
    };

    let root = lua.create_table().map_err(fail)?;
    for module in modules.iter() {
        let table = lua.create_table().map_err(fail)?;
        module.install(lua, &table)?;
        root.set(module.name().as_str(), table).map_err(fail)?;
    }
    lua.globals().set(ROOT_TABLE, root).map_err(fail)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use crate::modules::ModuleSet;
    use crate::{Engine, Sandbox, Script};

    fn probe(sandbox: Sandbox, source: &str) -> String {
        let engine = Engine::builder().sandbox(sandbox).build().unwrap();
        engine
            .eval_to::<String>(&Script::from_source(source, "probe").unwrap())
            .unwrap()
    }

    #[test]
    fn a_restricted_engine_withholds_the_chunk_loaders() {
        for name in ["load", "loadstring", "dofile", "loadfile"] {
            let found = probe(Sandbox::Restricted, &format!("return type({name})"));
            assert_eq!(found, "nil", "{name} should be withheld");
        }
    }

    #[test]
    fn a_restricted_engine_withholds_io_and_debug_entirely() {
        for name in ["io", "debug", "package"] {
            assert_eq!(
                probe(Sandbox::Restricted, &format!("return type({name})")),
                "nil"
            );
        }
    }

    #[test]
    fn a_restricted_engine_withholds_the_unsafe_os_functions() {
        for name in [
            "execute",
            "exit",
            "getenv",
            "remove",
            "rename",
            "tmpname",
            "setlocale",
        ] {
            let found = probe(Sandbox::Restricted, &format!("return type(os.{name})"));
            assert_eq!(found, "nil", "os.{name} should be withheld");
        }
    }

    #[test]
    fn a_restricted_engine_keeps_the_pure_os_functions() {
        for name in ["time", "date", "clock", "difftime"] {
            let found = probe(Sandbox::Restricted, &format!("return type(os.{name})"));
            assert_eq!(found, "function", "os.{name} should be available");
        }
    }

    #[test]
    fn a_restricted_engine_keeps_string_table_and_math() {
        for name in ["string", "table", "math"] {
            assert_eq!(
                probe(Sandbox::Restricted, &format!("return type({name})")),
                "table"
            );
        }
    }

    #[test]
    fn a_full_engine_exposes_io() {
        assert_eq!(probe(Sandbox::Full, "return type(io)"), "table");
    }

    #[test]
    fn an_empty_module_set_still_creates_the_root_table() {
        let engine = Engine::builder()
            .sandbox(Sandbox::Restricted)
            .stdlib(ModuleSet::new())
            .build()
            .unwrap();
        let found = engine
            .eval_to::<String>(&Script::from_source("return type(airsstack)", "probe").unwrap())
            .unwrap();
        assert_eq!(found, "table");
        assert!(engine.module_names().is_empty());
    }
}
