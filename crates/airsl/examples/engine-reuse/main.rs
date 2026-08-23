//! The lifecycle of one [`Engine`]: what an evaluation leaves behind, and what sharing one means.
//!
//! Exists as its own example because every other example builds an engine, runs one script and
//! exits — which is precisely the shape that hides the two facts a dispatch loop depends on. An
//! engine is not a fresh sandbox per evaluation, and an engine shared between threads is a way to
//! avoid rebuilding a state rather than a way to run scripts at the same time.
//!
//! Like `hello-eval`, the chunks stay inline: the subject is the Rust API and each chunk is a line.
//! The `require` section is the exception, because a module cache needs modules on disk.
//!
//! Responsibilities: what is reset before every evaluation (the instruction counter, the `arg`
//! table, the `require` root), what survives it (the Lua globals a script wrote, the `require`
//! module cache), and [`Engine`]'s `Send + Sync` behaviour under an [`Arc`].
//!
//! Non-responsibilities: measuring any of it. Whether reuse is worth having is a question for
//! `benches/eval.rs`, and a duration printed here would give this example a different output on
//! every run.

use std::error::Error;
use std::fs;
use std::sync::Arc;
use std::thread;

use airsl::{Engine, InstructionLimit, Policy, ResourceLimits, Script};
use tempfile::TempDir;

/// Doubles its first argument, so what it returns is evidence of which `arg` table was in place.
const DOUBLE: &str = "return tonumber(arg[1]) * 2";

/// Writes a global and reports its own chunk name.
///
/// Nothing between evaluations clears that global, which is the fact this example exists to make
/// visible rather than a defect it is demonstrating.
const REMEMBER: &str = "SESSION = 'written by the first script' return arg[0]";

/// Reads the same global back, as a chunk with its own name and no relationship to the first.
const RECALL: &str = "return tostring(SESSION)";

/// Never terminates: used only to exhaust a budget, so the next evaluation can show it restored.
const RUNAWAY: &str = "while true do end";

/// Low enough to trip in milliseconds, and a multiple of the hook's 10,000-instruction check
/// interval (`src/instruction_budget.rs:34`), below which a ceiling is noticed only at the first
/// fire.
const TIGHT_INSTRUCTIONS: InstructionLimit = InstructionLimit::count(100_000);

/// Threads sharing the engine. Enough to overlap; few enough that the output stays readable.
const WORKERS: i64 = 4;

fn main() -> Result<(), Box<dyn Error>> {
    // One engine for everything except the two sections that need a policy of their own. Building
    // four would be an odd way to open an example about reuse.
    let engine = Engine::builder().policy(Policy::confined()).build()?;

    arguments_are_per_evaluation(&engine)?;
    println!();
    the_instruction_counter_is_per_evaluation()?;
    println!();
    globals_outlive_the_evaluation(&engine)?;
    println!();
    require_is_rooted_per_script_and_cached_per_engine(&engine)?;
    println!();
    one_engine_across_threads()?;

    Ok(())
}

/// Runs one `Script` three times with different arguments on a single engine.
fn arguments_are_per_evaluation(engine: &Engine) -> Result<(), Box<dyn Error>> {
    println!("the arg table is written before every evaluation:");

    let double = Script::from_source(DOUBLE, "double")?;
    for n in 1..=3i64 {
        // `with_args` consumes the script, so this clones it. That is the shape worth copying: one
        // `Script` describes the chunk, and the arguments belong to the evaluation rather than to
        // the engine — nothing here has to be told that the previous run happened.
        let run = double.clone().with_args([n.to_string()]);
        println!("  arg[1] = {n} doubles to {}", engine.eval_to::<i64>(&run)?);
    }

    // `arg[0]` is the script's own name, which is Lua's convention for a standalone script and what
    // a ported shell script reads where it previously read `$0`.
    let reporter = Script::from_source("return arg[0]", "reporter")?;
    println!(
        "  arg[0] is the chunk's own name: {}",
        engine.eval_to::<String>(&reporter)?
    );

    Ok(())
}

/// Exhausts an instruction budget, then evaluates again on the same engine.
fn the_instruction_counter_is_per_evaluation() -> Result<(), Box<dyn Error>> {
    // Its own engine: this section needs a ceiling tight enough to trip in milliseconds, which is
    // not a ceiling the rest of the example wants to run under.
    let engine = Engine::builder()
        .policy(
            Policy::confined()
                .with_limits(ResourceLimits::none().with_instructions(Some(TIGHT_INSTRUCTIONS))),
        )
        .build()?;

    let stopped = engine
        .eval(&Script::from_source(RUNAWAY, "runaway")?)
        .err()
        .ok_or("the instruction ceiling did not stop a non-terminating script")?;

    println!("the instruction counter is reset before every evaluation:");
    println!("  the first evaluation exhausts the budget: {stopped}");

    // The whole ceiling, not whatever the previous script left of it. A dispatch loop where the
    // second script inherited the first's remainder would fail in a way that depended on what ran
    // before it, which is not a failure anyone can reproduce from the script alone.
    let after = engine.eval_to::<i64>(&Script::from_source("return 7", "after")?)?;
    println!("  the next evaluation gets the whole ceiling again, and returns {after}");

    Ok(())
}

