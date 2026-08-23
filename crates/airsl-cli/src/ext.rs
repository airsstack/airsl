//! `airsl ext`: dispatches the extension subcommands to their implementations.
//!
//! Its own module because `ext` fans out to two leaves — `doctor` and `fire` — that share only
//! the argument grammar `cli` already owns and otherwise have nothing in common: one never builds
//! an engine, the other always does. A dispatcher keeps that fan-out out of `main.rs`.
//!
//! Responsibilities: [`run`], matching an [`ExtCommand`] to `ext_doctor::run` or `ext_fire::run`.
//!
//! Non-responsibilities: negotiating a manifest against a ceiling (`ext_doctor`) and running an
//! extension's entry point (`ext_fire`); reading the process's actual stdin/stdout/stderr, which
//! `main.rs` supplies. `stdin`/`stdout`/`stderr` are taken as parameters here, the same shape
//! `ext_doctor::run` and `ext_fire::run` already use, so a unit test can dispatch through this
//! function with in-memory readers/writers instead of touching the process's real, possibly-open
//! streams.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::io::{Read, Write};

use crate::cli::ExtCommand;

/// Runs one `ext` subcommand and returns the process exit code.
///
/// `stdin` and `stderr` reach `ext fire` only: `ext doctor` never runs an extension's entry point
/// (no `stdin` to read) and reports its own failures with `eprintln!` directly, unchanged by this
/// pass. `stdout` reaches both arms.
pub(crate) fn run(
    command: ExtCommand,
    stdin: impl Read,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    match command {
        ExtCommand::Doctor { dir, flags } => crate::ext_doctor::run(&dir, flags, stdout),
        ExtCommand::Fire { dir, event, flags } => {
            crate::ext_fire::run(&dir, &event, flags, stdin, stdout, stderr)
        }
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::run;
    use crate::cli::{ExtCommand, ExtFlags};

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("extension.toml"),
            "[extension]\nname='t'\nversion='1'\nentry='main.lua'\napi=1\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("main.lua"), "return 1").unwrap();
        dir
    }

    #[test]
    fn doctor_dispatches_to_ext_doctor() {
        let dir = fixture();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run(
            ExtCommand::Doctor {
                dir: dir.path().to_path_buf(),
                flags: ExtFlags::default(),
            },
            &[][..],
            &mut out,
            &mut err,
        );
        assert_eq!(code, 0);
        let out = String::from_utf8(out).unwrap();
        assert!(out.starts_with("extension:"), "{out}");
    }

    #[test]
    fn fire_dispatches_to_ext_fire() {
        let dir = fixture();
        let mut out = Vec::new();
        let mut err = Vec::new();
        // The fixture's `main.lua` registers no handler, so a well-formed load with nothing
        // subscribed to `ping` is enough to prove this arm reaches `ext_fire::run`:
        // `ext_fire`'s own tests cover the dispatch behaviour itself.
        let code = run(
            ExtCommand::Fire {
                dir: dir.path().to_path_buf(),
                event: "ping".to_owned(),
                flags: ExtFlags::default(),
            },
            &[][..],
            &mut out,
            &mut err,
        );
        assert_eq!(code, 0);
    }
}
