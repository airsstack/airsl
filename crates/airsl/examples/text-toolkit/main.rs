//! The half of the host standard library that needs no file, variable or exec grant.
//!
//! Exists as its own example because every other example on the grant axis shows a module earning
//! authority, and the roster's larger half asks for little: `regex`, `path`, `hash`, `time` and
//! `glob.match` compute over the arguments they are handed. Three of their functions read the host
//! and need a host access member instead — `time.now` and `time.monotonic` need `clock`, and
//! `path.absolute` needs `cwd`. Reading only the other examples leaves the impression that a
//! script under a small grant set cannot do anything, when in fact it can do most text work —
//! which is the case this one puts in front of the reader.
//!
//! Responsibilities: running one script under [`Policy::pure`] — the tightest preset, which drops
//! `os` and `print` — given exactly two host-access members: `stdout`, so the script can write its
//! output, and `clock`, so it can read the monotonic clock. Showing that the whole sweep below
//! still runs under it.
//!
//! Non-responsibilities: the text work itself. `toolkit.lua` is the subject here and the host is
//! deliberately thin; a Rust file that reproduced the calls would be demonstrating `mlua` instead.

use std::error::Error;
use std::path::Path;

use airsl::{Engine, GrantSet, HostAccess, Policy, Script};

fn main() -> Result<(), Box<dyn Error>> {
    // `pure` rather than `confined`: it is the preset with the least to give away, so nothing the
    // script goes on to do can be explained by authority it happened to be left holding. The two
    // members are the whole of what it is given: `pure` has no `print`, so `stdout` carries the
    // output, and `clock` is what `time.monotonic` asks for.
    let policy = Policy::pure().with_grants(
        GrantSet::sealed()
            .with_access(HostAccess::Stdout)
            .with_access(HostAccess::Clock),
    );
    println!(
        "policy: pure — surface={} grants={}",
        policy.language(),
        policy.grants()
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