/// Writes a global in one chunk and reads it back in another.
fn globals_outlive_the_evaluation(engine: &Engine) -> Result<(), Box<dyn Error>> {
    let recall = Script::from_source(RECALL, "recall")?;

    println!("the globals a script writes outlive it:");
    println!(
        "  SESSION, before anything wrote it: {}",
        engine.eval_to::<String>(&recall)?
    );

    let author = engine.eval_to::<String>(&Script::from_source(REMEMBER, "remember")?)?;
    println!("  a script named `{author}` assigns it");

    // There is no per-evaluation environment restore to prevent this below `trusted`: `mlua`'s
    // one-call version is Luau-only, so isolating successive scripts on one engine would have to be
    // built rather than borrowed (`docs/architecture.md`). Give one engine scripts that trust each
    // other; give an untrusted script an engine of its own.
    println!(
        "  SESSION, read back by an unrelated script: {}",
        engine.eval_to::<String>(&recall)?
    );

    Ok(())
}

/// Shows the `require` root following the script, and the module cache following the engine.
fn require_is_rooted_per_script_and_cached_per_engine(
    engine: &Engine,
) -> Result<(), Box<dyn Error>> {
    // A temporary directory, removed when `dir` drops. Nothing this example runs is written inside
    // the repository.
    let dir = TempDir::new()?;

    for name in ["alpha", "beta"] {
        let root = dir.path().join(name);
        fs::create_dir(&root)?;
        fs::write(root.join("lib.lua"), format!("return '{name}'"))?;
        fs::write(root.join("main.lua"), "return require('lib')")?;
    }

    println!("the require root belongs to the script, not to the engine:");
    for name in ["alpha", "beta"] {
        let path = dir.path().join(name).join("main.lua");
        // Renamed off the absolute path `TempDir` produced: it differs on every run, so leaving it
        // as the chunk name would put an unreproducible string into any diagnostic this raised.
        let script = Script::from_file(&path)?.with_name(format!("{name}/main.lua"))?;
        println!(
            "  {name}/main.lua resolves require('lib') against its own directory: {}",
            engine.eval_to::<String>(&script)?
        );
    }

    // A module that counts its own executions. Lua's `package.loaded` answers 1, 1, 1; an engine
    // that rebuilt the cache per evaluation would answer 1, 2, 3, which is the inverted arrangement
    // this crate used to have — cached state discarded while leaked state was kept.
    let alpha = dir.path().join("alpha");
    fs::write(
        alpha.join("counter.lua"),
        "COUNT = (COUNT or 0) + 1 return COUNT",
    )?;
    fs::write(alpha.join("count.lua"), "return require('counter')")?;
    let count = Script::from_file(alpha.join("count.lua"))?.with_name("alpha/count.lua")?;

    let mut runs = Vec::new();
    for _ in 0..3 {
        runs.push(engine.eval_to::<i64>(&count)?.to_string());
    }

    println!();
    println!("the require module cache belongs to the engine, not to the script:");
    println!(
        "  one counting module, required by three evaluations: {}",
        runs.join(", ")
    );

    Ok(())
}

/// Shares one engine between threads through an [`Arc`] and collects what each of them got.
fn one_engine_across_threads() -> Result<(), Box<dyn Error>> {
    let engine = Arc::new(Engine::builder().policy(Policy::confined()).build()?);

    // Spawned eagerly. A lazy iterator that started each thread as it was joined would run them one
    // after another and so demonstrate nothing about sharing.
    let mut workers = Vec::new();
    for id in 0..WORKERS {
        let engine = Arc::clone(&engine);
        workers.push(thread::spawn(move || {
            let script = Script::from_source(DOUBLE, "double")?.with_args([id.to_string()]);
            engine.eval_to::<i64>(&script).map(|doubled| (id, doubled))
        }));
    }

    let mut results = Vec::new();
    for worker in workers {
        // `join`'s error is a panic payload rather than an `Error`. Discarding it costs nothing: a
        // panicking worker here is a defect in this example, not a condition to report on.
        results.push(worker.join().map_err(|_| "a worker thread panicked")??);
    }

    // The order threads finish in is not this example's to choose, and its output has to be
    // byte-identical on every run.
    results.sort_unstable();

    println!("one engine shared by {WORKERS} threads through an Arc:");
    for (id, doubled) in results {
        println!("  thread {id} saw its own argument and doubled it to {doubled}");
    }
    // The engine holds a lock spanning the whole evaluation, because `mlua`'s per-operation locking
    // is memory-safe without being correct across the four steps of one. Lua on a single state
    // cannot execute in parallel in any case, so the lock costs an uncontended acquisition and no
    // throughput — and a shared engine buys reuse, never concurrency.
    println!("  evaluations are serialised: sharing avoids rebuilding, it does not parallelise");

    Ok(())
}
