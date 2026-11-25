use airsl::mlua::{Lua, Value};
fn main() {
    let lua = Lua::new();
    let vals: Vec<Value> = lua.load(r#"return "s", 1, 1.5, true, {}, print, nil"#).eval::<airsl::mlua::MultiValue>().unwrap().into_iter().collect();
    for v in vals { println!("{}", v.type_name()); }
    let t = lua.create_table().unwrap();
    t.set(1, "x").unwrap(); t.set("k", 2).unwrap();
    for pair in t.pairs::<Value, Value>() { let (k, _) = pair.unwrap(); println!("key {:?} type {}", k.to_string().unwrap(), k.type_name()); }
}
