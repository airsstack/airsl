//! `airsl lua` — the subcommands that prepare a project for Lua tooling.
//!
//! Its own module, apart from `ext`, because nothing here loads or runs Lua: it is a thin caller
//! of [`airsl::luals::setup`] that turns the library's report or error into output and an exit
//! code. The streams are injected, the shape `ext_fire::run` has, so the tests assert on both.
//!
//! Responsibilities: [`run`], which dispatches a [`LuaCommand`], prints the report and maps
//! failures to an exit code.
//!
//! Non-responsibilities: deciding what gets written (`airsl::luals`) and parsing arguments (`cli`).
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::io::Write;
use std::path::Path;

use airsl::RootTable;
use airsl::luals::SetupReport;
use airsl::modules::stdlib;

use crate::cli::LuaCommand;

/// Runs one `lua` subcommand, writing its report to `stdout` and any diagnostic to `stderr`, and
/// returns the process exit code.
///
/// Every failure, including a failed write of the report, prints `airsl: {error}` to `stderr` and
/// exits 1. A failed report write comes after both files are written, so it never leaves a
/// half-set-up project behind.
pub(crate) fn run(command: LuaCommand, stdout: &mut impl Write, stderr: &mut impl Write) -> i32 {
    match command {
        LuaCommand::Setup { dir } => match setup(&dir) {
            Ok(report) => match writeln!(stdout, "{report}") {
                Ok(()) => 0,
                Err(error) => fail(stderr, error),
            },
            Err(error) => fail(stderr, error),
        },
    }
}

/// Sets `dir` up for the Lua language server with the built-in modules under the default root.
fn setup(dir: &Path) -> airsl::Result<SetupReport> {
    airsl::luals::setup(dir, &stdlib()?, &RootTable::default())
}

/// Writes `airsl: {error}` to `stderr` and returns the failing exit code.
fn fail(stderr: &mut impl Write, error: impl core::fmt::Display) -> i32 {
    let _ = writeln!(stderr, "airsl: {error}");
    1
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use std::io::{self, Write};
    use std::path::PathBuf;

    use super::run;
    use crate::cli::LuaCommand;

    /// A writer whose every write fails, to prove a failed report write is reported and not lost.
    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("boom"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("boom"))
        }
    }

    fn setup_command(dir: PathBuf) -> LuaCommand {
        LuaCommand::Setup { dir }
    }

    #[test]
    fn prints_the_report_and_exits_zero() {
        let project = tempfile::tempdir().unwrap();
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());

        let code = run(
            setup_command(project.path().to_path_buf()),
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            "created .airsl/types/airsstack.lua\ncreated .luarc.json\n"
        );
        assert!(stderr.is_empty());
        assert_eq!(code, 0);
    }

    #[test]
    fn prints_the_error_with_the_airsl_prefix_and_exits_one() {
        let project = tempfile::tempdir().unwrap();
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());

        let code = run(
            setup_command(project.path().join("missing")),
            &mut stdout,
            &mut stderr,
        );

        let stderr = String::from_utf8(stderr).unwrap();
        assert!(
            stderr.starts_with("airsl: open project directory failed on "),
            "{stderr}"
        );
        assert!(stdout.is_empty());
        assert_eq!(code, 1);
    }

    #[test]
    fn a_failed_report_write_exits_one() {
        let project = tempfile::tempdir().unwrap();
        let mut stderr = Vec::new();

        let code = run(
            setup_command(project.path().to_path_buf()),
            &mut FailingWriter,
            &mut stderr,
        );

        assert_eq!(String::from_utf8(stderr).unwrap(), "airsl: boom\n");
        assert_eq!(code, 1);
    }
}
