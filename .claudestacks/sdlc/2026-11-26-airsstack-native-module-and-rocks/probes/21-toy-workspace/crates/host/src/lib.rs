//! Toy stand-in for `airsl`: mlua with `vendored` and `send`, as the real host links it.
/// Builds a VM, proving the host member compiles.
#[must_use]
pub fn vm() -> mlua::Lua {
    mlua::Lua::new()
}
