//! The other two grant axes: an environment-variable allowlist, and an executable allowlist.
//!
//! Exists as its own example because `env` and `proc` are the pair that has to agree. A variable
//! set through `env.set` lands in a per-process overlay rather than in the host's own environment,
//! and the only way to show that the overlay is real is to spawn a child and let it report what it
//! sees.
//!
//! Responsibilities: [`EnvGrant`] as a name allowlist, [`ProcGrant`] as an executable allowlist,
//! the overlay `env.set` writes into, and `proc.run` as an argv array with no shell form.
//!
//! Non-responsibilities: the filesystem. No grant here touches a path, so `fs` refuses everything
//! for the whole run — which is [`filesystem-grants`](../filesystem-grants/)'s subject, not this
//! one's.

use std::path::Path;

/// A name every process on a unix system already has, granted so the script can read a value it
/// did not write. Its *value* is never printed: it differs on every machine.
const INHERITED: &str = "PATH";

/// Granted but never set anywhere, to show that `nil` and a refusal are different answers.
const UNSET: &str = "AIRSL_EXAMPLE_UNSET";

/// Written into the overlay by the script, then reported back by a child process.
const PASSED: &str = "AIRSL_EXAMPLE_PASSED";

/// A name the script asks for and was never granted.
const UNGRANTED: &str = "AIRSL_EXAMPLE_SECRET";

use airsl::{Engine, GrantSet, Policy, Script};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Three names granted, one of them never set. Everything else the host process inherited stays
    // invisible to the script — that is the whole point of an allowlist.
    let policy = Policy::confined().with_grants(
        GrantSet::declared()
            .with_env(|env| env.read([INHERITED, UNSET, PASSED]))
            // The grant matches the program name as written: `sh` does not permit `/bin/sh`.
            .with_proc(|proc| proc.allow(["sh"])),
    );
    let engine = Engine::builder().policy(policy).build()?;

    let lua = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/env-and-proc/child.lua");
    let script = Script::from_file(lua)?.with_args([
        INHERITED.to_owned(),
        UNSET.to_owned(),
        PASSED.to_owned(),
        UNGRANTED.to_owned(),
    ]);

    engine.eval(&script)?;

    // The script wrote `PASSED` into the overlay and a child process read it back, yet the host's
    // own environment never acquired it. `std::env::set_var` is `unsafe` in Edition 2024 and this
    // crate forbids `unsafe` — but the deeper reason is that a sandboxed script quietly mutating
    // the host's environment is not a capability anyone meant to grant.
    assert!(
        std::env::var(PASSED).is_err(),
        "`env.set` must not reach the host's own environment"
    );
    println!("host environment still does not define AIRSL_EXAMPLE_PASSED");

    Ok(())
}
