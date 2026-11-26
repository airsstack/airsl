//! Stand-in for `airsl-lua`: the loadable `airsstack` module.

use mlua::{Lua, Result as LuaResult, Table as LuaTable};

/// The `luaopen_airsstack` entry point.
///
/// ```
/// assert_eq!(1 + 1, 2);
/// ```
#[mlua::lua_module]
fn airsstack(lua: &Lua) -> LuaResult<LuaTable> {
    hostlib::root(lua)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_test_in_a_module_mode_crate_runs() {
        assert_eq!(2 + 2, 4);
    }
}
