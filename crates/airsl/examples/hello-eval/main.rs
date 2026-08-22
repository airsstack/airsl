//! The smallest complete embedding: build an engine, run a chunk, take a value back.
//!
//! Exists as its own example because it is the one that demonstrates [`Script::from_source`].
//! Every other example loads a `.lua` file from disk, which is what a real embedder does; this one
//! keeps the Lua inline precisely because the subject here is the Rust API and the chunk is a
//! single expression.
//!
//! Responsibilities: the type-state builder, [`Policy::confined`], and the difference between
//! [`Engine::eval`] and [`Engine::eval_to`].
//!
//! Non-responsibilities: granting authority. Nothing here touches the filesystem, the environment
//! or a subprocess, so the confined preset's empty grant set is never consulted.

use airsl::{Engine, Policy, Script};

/// A chunk with no side effects, returning a value the host converts into a Rust type.
const GREETING: &str = r#"return "hello from Lua " .. _VERSION"#;

/// Sums a sequence, to show a return type other than `String`.
const SUM: &str = "local total = 0 for i = 1, 10 do total = total + i end return total";

fn main() -> Result<(), airsl::Error> {
    // There is no `build()` until `policy()` has been called. "Did I remember to sandbox this?" is
    // not a question this call site is able to get wrong.
    let engine = Engine::builder().policy(Policy::confined()).build()?;

    let greeting = Script::from_source(GREETING, "greeting")?;
    println!("{}", engine.eval_to::<String>(&greeting)?);

    let sum = Script::from_source(SUM, "sum")?;
    println!("1..10 sums to {}", engine.eval_to::<i64>(&sum)?);

    // `eval` runs the chunk and discards whatever it returned. Use it when the script is run for
    // its effects; `eval_to` when the host wants the value.
    engine.eval(&greeting)?;

    // The policy reads back off the engine, which is what keeps `airsl doctor` and the modules
    // agreeing about what is enforced.
    println!(
        "surface: {}, grants: {}",
        engine.policy().language(),
        engine.policy().grants()
    );

    Ok(())
}
