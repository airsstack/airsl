//! The `luaopen_airsstack` entry point (probe 230: the spec's §3.2 one-expression form).

use airsl::mlua::{self, Lua, Result, Table};
use airsl::types::RootTable;

/// Builds the `airsstack` module table: every built-in except `ext`, trusted authority.
#[mlua::lua_module]
fn airsstack(lua: &Lua) -> Result<Table> {
    Ok(airsl::modules::standalone()?.open(lua, &RootTable::default())?)
}
