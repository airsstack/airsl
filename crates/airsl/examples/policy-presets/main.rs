//! One script, three presets: what `trusted`, `confined` and `pure` actually take away.
//!
//! Exists as its own example because the language surface is the axis of the policy that is
//! hardest to see from the outside. A grant refusal announces itself with a message naming what
//! was granted; a withheld standard library is simply a `nil` the script finds when it looks. Two
//! presets side by side is the only way to make that visible.
//!
//! Responsibilities: [`LanguageSurface`] as expressed by the three [`Policy`] presets, and the
//! distinction between a library being absent and a library being present with functions removed.
//!
//! Non-responsibilities: grants and ceilings. The presets vary all three axes at once, so this
//! prints the other two rather than exploring them —
//! [`filesystem-grants`](../filesystem-grants/) and [`resource-limits`](../resource-limits/) take
//! those.

use std::path::Path;

use airsl::{Engine, Policy, Script};

fn main() -> Result<(), airsl::Error> {
    let script = Script::from_file(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/policy-presets/probe.lua"),
    )?;

    for (label, policy) in [
        ("trusted", Policy::trusted()),
        ("confined", Policy::confined()),
        ("pure", Policy::pure()),
    ] {
        // A policy belongs to an engine, not to an evaluation, so comparing presets means three
        // engines. This is also the reason a long-lived host builds its engine once: the surface
        // is decided when the Lua state is created and cannot be renegotiated afterwards.
        let engine = Engine::builder().policy(policy).build()?;
        println!("{label:<8} {}", engine.eval_to::<String>(&script)?);
    }

    println!();
    for (label, policy) in [
        ("trusted", Policy::trusted()),
        ("confined", Policy::confined()),
        ("pure", Policy::pure()),
    ] {
        // The other two axes, for context: a preset is a choice about all three at once, and
        // reading only the surface would make `trusted` look like a smaller step than it is.
        // Both limit types carry their unit in `Display`, so the format string must not add one.
        let limits = policy.limits();
        let memory = limits
            .memory()
            .map_or_else(|| String::from("unbounded"), |limit| limit.to_string());
        let instructions = limits
            .instructions()
            .map_or_else(|| String::from("unbounded"), |limit| limit.to_string());
        println!(
            "{label:<8} surface={} grants={} memory={memory} instructions={instructions}",
            policy.language(),
            policy.grants(),
        );
    }

    Ok(())
}
