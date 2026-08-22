//! Every module is present even when nothing is granted, and says what *was* granted when it
//! refuses.
//!
//! Exists as its own example because the alternative design is the tempting one. Omitting a module
//! a script may not use would make `if airsstack.fs then` mean "am I allowed to read files" instead
//! of "does this runtime have a filesystem module" — two questions that need different answers, and
//! a script written against the first cannot be run under a different policy without editing it.
//!
//! Responsibilities: presence under a policy that grants almost nothing, and the refusal text each
//! guarded module produces.
//!
//! Non-responsibilities: the grants themselves. What a grant *permits* is
//! [`filesystem-grants`](../filesystem-grants/) and [`env-and-proc`](../env-and-proc/); this
//! example is about what happens at the edge of one.

use std::path::Path;

use airsl::{Engine, GrantSet, Policy, Script};
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // One narrow grant, so every refusal has something to name. A policy granting nothing at all
    // would print "no read roots are granted" four times and show nothing about how a refusal
    // helps: the usual cause of one is a grant a single directory too deep, which is invisible
    // unless the message lists the roots that did apply.
    let granted = TempDir::new()?;
    let granted_root = granted.path().canonicalize()?;
    std::fs::write(granted_root.join("visible.txt"), "inside the grant\n")?;

    let policy = Policy::confined().with_grants(
        GrantSet::declared()
            .with_fs(|fs| fs.read(&granted_root))
            .with_env(|env| env.read(["PATH"]))
            .with_proc(|proc| proc.allow(["sh"])),
    );
    let engine = Engine::builder().policy(policy).build()?;

    let lua = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/denials-are-data/denials.lua");
    let script = Script::from_file(lua)?.with_args([granted_root.to_string_lossy().into_owned()]);

    engine.eval(&script)?;

    Ok(())
}
