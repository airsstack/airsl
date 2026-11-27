// Probe 238: how a member manifest's `[package] version` reads through `toml::Value` (toml 1.2.2, the
// workspace's features), and which §5.6.3 message the classification below gives each shape.
use toml::Value;

fn classify(text: &str) -> String {
    let manifest: Value = match toml::from_str(text) {
        Ok(v) => v,
        Err(e) => return format!("is not valid TOML: {}", e.to_string().lines().next().unwrap_or("")),
    };
    let Some(package) = manifest.get("package").and_then(Value::as_table) else {
        return format!("NO-PACKAGE (manifest.get(\"package\") = {:?})", manifest.get("package").map(Value::type_str));
    };
    let Some(version) = package.get("version") else {
        return "NO-VERSION".to_owned();
    };
    let inherits = version
        .as_table()
        .is_some_and(|t| t.len() == 1 && t.get("workspace").and_then(Value::as_bool) == Some(true));
    if inherits {
        return "ok: inherits".to_owned();
    }
    match version {
        Value::String(s) => format!("SETS-OWN (string {s:?})"),
        other => format!("WRONG-TYPE (type_str = {:?}, value = {:?})", other.type_str(), other),
    }
}

fn main() {
    let cases: [(&str, &str); 14] = [
        ("dotted key", "[package]\nname = \"airsl\"\nversion.workspace = true\n"),
        ("inline table", "[package]\nname = \"airsl\"\nversion = { workspace = true }\n"),
        ("string", "[package]\nname = \"airsl\"\nversion = \"0.2.0\"\n"),
        ("empty string", "[package]\nname = \"airsl\"\nversion = \"\"\n"),
        ("no version key", "[package]\nname = \"airsl\"\n"),
        ("no [package]", "[lib]\nname = \"airsl\"\n"),
        ("empty file", ""),
        ("package is a string", "package = \"airsl\"\n"),
        ("integer", "[package]\nversion = 1\n"),
        ("float", "[package]\nversion = 0.2\n"),
        ("workspace = false", "[package]\nversion.workspace = false\n"),
        ("extra key", "[package]\nversion = { workspace = true, path = \"x\" }\n"),
        ("array", "[package]\nversion = [\"0.2.0\"]\n"),
        ("date", "[package]\nversion = 2026-11-26\n"),
    ];
    for (label, text) in cases {
        println!("{label:<20} -> {}", classify(text));
    }
    println!("{:<20} -> {}", "boolean", classify("[package]\nversion = true\n"));
    println!("{:<20} -> {}", "invalid TOML", classify("[package\n"));
}
