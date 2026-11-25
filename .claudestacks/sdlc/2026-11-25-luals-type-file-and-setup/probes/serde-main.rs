fn main() {
    let cases = [
        ("comment", "{\n  // keep me\n  \"workspace.library\": []\n}"),
        ("trailing comma", "{\"a\": 1,}"),
        ("not object", "[1, 2]"),
        ("empty file", ""),
        ("whitespace only", "  \n "),
        ("duplicate key", "{\"a\": 1, \"a\": 2}"),
        ("order", "{\"z\": 1, \"runtime.version\": \"Lua 5.4\", \"a\": {\"y\": 1, \"b\": 2}}"),
    ];
    for (name, text) in cases {
        match serde_json::from_str::<serde_json::Value>(text) {
            Ok(v) => println!("{name}: ok -> {}", serde_json::to_string_pretty(&v).unwrap()),
            Err(e) => println!("{name}: err -> {e}"),
        }
    }
}
