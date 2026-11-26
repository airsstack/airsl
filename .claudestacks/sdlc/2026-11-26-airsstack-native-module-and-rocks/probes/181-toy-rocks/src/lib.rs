use mlua::prelude::*;

// Probe M toy: same as probe 66, plus a counter and the crate version so a
// probe can tell which build and which call produced a table.
static OPENS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[mlua::lua_module]
fn airsstack(lua: &Lua) -> LuaResult<LuaTable> {
    let n = OPENS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let t = lua.create_table()?;
    t.set("ok", true)?;
    t.set("version", env!("CARGO_PKG_VERSION"))?;
    t.set("opens", n)?;
    Ok(t)
}
