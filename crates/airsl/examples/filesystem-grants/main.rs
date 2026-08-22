//! Handing a script filesystem authority: two roots, two directions, one containment rule.
//!
//! Exists as its own example because the read root and the write root are separate grants, and
//! that is the single fact about `FsGrant` most likely to be assumed away. A script granted a
//! write root cannot read what it just wrote unless the host said so twice.
//!
//! Responsibilities: [`FsGrant::read`] and [`FsGrant::write`] as independent roots, the `fs`
//! functions that go through each direction, and the shape of a refusal.
//!
//! Non-responsibilities: loading the script. [`Script::from_file`] is a *host* read and is not
//! governed by the grants — the grant governs what `airsstack.fs` does once the script is running.

use std::path::Path;

use airsl::{Engine, GrantSet, Policy, Script};
use tempfile::TempDir;

/// Contents seeded into the read root, fixed so the byte count in the output does not drift.
const NOTES: &str = "the grant is checked inside the host function, before the operation.\n";

/// A file the script is never granted any authority over.
const SECRET: &str = "not reachable from the script\n";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Three separate directories, none of them inside the repository. Canonicalised because a
    // refusal names the path the operation would actually touch, and on macOS a temporary
    // directory reaches its real location through a symlink — without this the roots the script
    // was told about would not be spelled the same way as the roots in the message.
    let source = TempDir::new()?;
    let output = TempDir::new()?;
    let outside = TempDir::new()?;
    let source_root = source.path().canonicalize()?;
    let output_root = output.path().canonicalize()?;
    let outside_root = outside.path().canonicalize()?;

    std::fs::write(source_root.join("notes.txt"), NOTES)?;
    std::fs::write(outside_root.join("secret.txt"), SECRET)?;

    // Read and write are granted separately and point at different directories. Granting one root
    // for both would hide the distinction this example exists to show.
    let policy = Policy::confined()
        .with_grants(GrantSet::declared().with_fs(|fs| fs.read(&source_root).write(&output_root)));
    let engine = Engine::builder().policy(policy).build()?;

    // Reading the script off disk is the host's own `std::fs` call. It is not checked against the
    // grants, and the script file does not have to live under a granted root.
    let lua = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/filesystem-grants/copy.lua");
    let script = Script::from_file(lua)?.with_args([
        source_root.to_string_lossy().into_owned(),
        output_root.to_string_lossy().into_owned(),
        outside_root.to_string_lossy().into_owned(),
    ]);

    engine.eval(&script)?;

    Ok(())
}
