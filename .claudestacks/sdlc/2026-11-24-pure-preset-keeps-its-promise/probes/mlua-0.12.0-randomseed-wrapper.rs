// Throwaway probe: can Rust wrap Lua's own math.randomseed, refusing only the no-argument form?
use mlua::{Function, Lua, LuaOptions, MultiValue, StdLib, Table};

fn main() {
    let lua = Lua::new_with(StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8, LuaOptions::default()).unwrap();
    let math: Table = lua.globals().get("math").unwrap();
    let original: Function = math.get("randomseed").unwrap();
    original.call::<()>(0).unwrap();
    let wrapped = lua
        .create_function(move |_, args: MultiValue| {
            if args.is_empty() {
                return Err(mlua::Error::runtime("no-argument form refused"));
            }
            original.call::<MultiValue>(args)
        })
        .unwrap();
    math.set("randomseed", wrapped).unwrap();
    let out: String = lua
        .load(r#"
            local first = math.random(1, 1000000)
            local ok, err = pcall(math.randomseed)
            local a, b = math.randomseed(42)
            local x = math.random(1, 1000000)
            math.randomseed(42)
            local y = math.random(1, 1000000)
            local okn, errn = pcall(math.randomseed, nil)
            return table.concat({ "first=" .. first, "noarg=" .. tostring(ok) .. " " .. tostring(err),
              "returns=" .. tostring(a) .. "," .. tostring(b), "repeat=" .. x .. "," .. y,
              "nil-arg=" .. tostring(okn) .. " " .. tostring(errn) }, "\n")
        "#)
        .eval()
        .unwrap();
    println!("{out}");
}
