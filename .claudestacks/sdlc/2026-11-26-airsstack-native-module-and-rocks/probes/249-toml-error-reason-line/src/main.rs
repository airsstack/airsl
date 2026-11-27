// Probe 249: the `{toml error}` of docs-check's "is not valid TOML" message in form B: the first line of toml 1.2.2's error
// Display, then ": ", then the reason line. The rule probed (`toml_error` below): split the Display with `str::lines`; the
// first line is kept; the reason is the last line that is not empty, looked for after the first line only; with no such
// line (a Display of one line) `{toml error}` is the first line alone. The cases are probe 244's six, then kinds chosen to
// break the rule: a CRLF file, an error with no span (toml's key recursion limit, 100 dotted parts), nesting past the
// parser's depth limit, a date-time error (its text comes from toml_datetime), a literal newline in the expected list,
// a bare carriage return, an invalid escape, an error at the end of a file with no final newline, a dotted key that
// extends a value.
fn toml_error(e: &toml::de::Error) -> String {
    let text = e.to_string();
    let mut lines = text.lines();
    let first = lines.next().unwrap_or("");
    match lines.filter(|l| !l.is_empty()).last() {
        Some(reason) => format!("{first}: {reason}"),
        None => first.to_owned(),
    }
}

fn main() {
    let deep_key = format!("[package]\n{} = 1\n", vec!["a"; 100].join("."));
    let deep_array = format!("x = {}{}\n", "[".repeat(100), "]".repeat(100));
    let cases: Vec<(&str, String)> = vec![
        ("unclosed header", "[package\n".to_owned()),
        ("missing value", "[package]\nname =\n".to_owned()),
        ("duplicate key", "[package]\nname = \"a\"\nname = \"b\"\n".to_owned()),
        ("unterminated string", "[package]\nname = \"airsl\nversion.workspace = true\n".to_owned()),
        ("error on line 3", "[package]\nname = \"airsl\"\nversion.workspace = = true\n".to_owned()),
        ("duplicate table", "[package]\n[package]\n".to_owned()),
        ("crlf unclosed header", "[package\r\nname = \"a\"\r\n".to_owned()),
        ("dotted key of 100 parts", deep_key),
        ("arrays nested 100 deep", deep_array),
        ("invalid date-time", "[package]\ndate = 2026-13-45\n".to_owned()),
        ("two values on one line", "[package]\nname = \"a\" version = \"b\"\n".to_owned()),
        ("bare carriage return", "[package]\nname = \"a\"\r\r\n".to_owned()),
        ("invalid escape", "[package]\nname = \"\\q\"\n".to_owned()),
        ("end of file, no newline", "[package]\nname =".to_owned()),
        ("dotted key extends a value", "[package]\nversion = 1\nversion.workspace = true\n".to_owned()),
    ];
    for (label, text) in &cases {
        match toml::from_str::<toml::Value>(text) {
            Ok(v) => println!("{label}: parsed?! {v:?}"),
            Err(e) => {
                let full = e.to_string();
                let cut = toml_error(&e);
                println!("{label}: Display has {} lines; full = {full:?}", full.lines().count());
                println!("{label}: message = {:?}", format!("crates/airsl/Cargo.toml: is not valid TOML: {cut}"));
            }
        }
    }
}
