use airsl::mlua::{Lua, Value};
fn main() {
    // serde_json on bytes
    for (name, bytes) in [
        ("bom", b"\xEF\xBB\xBF{\"a\":1}".to_vec()),
        ("bom stripped", b"{\"a\":1}".to_vec()),
        ("not utf-8", b"{\"a\":\"\xFF\"}".to_vec()),
        ("comment (from_slice)", b"{\n  // keep me\n  \"workspace.library\": []\n}".to_vec()),
        ("trailing comma (from_slice)", b"{\"a\": 1,}".to_vec()),
        ("non-object with trailing comma (from_slice)", b"[1, 2,]".to_vec()),
    ] {
        match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(v) => println!("{name}: ok -> {v}"),
            Err(e) => println!("{name}: err -> {e}"),
        }
    }
    println!("L1 bytes: {:?}", serde_json::to_string_pretty(&serde_json::json!({"workspace.library": [".airsl/types"]})).unwrap() + "\n");
    // mlua Table::pairs vs metatables
    let lua = Lua::new();
    let t: airsl::mlua::Table = lua.load(r#"
        local t = { real = 1 }
        return setmetatable(t, {
            __index = { hidden = 2 },
            __pairs = function(tt) return next, { fake = 3 }, nil end,
        })
    "#).eval().unwrap();
    let mut keys: Vec<String> = t.pairs::<Value, Value>().map(|p| p.unwrap().0.to_string().unwrap()).collect();
    keys.sort();
    println!("Table::pairs keys: {keys:?}");
    let lua_pairs: String = lua.load(r#"local t = ... ; local ks = {} ; for k in pairs(t) do ks[#ks+1] = k end ; return table.concat(ks, ",")"#).call(&t).unwrap();
    println!("Lua pairs() keys: {lua_pairs}");
    let hidden: Value = t.get("hidden").unwrap();
    let raw_hidden: Value = t.raw_get("hidden").unwrap();
    println!("Table::get(hidden) = {:?}, raw_get(hidden) = {:?}", hidden.type_name(), raw_hidden.type_name());
    let tk: Value = lua.load("return {}").eval().unwrap();
    println!("table key to_string: {:?}", tk.to_string().unwrap());
}
