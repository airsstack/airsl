//! A4-A6 probe: a loadable `airsstack` module.

use mlua::{Lua, Result as LuaResult, Table as LuaTable};

#[mlua::lua_module]
fn airsstack(lua: &Lua) -> LuaResult<LuaTable> {
    let table = lua.create_table()?;

    // A5: what set_memory_limit does when the state belongs to the host interpreter.
    let limit = match lua.set_memory_limit(1 << 20) {
        Ok(previous) => format!("Ok({previous})"),
        Err(e) => format!("Err(Display={e}; Debug={e:?})"),
    };
    table.set("memory_limit_result", limit)?;

    // A6: airsl's exact encode path (crates/airsl/src/convert.rs `sorted` + `to_json`).
    let encode = lua.create_function(|_, value: mlua::Value| {
        serde_json::to_string(&value.to_serializable().sort_keys(true))
            .map_err(mlua::Error::external)
    })?;
    table.set("encode", encode)?;
    Ok(table)
}
