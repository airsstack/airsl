//! A host module of your own, installed next to the built-ins under a root table named for your
//! program.
//!
//! This is the program the "Writing Your Own Module" chapter shows: a `notes` module whose `add`
//! function collects strings from a script, and an engine that reads them back.
//!
//! Run it with `cargo run -p airsl --example notes-module`. The editor setup for the same module
//! is in `setup.rs`.

#![expect(
    clippy::expect_used,
    reason = "the notes lock is only held for a push and is never poisoned"
)]

include!("notes.rs");

// ANCHOR: run
use airsl::{Engine, Policy, RootTable, Script};

fn main() -> Result<(), airsl::Error> {
    let collected = Arc::new(Mutex::new(Vec::new()));

    let mut modules = airsl::modules::stdlib()?;
    modules.insert(Box::new(Notes {
        name: ModuleName::new("notes")?,
        collected: Arc::clone(&collected),
    }))?;

    let engine = Engine::builder()
        .policy(Policy::confined())
        .root_table(RootTable::new("myapp")?)
        .stdlib(modules)
        .build()?;

    let script = Script::from_source(
        r#"
          myapp.notes.add("disk is 91% full")
          myapp.notes.add("backup finished in " .. 42 .. "s")
        "#,
        "report",
    )?;
    engine.eval(&script)?;

    for note in collected.lock().expect("notes lock poisoned").iter() {
        println!("note: {note}");
    }
    Ok(())
}
// ANCHOR_END: run
