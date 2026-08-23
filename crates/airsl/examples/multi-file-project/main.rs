//! A Lua project split across files, resolving against its own directory and nowhere else.
//!
//! Exists as its own example because `require` is the one global whose *identity* changes with the
//! language surface rather than its availability. On `full` a script keeps Lua's own loader, which
//! searches `package.path` — the process's working directory and its installation prefixes — and
//! can open anything on the machine. On `confined` it gets a Rust function that resolves under the
//! directory the script was read from, has no `package` table to widen, and refuses a name that
//! could describe a parent. Two functions behind one name is not something a single-file example
//! can show.
//!
//! Responsibilities: the confined `require` — resolution through `<stem>.lua` and
//! `<stem>/init.lua`, the module cache and how long it lives, the three ways a target is refused,
//! and the three-way answer to which `require` a script gets at all.
//!
//! Non-responsibilities: the rest of the policy. A preset decides grants and ceilings too, and
//! [`policy-presets`](../policy-presets/) is where those are read.

use std::error::Error;
use std::path::{Path, PathBuf};

use airsl::{Engine, Policy, Script};

/// A confined script the engine has no directory for, because it never came from one.
///
/// [`Script::from_source`] records no root, and the disposition for a restricted surface without
/// one is to clear the global: there is nothing to confine a loader to, and a loader that fell
/// back to the working directory would resolve differently depending on where the host was
/// started. Inline because the point is precisely that this script is not a file.
const FROM_MEMORY: &str = r#"print(string.format(
  "%-8s require=%-8s and no directory that could give it one",
  "memory",
  require == nil and "absent" or "present"
))"#;

fn main() -> Result<(), Box<dyn Error>> {
    let dir = project_dir()?;
    let root = dir.display().to_string();
    let app = Script::from_file(dir.join("app.lua"))?.with_name("app.lua")?;

    // Two evaluations on one engine, then one on another. The module cache belongs to the engine
    // and outlives the evaluation that filled it, so the second run is where a module body would
    // show up again if it did not — and the third run is the control that proves the first run's
    // output was not simply what `app.lua` always prints.
    let engine = Engine::builder().policy(Policy::confined()).build()?;
    engine.eval(&run(&app, &root, "-- a fresh engine"))?;
    engine.eval(&run(&app, &root, "\n-- the same engine again"))?;

    let second = Engine::builder().policy(Policy::confined()).build()?;
    second.eval(&run(&app, &root, "\n-- a second engine"))?;

    // One script under three presets, the way `policy-presets` compares them — narrowed here to
    // the single global whose meaning the surface changes rather than merely withholding.
    let which = Script::from_file(dir.join("which_require.lua"))?.with_name("which_require.lua")?;
    for (heading, label, policy) in [
        (
            "\n-- one name, three functions",
            "trusted",
            Policy::trusted(),
        ),
        ("", "confined", Policy::confined()),
        ("", "pure", Policy::pure()),
    ] {
        let probe = Engine::builder().policy(policy).build()?;
        probe.eval(&which.clone().with_args([label, heading]))?;
    }

    // The same surface as the middle row above, and still no `require`. The loader is confined to
    // the directory the script was read from, and this one was never read from a directory at all.
    engine.eval(&Script::from_source(FROM_MEMORY, "memory")?)?;

    Ok(())
}

/// `app.lua` with the two arguments one evaluation of it needs.
///
/// The heading is an argument, and carries its own leading blank line, because the host writes
/// nothing to stdout itself. Not an ordering constraint — Lua's `print` and Rust's `println!` both
/// flush per line and interleave correctly — but a substitution one: every line below is put
/// through the script's own redaction of the project root, and a heading printed from here would
/// be the only line in the block that had not been.
///
/// `root` is the second thing the script cannot work out for itself: a refusal names the directory
/// that was searched, and an absolute path differs on every machine, so the script is given the
/// directory in order to rewrite it back out of what it prints.
fn run(app: &Script, root: &str, heading: &str) -> Script {
    app.clone().with_args([root, heading])
}

/// The directory this example's Lua files live in.
///
/// `CARGO_MANIFEST_DIR` rather than the working directory, so the example runs the same from the
/// workspace root, from the crate directory, or from a `cargo run` in an editor.
///
/// Canonical because the loader canonicalises the root before naming it in a refusal. A path that
/// still held a symlink would be printed in one form and handed to the script in another, and the
/// substitution the script performs would then miss.
fn project_dir() -> Result<PathBuf, Box<dyn Error>> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/multi-file-project")
        .canonicalize()?)
}
