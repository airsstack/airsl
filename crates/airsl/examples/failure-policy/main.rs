//! Whether a caller surfaces a script's failure or discards it — and the one case that overrides
//! the choice.
//!
//! Exists as its own example because [`FailurePolicy`] is the only part of this crate that
//! describes the *caller's* behaviour rather than the script's permissions, and the two are read
//! at different times: the engine reads a [`Policy`] while building a state, this is read after
//! evaluation has already returned. Running it next to a sandbox example is what makes the
//! distinction land.
//!
//! Responsibilities: [`FailurePolicy::swallows_errors`], [`FailurePolicy::exit_code`], and the
//! asymmetry between an ordinary failure and a resource breach.
//!
//! Non-responsibilities: exiting. This example demonstrates the decision; it does not take it, and
//! returns zero whatever the scripts do.

use std::error::Error;
use std::path::{Path, PathBuf};

use airsl::{Engine, FailurePolicy, InstructionLimit, Policy, ResourceLimits, Script};

/// A budget too small for `counts.lua`, so an ordinary script becomes a breach.
///
/// Deliberately not a runaway script. The script this stops is correct and terminating; only the
/// budget it was given is small. That keeps the `.lua` file safe to run on its own, and still
/// produces the breach this example needs.
const TIGHT_INSTRUCTIONS: InstructionLimit = InstructionLimit::count(100_000);

fn main() -> Result<(), Box<dyn Error>> {
    let raises = script("raises.lua")?.with_args(["the tool call is not permitted here"]);
    let counts = script("counts.lua")?;

    println!("-- an ordinary failure: the script raised");
    let generous = Engine::builder().policy(Policy::confined()).build()?;
    for policy in [FailurePolicy::Report, FailurePolicy::FailOpen] {
        describe(policy, &generous, &raises);
    }

    println!();
    println!("-- a resource breach: the same policies, a different kind of failure");
    let tight = Engine::builder()
        .policy(
            Policy::confined()
                .with_limits(ResourceLimits::none().with_instructions(Some(TIGHT_INSTRUCTIONS))),
        )
        .build()?;
    for policy in [FailurePolicy::Report, FailurePolicy::FailOpen] {
        describe(policy, &tight, &counts);
    }

    println!();
    println!("-- and with a budget that fits, there is no failure to react to");
    println!("counts.lua returned {}", generous.eval_to::<i64>(&counts)?);

    Ok(())
}

/// Loads a script that lives beside this example, naming the chunk after the file alone.
///
/// `CARGO_MANIFEST_DIR` rather than the working directory, so the example runs the same from the
/// workspace root, from the crate directory, or from a `cargo run` in an editor.
///
/// `from_file` alone would take the chunk name from the path *as given*, and the path built here
/// is absolute. That name appears in every error message this example prints, so inheriting it
/// would put the developer's home directory into the diagnostics — and for a hook whose stderr
/// ends up in someone else's log that is a real leak, not only an untidy line. `with_name`
/// replaces the label and leaves everything else alone, so the script keeps the root it was read
/// from and could still `require` a sibling.
fn script(file: &str) -> Result<Script, Box<dyn Error>> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/failure-policy")
        .join(file);
    Ok(Script::from_file(&path)?.with_name(file)?)
}

/// Runs `script` and reports what `policy` would have the caller do about the outcome.
///
/// This is the shape from the how-to guide, with the reaction printed instead of performed: the
/// breach test is `||`, not `&&`, because a caller that discards ordinary script failures usually
/// still wants to hear that the host's memory or CPU was consumed.
fn describe(policy: FailurePolicy, engine: &Engine, script: &Script) {
    let Err(error) = engine.eval(script) else {
        println!("{policy:?}: succeeded, nothing to react to");
        return;
    };

    let breached = error.exhausted_limit().is_some();
    let surfaced = !policy.swallows_errors() || breached;

    // A breach reported under `FailOpen` still exits zero. Surfacing the diagnostic and choosing
    // the exit status are separate decisions, and only the second one can block a tool call.
    println!(
        "{policy:?}: surfaced={surfaced} exit={} breach={breached}",
        policy.exit_code()
    );
    if surfaced {
        println!("  {error}");
    }
}
