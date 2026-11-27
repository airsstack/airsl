// Probe 244: what toml 1.2.2's parse error Display holds (how many lines, what the first one is), and what
// "the text up to the first newline" -- the cut docs-check makes for `{toml error}` -- gives for several invalid manifests.
fn main() {
    let cases: [(&str, &str); 6] = [
        ("unclosed header", "[package\n"),
        ("missing value", "[package]\nname =\n"),
        ("duplicate key", "[package]\nname = \"a\"\nname = \"b\"\n"),
        ("unterminated string", "[package]\nname = \"airsl\nversion.workspace = true\n"),
        ("error on line 3", "[package]\nname = \"airsl\"\nversion.workspace = = true\n"),
        ("duplicate table", "[package]\n[package]\n"),
    ];
    for (label, text) in cases {
        match toml::from_str::<toml::Value>(text) {
            Ok(v) => println!("{label}: parsed?! {v:?}"),
            Err(e) => {
                let full = e.to_string();
                let first = full.split('\n').next().unwrap_or("");
                println!("{label}: Display has {} lines; full = {full:?}", full.lines().count());
                println!("{label}: first line = {first:?}");
            }
        }
    }
}
