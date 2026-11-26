//! Variant (k2): as (k1), plus `use hostlib::mlua::{self, ..}` so the expansion finds `mlua`.

use hostlib::mlua::{self, Lua, Result as LuaResult, Table as LuaTable};

/// The `luaopen_airsstack` entry point.
#[hostlib::mlua::lua_module]
fn airsstack(lua: &Lua) -> LuaResult<LuaTable> {
    hostlib::root(lua)
}
