//! Moving values across the Rust/Lua boundary, and the JSON that comes back byte-stable.
//!
//! Exists as its own example because byte-stability is a *correctness* property of this crate
//! rather than a formatting preference, and the only way to show it is to encode the same table
//! twice and compare. A snippet in a doc comment can assert that JSON came out; it cannot assert
//! that the same input produced the same bytes.
//!
//! Responsibilities: [`Script::with_args`] and Lua's `arg` table, returning a Lua table to the
//! host, and `airsstack.json`'s `encode` / `encode_pretty` / `decode`.
//!
//! Non-responsibilities: writing any of it to disk. Encoding needs no authority, so this runs on
//! the confined preset's empty grant set; [`filesystem-grants`](../filesystem-grants/) is where a
//! script earns the right to persist what it built.

use std::path::Path;

use airsl::{Engine, Policy, Script};

/// The chunk name reported when a `mlua` conversion fails on the host side of the boundary.
const CHUNK: &str = "encode.lua";

fn main() -> Result<(), airsl::Error> {
    let engine = Engine::builder().policy(Policy::confined()).build()?;

    // `CARGO_MANIFEST_DIR` rather than a relative path, so the example runs the same from the
    // workspace root, from the crate directory, or from a `cargo make` task.
    let script = Script::from_file(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/values-and-json/encode.lua"),
    )?
    .with_args(["widget", "3"]);

    // The script returns a table; `eval_to` converts it into whatever `FromLuaMulti` type the host
    // asks for. `mlua` is re-exported by this crate deliberately — naming it separately would put
    // a second, possibly different, version of these types in scope.
    let out: airsl::mlua::Table = engine.eval_to(&script)?;
    let field = |key: &'static str| -> Result<String, airsl::Error> {
        out.get::<String>(key)
            .map_err(|e| airsl::Error::lua(CHUNK, e))
    };

    let compact = field("compact")?;
    let again = field("again")?;
    let pretty = field("pretty")?;

    // The point of the example. Lua iterates a table in hash order, which varies between runs, so
    // encoding straight through `serde_json` produced different bytes each time. `airsl` sorts
    // object keys unconditionally — see `sorted` in `src/convert.rs:30` — which is what lets a
    // script write an index or a lockfile that diffs cleanly.
    assert_eq!(
        compact, again,
        "the same table must encode to the same bytes"
    );

    // Object keys come out sorted; array elements keep the order the script gave them, because
    // that order is data rather than an artefact of Lua's hashing.
    println!("compact: {compact}");

    // An empty Lua table is an empty JSON object: Lua has no distinct empty-sequence value, so
    // there is nothing to disambiguate it with.
    assert!(compact.contains(r#""meta":{}"#), "{compact}");

    // Lua 5.4's integer subtype survives the round trip. On 5.1 or LuaJIT this would read
    // `1.7e9`, which is why the crate pins 5.4.
    assert!(compact.contains(r#""observed_at":1700000000"#), "{compact}");

    // `encode_pretty` indents and ends with a newline, so a script can write it straight to a file
    // that a human will read and a `git diff` will line up.
    print!("pretty:\n{pretty}");

    println!(
        "decoded back: name={} observed_at={} tags={}",
        field("decoded_name")?,
        out.get::<i64>("decoded_observed_at")
            .map_err(|e| airsl::Error::lua(CHUNK, e))?,
        out.get::<i64>("decoded_tag_count")
            .map_err(|e| airsl::Error::lua(CHUNK, e))?,
    );

    Ok(())
}
