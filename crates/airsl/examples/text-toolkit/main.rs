//! The half of the host standard library that needs no grant at all.
//!
//! Exists as its own example because every other example on the grant axis shows a module earning
//! authority, and the roster's larger half never asks for any: `regex`, `path`, `hash`, `time` and
//! `glob.match` compute over the arguments they are handed. Reading only those examples leaves the
//! impression that a script under an empty grant set cannot do anything, when in fact it can do
//! most text work — which is the case this one puts in front of the reader.
//!
//! Responsibilities: running one script under [`Policy::pure`] — the tightest preset, granting
//! nothing and dropping `os` — and showing that the whole sweep below still runs under it.
//!
//! Non-responsibilities: the text work itself. `toolkit.lua` is the subject here and the host is
//! deliberately thin; a Rust file that reproduced the calls would be demonstrating `mlua` instead.

use std::error::Error;
use std::path::Path;

use airsl::{Engine, Policy, Script};

fn main() -> Result<(), Box<dyn Error>> {
    // `pure` rather than `confined`: it is the preset with the least to give away, so nothing the
    // script goes on to do can be explained by authority it happened to be left holding.
    let policy = Policy::pure();
    println!(
        "policy: pure — surface={} grants={}",
        policy.language(),
        if policy.grants().is_empty() {
            "none"
        } else {
            "some"
        }
    );
    println!();

    let engine = Engine::builder().policy(policy).build()?;

    // `CARGO_MANIFEST_DIR` rather than a relative path, so the example runs the same from the
    // workspace root, from the crate directory, or from a `cargo make` task.
    let script = Script::from_file(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/text-toolkit/toolkit.lua"),
    )?;

    engine.eval(&script)?;

    Ok(())
}
