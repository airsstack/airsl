// Throwaway probe: (a) the text a script sees calling a withheld `print`; (b) whether a seeded
// generator continues its sequence across two evaluations on one state.
use mlua::{Lua, LuaOptions, StdLib, Value};

fn main() {
    let lua = Lua::new_with(StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8, LuaOptions::default()).unwrap();
    lua.globals().set("print", Value::Nil).unwrap();
    lua.load("math.randomseed(0)").exec().unwrap();
    let err = lua.load("print('x')").set_name("@printnil.lua").exec().unwrap_err();
    println!("print error: {}", err.to_string().lines().next().unwrap());
    let a: i64 = lua.load("return math.random(1, 1000000)").eval().unwrap();
    let b: i64 = lua.load("return math.random(1, 1000000)").eval().unwrap();
    println!("eval 1: {a}\neval 2: {b}");
}
