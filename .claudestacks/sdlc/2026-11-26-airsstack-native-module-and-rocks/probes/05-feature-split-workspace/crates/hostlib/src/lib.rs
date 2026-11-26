//! Stand-in for `airsl`: re-exports mlua and fills a table.

pub use mlua;

/// Builds the table the native module returns.
///
/// # Errors
///
/// Returns the mlua error when the table cannot be built.
pub fn root(lua: &mlua::Lua) -> mlua::Result<mlua::Table> {
    let table = lua.create_table()?;
    table.set("version", "0.0.0")?;
    Ok(table)
}
