//! Lua 5.4 `require` facts on mlua 0.12.0 (lua54, vendored, serialize, send) — airsl's feature set.
use mlua::{Lua, LuaOptions, StdLib, Table, Value};

fn eval(lua: &Lua, label: &str, src: &str) {
    let out = lua.load(src).eval::<String>().unwrap_or_else(|e| format!("eval error: {e}"));
    println!("{label:<52} | {out}");
}

fn main() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("airsstack.lua"), "return { from_user_file = true }").unwrap();
    std::fs::write(dir.path().join("lib.lua"), "return { lib = true }").unwrap(); std::fs::write(dir.path().join("lib3.lua"), "return 3").unwrap(); std::fs::write(dir.path().join("lib4.lua"), "return 4").unwrap();
    let path = format!("{}/?.lua", dir.path().display());

    let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default()).unwrap();
    println!("{}", lua.load("return _VERSION").eval::<String>().unwrap());
    lua.globals().get::<Table>("package").unwrap().set("path", path.clone()).unwrap();

    // The root table, installed as a global and seeded into package.loaded from Rust.
    let root = lua.create_table().unwrap();
    root.set("marker", "root").unwrap();
    lua.globals().set("airsstack", root.clone()).unwrap();
    lua.set_named_registry_value("probe.root", root.clone()).unwrap();
    let loaded: Table = lua.globals().get::<Table>("package").unwrap().get("loaded").unwrap();
    loaded.set("airsstack", root.clone()).unwrap();

    eval(&lua, "a: package.loaded hit, user file on package.path", r#"
      local n = select('#', require("airsstack"))
      local v = require("airsstack")
      return "values=" .. n .. " root=" .. tostring(rawequal(v, airsstack)) .. " user=" .. tostring(v.from_user_file)"#);
    eval(&lua, "b: fresh load through Lua's require", r#"
      local a, b = require("lib")
      return "values=" .. select('#', require("lib")) .. " (cached) first-call second value type=" .. type(b)"#);
    eval(&lua, "b2: fresh load, count of values on first call", r#"
      return "values=" .. select('#', require("lib3") ) "#);
    eval(&lua, "j: script clears package.loaded, then requires", r#"
      package.loaded.airsstack = nil
      local v = require("airsstack")
      package.loaded.airsstack = airsstack
      return "root=" .. tostring(rawequal(v, airsstack)) .. " user=" .. tostring(v.from_user_file)"#);

    // c: error text of a missing module, called directly and through a Rust wrapper.
    let original: mlua::Function = lua.globals().get("require").unwrap();
    let wrapped = lua.create_function(move |_, args: mlua::MultiValue| original.call::<mlua::MultiValue>(args)).unwrap();
    lua.globals().set("wrapped_require", wrapped).unwrap();
    eval(&lua, "c1: Lua's require, missing module", r#"local ok, e = pcall(require, "nope") return tostring(e)"#);
    eval(&lua, "c2: Rust wrapper around it, missing module", r#"local ok, e = pcall(wrapped_require, "nope") return tostring(e)"#);
    eval(&lua, "c3: Rust wrapper, values from a fresh load", r#"return "values=" .. select('#', wrapped_require("lib4"))"#);

    // d: a table stored in the registry is the same table after the global is replaced.
    lua.load("airsstack = nil").exec().unwrap();
    let back: Table = lua.named_registry_value("probe.root").unwrap();
    lua.globals().set("from_registry", back).unwrap();
    lua.globals().set("original_root", root).unwrap();
    eval(&lua, "d: registry table after `airsstack = nil`", r#"return "same=" .. tostring(rawequal(from_registry, original_root)) .. " marker=" .. from_registry.marker"#);

    // f: what a Rust function taking mlua::LuaString does with a non-string argument.
    let takes = lua.create_function(|_, s: mlua::LuaString| Ok(s.to_str()?.to_owned())).unwrap();
    lua.globals().set("takes_string", takes).unwrap();
    for arg in ["", "nil", "1", "1.5", "{}", "'airsstack'", "'airsstack', 'x'"] {
        let call = if arg.is_empty() { "pcall(takes_string)".to_owned() } else { format!("pcall(takes_string, {arg})") };
        eval(&lua, &format!("f: takes_string({arg})"), &format!(r#"local ok, e = {call} return tostring(ok) .. " " .. tostring(e)"#));
    }
    // g: what Lua's own require does with the same arguments.
    for arg in ["", "nil", "1", "{}"] {
        let call = if arg.is_empty() { "pcall(require)".to_owned() } else { format!("pcall(require, {arg})") };
        eval(&lua, &format!("g: require({arg})"), &format!(r#"local ok, e = {call} return tostring(ok) .. " " .. tostring(e)"#));
    }
    // h: Lua's own require on a package.loaded hit, with a second argument.
    lua.load("airsstack = original_root").exec().unwrap();
    eval(&lua, "h: require('airsstack', 'x') on a package.loaded hit", r#"
      local n = select('#', require("airsstack", "x"))
      return "values=" .. n .. " root=" .. tostring(rawequal(require("airsstack", "x"), airsstack))"#);
    // k: a script replaces the package.loaded entry instead of clearing it.
    eval(&lua, "k: package.loaded.airsstack = {} then require", r#"
      local mine = {}
      package.loaded.airsstack = mine
      local v = require("airsstack")
      package.loaded.airsstack = airsstack
      return "mine=" .. tostring(rawequal(v, mine))"#);
    let _ = Value::Nil;
}
