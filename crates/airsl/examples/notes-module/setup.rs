//! Setting up an editor for a host module of your own, and the test that keeps its description
//! true.
//!
//! This is the second program the "Writing Your Own Module" chapter shows. Run it with
//! `cargo run -p airsl --example notes-module-setup` from a scratch folder: it writes `.airsl/`
//! and `.luarc.json` into the folder you run it from. Its test checks that the description matches
//! what the module installs; run it with `cargo test -p airsl --example notes-module-setup`. The
//! module itself is in `notes.rs`.

#![cfg_attr(
    test,
    expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )
)]
#![expect(
    clippy::expect_used,
    reason = "the notes lock is only held for a push and is never poisoned"
)]

include!("notes.rs");

// ANCHOR: setup
use std::path::Path;

use airsl::RootTable;
use airsl::luals::setup;

fn notes() -> Result<Notes, airsl::Error> {
    Ok(Notes {
        name: ModuleName::new("notes")?,
        collected: Arc::new(Mutex::new(Vec::new())),
    })
}

fn main() -> Result<(), airsl::Error> {
    let root = RootTable::new("myapp")?;

    let mut modules = airsl::modules::stdlib()?;
    modules.insert(Box::new(notes()?))?;

    let report = setup(Path::new("."), &modules, &root)?;
    println!("{report}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use airsl::luals::verify;

    use super::*;

    #[test]
    fn notes_describes_what_it_installs() {
        verify(&notes().unwrap(), &RootTable::new("myapp").unwrap()).unwrap();
    }
}
// ANCHOR_END: setup
