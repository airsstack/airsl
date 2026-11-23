use mlua::{Function, Lua, Value};

const NAME: &str = "@m.lua";

fn first_line(e: &mlua::Error) -> String {
    e.to_string().lines().next().unwrap_or("").to_owned()
}

fn show(label: &str, r: mlua::Result<Value>) {
    match r {
        Ok(Value::Table(t)) => println!("{label}: Ok(table v={:?})", t.get::<Value>("v")),
        Ok(v) => println!("{label}: Ok({v:?})"),
        Err(e) => println!("{label}: Err({})", first_line(&e)),
    }
}

// Approach A: eval's order (expression first, then block), called with the module name.
fn expression_first(lua: &Lua, src: &str, name: &str) -> mlua::Result<Value> {
    let f: Function = match lua.load(format!("return {src}")).set_name(NAME).into_function() {
        Ok(f) => f,
        Err(_) => lua.load(src).set_name(NAME).into_function()?,
    };
    f.call::<Value>((name, "/abs/m.lua"))
}

// Approach B: block first, expression only when the block does not compile.
fn block_first(lua: &Lua, src: &str, name: &str) -> mlua::Result<Value> {
    let f: Function = match lua.load(src).set_name(NAME).into_function() {
        Ok(f) => f,
        Err(_) => lua.load(format!("return {src}")).set_name(NAME).into_function()?,
    };
    f.call::<Value>((name, "/abs/m.lua"))
}

fn main() {
    let lua = Lua::new();
    let cases = [
        ("block returning ...", "return (...)"),
        ("bare table expression", "{ v = 'bare' }"),
        ("bare ... expression", "..."),
        ("both expression and statement", "tostring(1)"),
        ("argument count", "return select('#', ...)"),
        ("syntax error", "local x = \n"),
        ("two return values", "return { v = 1 }, 'extra'"),
    ];
    for (label, src) in cases {
        println!("--- {label}: {src:?}");
        show("  eval (0.1.4)      ", lua.load(src).set_name(NAME).eval::<Value>());
        show("  call (issue's fix)", lua.load(src).set_name(NAME).call::<Value>("pkg.mod"));
        show("  A expression-first", expression_first(&lua, src, "pkg.mod"));
        show("  B block-first     ", block_first(&lua, src, "pkg.mod"));
    }

    println!("--- bytecode header as UTF-8");
    let dump: mlua::String = lua.load("return string.dump(function() end)").eval().unwrap_or_else(|e| panic!("{e}"));
    let bytes = dump.as_bytes().to_vec();
    println!("  first 12 bytes: {:02x?}", &bytes[..12.min(bytes.len())]);
    println!("  String::from_utf8: {:?}", String::from_utf8(bytes).map(|_| ()).map_err(|e| e.to_string()));
}
