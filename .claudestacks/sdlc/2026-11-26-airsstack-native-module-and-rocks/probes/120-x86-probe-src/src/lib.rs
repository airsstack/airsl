//! X probe: a minimal mlua 0.12.0 module (lua54 or luajit) for x86_64 Linux/macOS and luarocks packing.
use mlua::prelude::*;

#[mlua::lua_module]
fn airsstack(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;
    exports.set("ok", true)?;
    let seq = lua.create_sequence_from([LuaValue::Integer(2), LuaValue::Number(2.0)])?;
    let json = serde_json::to_string(&LuaValue::Table(seq)).map_err(LuaError::external)?;
    exports.set("json", json)?;
    Ok(exports)
}
