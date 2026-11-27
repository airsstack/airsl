//! The `luaopen_airsstack` entry point of the native module.
//!
//! `#[mlua::lua_module]` exports the function below as the C symbol `luaopen_airsstack`, which is
//! what an interpreter calls on `require("airsstack")`. It is its own file so the one expression
//! the module consists of has a single home.
//!
//! No inline tests: a module-mode build cannot link a test that touches Lua, so none can be
//! written here. The logic this expression delegates to, [`airsl::modules::standalone`] and
//! [`airsl::modules::ModuleSet::open`], is unit-tested in `airsl`, and `lua/check.lua` runs the
//! entry point in a real interpreter.
//!
//! Responsibilities: building the module table for the Lua state that loads the library.
//!
//! Non-responsibilities: choosing the modules, which `airsl` does.

use airsl::mlua::{self, Lua, Result, Table};
use airsl::types::RootTable;

/// Builds the `airsstack` module table: every built-in except `ext`, with trusted authority.
#[mlua::lua_module]
fn airsstack(lua: &Lua) -> Result<Table> {
    Ok(airsl::modules::standalone()?.open(lua, &RootTable::default())?)
}
