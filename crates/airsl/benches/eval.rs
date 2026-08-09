//! Timing harness for the three engine paths whose relative cost shapes the crate's API.
//!
//! Exists as a bench target rather than a test because the numbers it produces decide an API
//! question — whether an embedded consumer should reuse an engine or rebuild one — and that
//! decision needs a measurement that can be re-taken after a dependency or configuration change.
//!
//! Responsibilities: [`main`], which times engine construction, evaluation on a reused engine, and
//! evaluation on a fresh engine, and prints one line per measurement.
//!
//! Non-responsibilities: judging the result. It reports nanoseconds per iteration and nothing else.
//!
//! Run the full pass with `cargo bench -p airsl`. Any other invocation — notably the
//! `--all-targets` test build, which links and runs this target's `main` — takes a short smoke
//! pass instead, so the ordinary build does not pay for a full measurement.

use std::hint::black_box;
use std::time::{Duration, Instant};

use airsl::{Engine, Policy, ResourceLimits, Script};

/// Iterations per measurement when invoked as a benchmark.
const FULL_ITERATIONS: usize = 1000;

/// Iterations per measurement otherwise, enough to prove the target still runs.
const SMOKE_ITERATIONS: usize = 10;

/// A chunk that does nothing, isolating the cost of loading and running a chunk at all.
const TRIVIAL: &str = "return 1";

/// The confined preset with both ceilings lifted.
///
/// Paired with the default in every measurement so the cost of arming the ceilings is separable
/// from the cost of running the script at all. The instruction ceiling is a VM hook on the hot
/// path, so it is the one policy choice with a price worth knowing before making it.
const fn unbounded() -> Policy {
    Policy::confined().with_limits(ResourceLimits::none())
}

/// A chunk that crosses the Lua-to-Rust boundary a thousand times.
///
/// Separate from [`TRIVIAL`] because the two answer different questions: a boundary crossing pays
/// costs a pure-Lua chunk never touches, so a configuration change can leave one flat and move the
/// other substantially.
const CALLBACK_HEAVY: &str = "for i = 1, 1000 do airsstack.json.encode({ a = i }) end return 1";

fn main() -> Result<(), airsl::Error> {
    let iterations = if std::env::args().any(|arg| arg == "--bench") {
        FULL_ITERATIONS
    } else {
        SMOKE_ITERATIONS
    };

    println!("iterations: {iterations}");
    for (label, policy) in [
        ("ceilings armed", Policy::confined()),
        ("no ceilings", unbounded()),
    ] {
        report(
            &format!("engine construction, {label}"),
            iterations,
            construction(&policy, iterations)?,
        );
        report(
            &format!("eval, reused engine, trivial, {label}"),
            iterations,
            reused(&policy, TRIVIAL, iterations)?,
        );
        report(
            &format!("eval, reused engine, callback-heavy, {label}"),
            iterations,
            reused(&policy, CALLBACK_HEAVY, iterations)?,
        );
        report(
            &format!("eval, fresh engine, trivial, {label}"),
            iterations,
            fresh(&policy, iterations)?,
        );
    }
    Ok(())
}

/// Prints one measurement as nanoseconds per iteration.
fn report(label: &str, iterations: usize, elapsed: Duration) {
    let divisor = u128::try_from(iterations).unwrap_or(1).max(1);
    let per = elapsed.as_nanos() / divisor;
    println!("{label:<52} {per:>10} ns/iter");
}

/// Times building an engine, repeatedly, from scratch.
fn construction(policy: &Policy, iterations: usize) -> Result<Duration, airsl::Error> {
    let start = Instant::now();
    for _ in 0..iterations {
        let engine = Engine::builder().policy(policy.clone()).build()?;
        black_box(&engine);
    }
    Ok(start.elapsed())
}

/// Times evaluating `source` on one engine built up front.
fn reused(policy: &Policy, source: &str, iterations: usize) -> Result<Duration, airsl::Error> {
    let engine = Engine::builder().policy(policy.clone()).build()?;
    let script = Script::from_source(source, "bench")?;

    let start = Instant::now();
    for _ in 0..iterations {
        black_box(engine.eval_to::<i64>(&script)?);
    }
    Ok(start.elapsed())
}

/// Times evaluating a trivial chunk on an engine built fresh each time.
fn fresh(policy: &Policy, iterations: usize) -> Result<Duration, airsl::Error> {
    let script = Script::from_source(TRIVIAL, "bench")?;

    let start = Instant::now();
    for _ in 0..iterations {
        let engine = Engine::builder().policy(policy.clone()).build()?;
        black_box(engine.eval_to::<i64>(&script)?);
    }
    Ok(start.elapsed())
}
