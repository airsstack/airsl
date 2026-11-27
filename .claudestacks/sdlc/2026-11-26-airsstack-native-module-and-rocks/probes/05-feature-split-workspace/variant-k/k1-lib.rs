//! Variant (k1): the attribute is reached through the re-export, with no `use` of `mlua`.

use hostlib::mlua::{Lua, Result as LuaResult, Table as LuaTable};

/// The `luaopen_airsstack` entry point.
#[hostlib::mlua::lua_module]
fn airsstack(lua: &Lua) -> LuaResult<LuaTable> {
    hostlib::root(lua)
}
