//! Toy native module: `require("airsstack")` returns a table.
use mlua::prelude::*;

#[mlua::lua_module]
fn airsstack(lua: &Lua) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("answer", airsstack_core::answer())?;
    t.set("jit", cfg!(feature = "luajit"))?;
    Ok(t)
}
