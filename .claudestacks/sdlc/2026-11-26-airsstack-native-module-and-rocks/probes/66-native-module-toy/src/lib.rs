use mlua::prelude::*;

#[mlua::lua_module]
fn airsstack(lua: &Lua) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("ok", true)?;
    Ok(t)
}
