// Throwaway probe: one question per mode, against the state airsl's Minimal surface builds.
use mlua::{Lua, LuaOptions, StdLib, Value};

fn minimal() -> Lua {
    Lua::new_with(StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8, LuaOptions::default()).unwrap()
}

fn main() {
    let mode = std::env::args().nth(1).unwrap();
    let lua = minimal();
    let src = match mode.as_str() {
        // Q1: initial seed of a fresh state varies per process.
        "fresh-random" => "return tostring(math.random(1, 1000000))",
        // Q2: seeding from Rust right after creation makes math.random repeatable.
        "seeded-random" => {
            lua.load("math.randomseed(0)").exec().unwrap();
            "return tostring(math.random(1, 1000000))"
        }
        // Q3: no-argument randomseed returns the wall clock as its first value.
        "randomseed-noarg" => "local a, b = math.randomseed(); return a .. ' ' .. b",
        // Q4: removing print from the globals table leaves nothing callable.
        "print-removed" => {
            lua.globals().set("print", Value::Nil).unwrap();
            "return type(print) .. ' ' .. tostring(pcall(function() print('x') end))"
        }
        // Q5: collectgarbage('count') across processes.
        "gc-count" => "local t = {} for i = 1, 1000 do t[i] = tostring(i) end return string.format('%.4f', collectgarbage('count'))",
        // Q6: string.format %p prints an address.
        "format-p" => "return string.format('%p', {})",
        // Q7: warn writes nothing on an mlua-created state.
        "warn" => "warn('@on') warn('probe warn text') return 'returned'",
        // Q8: Lua release.
        "version" => "return _VERSION",
        _ => panic!("unknown mode"),
    };
    let out: String = lua.load(src).eval().unwrap();
    println!("{out}");
}
