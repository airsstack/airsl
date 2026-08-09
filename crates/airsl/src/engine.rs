//! The Lua state, configured and ready to run scripts.
//!
//! This is the crate's entry point. It exists as a distinct type from the builder so that a
//! configured engine is immutable in the ways that matter — the sandbox has been applied and the
//! host modules installed before any caller can hold one, which removes the "did I remember to
//! sandbox it" question from every call site.
//!
//! Responsibilities:
//!
//! - [`Engine`], owning the [`mlua::Lua`] state and the installed [`crate::ModuleSet`].
//! - Evaluating a [`Script`], with and without a typed return value.
//!
//! Non-responsibilities: deciding what to do about a failure. [`Engine::eval`] returns a
//! [`Result`]; [`crate::FailurePolicy`] describes how the caller should treat it.

use mlua::FromLuaMulti;

use crate::builder::{EngineBuilder, Missing};
use crate::error::{Error, Result};
use crate::modules::ModuleSet;
use crate::script::Script;
use crate::types::ModuleName;

/// Name of the single global table every host module is installed under.
pub const ROOT_TABLE: &str = "airsstack";

/// A configured Lua state.
///
/// Build one with [`Engine::builder`]. The sandbox policy is required, so an engine cannot be
/// constructed without deciding what scripts are allowed to reach.
///
/// # Examples
///
/// ```
/// use airsl::{Engine, Sandbox, Script};
///
/// let engine = Engine::builder().sandbox(Sandbox::Restricted).build()?;
/// let script = Script::from_source("return airsstack.json.encode({ok = true})", "demo")?;
/// assert_eq!(engine.eval_to::<String>(&script)?, r#"{"ok":true}"#);
/// # Ok::<(), airsl::Error>(())
/// ```
pub struct Engine {
    lua: mlua::Lua,
    modules: ModuleSet,
}

impl Engine {
    /// Starts configuring an engine.
    #[must_use]
    pub const fn builder() -> EngineBuilder<Missing> {
        EngineBuilder::new()
    }

    /// Wraps an already-configured state. Called by [`EngineBuilder::build`].
    pub(crate) const fn from_parts(lua: mlua::Lua, modules: ModuleSet) -> Self {
        Self { lua, modules }
    }

    /// The underlying Lua state, for callers registering their own values after construction.
    #[must_use]
    pub const fn lua(&self) -> &mlua::Lua {
        &self.lua
    }

    /// The names of the installed host modules, in installation order.
    #[must_use]
    pub fn module_names(&self) -> Vec<&ModuleName> {
        self.modules.names()
    }

    /// Runs `script`, discarding whatever it returns.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Lua`] when the chunk fails to compile or raises while running.
    pub fn eval(&self, script: &Script) -> Result<()> {
        self.eval_to::<()>(script)
    }

    /// Runs `script` and converts its return value to `T`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Lua`] when the chunk fails to compile, raises while running, or returns
    /// something that cannot be converted to `T`.
    pub fn eval_to<T: FromLuaMulti>(&self, script: &Script) -> Result<T> {
        self.lua
            .load(script.source())
            .set_name(script.name().as_lua())
            .eval::<T>()
            .map_err(|e| Error::lua(script.name().as_str(), e))
    }
}

impl core::fmt::Debug for Engine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Engine")
            .field("modules", &self.modules)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{Engine, ROOT_TABLE};
    use crate::{Sandbox, Script};

    fn engine() -> Engine {
        Engine::builder()
            .sandbox(Sandbox::Restricted)
            .build()
            .unwrap()
    }

    fn script(source: &str) -> Script {
        Script::from_source(source, "test").unwrap()
    }

    #[test]
    fn the_root_table_is_named_after_the_workspace() {
        assert_eq!(ROOT_TABLE, "airsstack");
    }

    #[test]
    fn eval_runs_a_chunk_for_its_effect() {
        assert!(engine().eval(&script("local x = 1")).is_ok());
    }

    #[test]
    fn eval_to_converts_the_return_value() {
        assert_eq!(
            engine().eval_to::<i64>(&script("return 6 * 7")).unwrap(),
            42
        );
    }

    #[test]
    fn a_syntax_error_is_reported_against_the_chunk_name() {
        let err = engine().eval(&script("this is not lua")).unwrap_err();
        assert!(err.to_string().contains("test"), "{err}");
    }

    #[test]
    fn a_runtime_error_is_returned_not_panicked() {
        let err = engine().eval(&script("error('boom')")).unwrap_err();
        assert!(err.to_string().contains("boom"), "{err}");
    }

    #[test]
    fn the_root_table_is_visible_to_scripts() {
        let found = engine()
            .eval_to::<String>(&script("return type(airsstack)"))
            .unwrap();
        assert_eq!(found, "table");
    }

    #[test]
    fn module_names_reports_the_installed_standard_library() {
        let engine = engine();
        let names: Vec<_> = engine
            .module_names()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(names.contains(&String::from("json")), "{names:?}");
    }
}
