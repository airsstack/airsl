//! The `luaopen_airsstack` entry point (probe toy).

use airsl::mlua::{self, Lua, Result, Table};
use airsl::modules::{Json, ModuleSet};
use airsl::types::RootTable;

/// Builds the `airsstack` module table: the `json` host module, installed through
/// `ModuleSet::open`, plus the library version.
#[mlua::lua_module]
fn airsstack(lua: &Lua) -> Result<Table> {
    let mut set = ModuleSet::new();
    set.insert(Box::new(Json::new())).map_err(mlua::Error::external)?;
    let root = set
        .open(lua, &RootTable::default())
        .map_err(mlua::Error::external)?;
    root.set("version", airsl::VERSION)?;
    Ok(root)
}
