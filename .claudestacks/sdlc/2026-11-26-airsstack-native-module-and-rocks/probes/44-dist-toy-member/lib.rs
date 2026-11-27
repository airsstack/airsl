//! probe crate
use mlua::prelude::*;

#[mlua::lua_module]
fn airsstack(lua: &Lua) -> LuaResult<LuaTable> {
    lua.create_table()
}
