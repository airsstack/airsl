//! Stand-in for `airsl-cli`: an executable on the default (vendored, send) features.

fn main() -> hostlib::mlua::Result<()> {
    let lua = hostlib::mlua::Lua::new();
    let root = hostlib::root(&lua)?;
    let version: String = root.get("version")?;
    println!("cli sees version {version}");
    Ok(())
}
