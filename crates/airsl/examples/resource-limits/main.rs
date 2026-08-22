//! What happens when a script does not terminate, or eats the host's memory.
//!
//! Exists as its own example, and keeps its Lua inline, for a reason the other examples do not
//! share: the chunks here are deliberately malicious. A shipped `while true do end` file would be
//! a footgun for anyone pointing `airsl run` or `airsl check` at this directory, so these two
//! chunks live as `const`s that only this example can reach.
//!
//! Responsibilities: arming both ceilings, tripping each one, and showing that
//! [`airsl::Error::exhausted_limit`] separates a resource breach from a script defect.
//!
//! Non-responsibilities: deciding what a breach means to the caller. That is
//! [`airsl::FailurePolicy`]'s question, and the `failure-policy` example's subject.

use std::error::Error;

use airsl::{
    Engine, ExhaustedLimit, InstructionLimit, MemoryLimit, Policy, ResourceLimits, Script,
};

/// A loop with no exit condition: the script the instruction ceiling exists for.
const RUNAWAY_LOOP: &str = "while true do end";

/// Allocation with no upper bound, holding every string so the collector cannot reclaim them.
///
/// Separate from [`RUNAWAY_LOOP`] because the two breach different ceilings, and a caller that
/// cares about the difference has to be able to tell them apart from the error alone.
const RUNAWAY_ALLOCATION: &str =
    "local held = {} while true do held[#held + 1] = string.rep('x', 4096) end";

/// The same shape of loop, but bounded — the case both ceilings are supposed to leave alone.
const BOUNDED_LOOP: &str = "local n = 0 for _ = 1, 200000 do n = n + 1 end return n";

/// Low enough to trip in milliseconds.
///
/// The engine's hook fires every 10,000 instructions (`src/instruction_budget.rs:34`), so a
/// ceiling is enforced to within that many rather than exactly. A ceiling below the check interval
/// would be noticed only at the first fire, which is why this is a multiple of it and not, say, 7.
const TIGHT_INSTRUCTIONS: InstructionLimit = InstructionLimit::count(100_000);

/// Small enough that a runaway allocation reaches it quickly, large enough to build a state in.
const TIGHT_MEMORY: MemoryLimit = MemoryLimit::mebibytes(8);

fn main() -> Result<(), Box<dyn Error>> {
    instruction_ceiling()?;
    memory_ceiling()?;
    ceilings_lifted()?;
    Ok(())
}

/// Trips the instruction ceiling and classifies the result.
fn instruction_ceiling() -> Result<(), Box<dyn Error>> {
    let policy = Policy::confined()
        .with_limits(ResourceLimits::none().with_instructions(Some(TIGHT_INSTRUCTIONS)));
    let engine = Engine::builder().policy(policy).build()?;

    let error = engine
        .eval(&Script::from_source(RUNAWAY_LOOP, "runaway-loop")?)
        .err()
        .ok_or("the instruction ceiling did not stop a non-terminating script")?;

    // The classification is structural: the engine consults its own counter and the VM error
    // chain, never the message text (`src/engine.rs:248`). A script is free to raise a string that
    // reads exactly like this report, and matching on text would let it disguise its own failure
    // as a resource breach — or the reverse.
    assert_eq!(error.exhausted_limit(), Some(ExhaustedLimit::Instructions));

    println!("instruction ceiling: {error}");
    println!("  classified as: {:?}", error.exhausted_limit());
    Ok(())
}

/// Trips the memory ceiling and classifies the result.
fn memory_ceiling() -> Result<(), Box<dyn Error>> {
    // The instruction ceiling is left off deliberately. `classify` tests the budget first, so a
    // script that would breach both is reported as an instruction breach — arming only the one
    // under test is what makes this example's answer unambiguous.
    let policy =
        Policy::confined().with_limits(ResourceLimits::none().with_memory(Some(TIGHT_MEMORY)));
    let engine = Engine::builder().policy(policy).build()?;

    let error = engine
        .eval(&Script::from_source(RUNAWAY_ALLOCATION, "runaway-alloc")?)
        .err()
        .ok_or("the memory ceiling did not stop an unbounded allocation")?;

    assert_eq!(error.exhausted_limit(), Some(ExhaustedLimit::Memory));

    println!("memory ceiling: {error}");
    println!("  classified as: {:?}", error.exhausted_limit());
    Ok(())
}

/// Runs the bounded loop with no ceilings, then under one tight enough to stop it.
fn ceilings_lifted() -> Result<(), Box<dyn Error>> {
    // `ResourceLimits::none()` is what `Policy::trusted()` carries. It is the right choice for
    // first-party code and the wrong one for anything else: without the instruction ceiling there
    // is no defence against a script that never finishes, and the crate offers no `sleep` or
    // timeout to substitute for one.
    let unbounded = Engine::builder()
        .policy(Policy::confined().with_limits(ResourceLimits::none()))
        .build()?;

    let counted = unbounded.eval_to::<i64>(&Script::from_source(BOUNDED_LOOP, "bounded-loop")?)?;
    println!("no ceilings: the bounded loop counted to {counted}");

    // The same chunk under the tight ceiling is stopped, because 200,000 iterations cost more than
    // 100,000 instructions. A ceiling is a budget, not a judgement about the script: nothing here
    // is wrong with the loop, it was simply given less than it needed.
    let tight = Engine::builder()
        .policy(
            Policy::confined()
                .with_limits(ResourceLimits::none().with_instructions(Some(TIGHT_INSTRUCTIONS))),
        )
        .build()?;
    let stopped = tight.eval_to::<i64>(&Script::from_source(BOUNDED_LOOP, "bounded-loop")?);
    println!(
        "tight ceiling: the same bounded loop is stopped: {}",
        stopped.is_err()
    );

    Ok(())
}
