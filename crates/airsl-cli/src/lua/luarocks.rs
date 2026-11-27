//! The process boundary to `LuaRocks`.
//!
//! Its own file because it holds the only process spawn in the CLI, behind a trait, so every
//! failure path of the native flow is unit-tested with a scripted double and no luarocks on the
//! machine.
//!
//! Responsibilities: the [`Luarocks`] trait, the crate-owned results it returns ([`Captured`],
//! [`Outcome`], which exist because a `std::process::ExitStatus` cannot be built portably in
//! tests), [`System`], the implementation that spawns the real program, and [`LuarocksError`],
//! the two ways a spawn fails.
//!
//! Non-responsibilities: deciding what to run and what a failure means, which belong to
//! `native`.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::ffi::OsString;
use std::io;
use std::process::{Command, Stdio};

/// What a command that was run to read its output left behind.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Captured {
    /// Everything the child wrote to stdout, undecoded.
    pub stdout: Vec<u8>,
    /// The child's exit code, or `None` when a signal ended it.
    pub code: Option<i32>,
}

/// How a command that was run for its effects ended.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Outcome {
    /// The child's exit code, or `None` when a signal ended it.
    pub code: Option<i32>,
}

/// Why luarocks could not be started.
#[derive(Debug, thiserror::Error)]
pub(crate) enum LuarocksError {
    /// No program of that name exists, or none is on `PATH`.
    #[error("luarocks was not found")]
    Missing,
    /// The program exists but the operating system refused to start it.
    #[error("luarocks could not be started")]
    Spawn {
        /// The operating system's reason.
        #[source]
        source: io::Error,
    },
}

/// Runs luarocks. Implemented by [`System`] for real and by a scripted double in tests.
pub(crate) trait Luarocks {
    /// Runs luarocks with `args` and captures its stdout. Stderr is inherited and stdin is null.
    ///
    /// # Errors
    ///
    /// [`LuarocksError::Missing`] when the program does not exist, and
    /// [`LuarocksError::Spawn`] for any other failure to start it.
    fn query(&self, args: &[&str]) -> Result<Captured, LuarocksError>;

    /// Runs luarocks with `args`, inheriting stdout and stderr, with stdin null.
    ///
    /// # Errors
    ///
    /// [`LuarocksError::Missing`] when the program does not exist, and
    /// [`LuarocksError::Spawn`] for any other failure to start it.
    fn run(&self, args: &[&str]) -> Result<Outcome, LuarocksError>;
}

/// The program name luarocks is run and spoken of by.
pub(crate) const PROGRAM: &str = "luarocks";

/// The file name the operating system starts for luarocks on Windows, where only an `.exe` is
/// found by name.
pub(crate) const WINDOWS_PROGRAM: &str = "luarocks.exe";

/// The real luarocks, started by the bare name [`PROGRAM`].
///
/// The name is looked up through `PATH`. On Windows `std` first tries the folder `airsl.exe` is in
/// and the system folders, then `PATH`, and finds only a `luarocks.exe`.
///
/// Runs in airsl's own working directory, never another, so a relative path luarocks reads means
/// what the user typed.
#[derive(Debug)]
pub(crate) struct System {
    program: OsString,
}

impl Default for System {
    fn default() -> Self {
        Self {
            program: OsString::from(PROGRAM),
        }
    }
}

impl System {
    /// The command to spawn: the program with `args` and a null stdin, in airsl's own directory.
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(&self.program);
        command.args(args).stdin(Stdio::null());
        command
    }
}

/// Sorts a spawn error into the one case callers tell apart from the rest.
fn spawn_error(source: io::Error) -> LuarocksError {
    if source.kind() == io::ErrorKind::NotFound {
        LuarocksError::Missing
    } else {
        LuarocksError::Spawn { source }
    }
}

impl Luarocks for System {
    fn query(&self, args: &[&str]) -> Result<Captured, LuarocksError> {
        let output = self
            .command(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()
            .map_err(spawn_error)?;
        Ok(Captured {
            stdout: output.stdout,
            code: output.status.code(),
        })
    }

    fn run(&self, args: &[&str]) -> Result<Outcome, LuarocksError> {
        let status = self
            .command(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(spawn_error)?;
        Ok(Outcome {
            code: status.code(),
        })
    }
}

/// A scripted stand-in for luarocks, shared by the tests of the callers.
#[cfg(test)]
pub(crate) mod scripted {
    #![expect(
        clippy::panic,
        reason = "an unscripted call is a test bug; a panic is the intended failure signal"
    )]

    use std::cell::RefCell;
    use std::collections::VecDeque;

    use super::{Captured, Luarocks, LuarocksError, Outcome};

    /// Answers each call with the next queued result and records the arguments it was given.
    #[derive(Default)]
    pub(crate) struct Scripted {
        calls: RefCell<Vec<String>>,
        queries: RefCell<VecDeque<Result<Captured, LuarocksError>>>,
        runs: RefCell<VecDeque<Result<Outcome, LuarocksError>>>,
    }

    impl Scripted {
        /// Queues the answer to the next `query`.
        pub(crate) fn then_query(self, answer: Result<Captured, LuarocksError>) -> Self {
            self.queries.borrow_mut().push_back(answer);
            self
        }

        /// Queues the answer to the next `run`.
        pub(crate) fn then_run(self, answer: Result<Outcome, LuarocksError>) -> Self {
            self.runs.borrow_mut().push_back(answer);
            self
        }

        /// Queues a successful `query` that printed `stdout`.
        pub(crate) fn then_stdout(self, stdout: &[u8]) -> Self {
            self.then_query(Ok(Captured {
                stdout: stdout.to_vec(),
                code: Some(0),
            }))
        }

        /// The arguments of every call so far, each joined with single spaces.
        pub(crate) fn calls(&self) -> Vec<String> {
            self.calls.borrow().clone()
        }
    }

    impl Luarocks for Scripted {
        fn query(&self, args: &[&str]) -> Result<Captured, LuarocksError> {
            self.calls.borrow_mut().push(args.join(" "));
            self.queries
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| panic!("no scripted answer for query {args:?}"))
        }

        fn run(&self, args: &[&str]) -> Result<Outcome, LuarocksError> {
            self.calls.borrow_mut().push(args.join(" "));
            self.runs
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| panic!("no scripted answer for run {args:?}"))
        }
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]
    use super::{Luarocks as _, LuarocksError, PROGRAM, System, WINDOWS_PROGRAM};

    #[test]
    fn the_windows_program_is_the_program_with_an_exe_extension() {
        assert_eq!(WINDOWS_PROGRAM, format!("{PROGRAM}.exe"));
        assert_eq!(System::default().program, PROGRAM);
    }

    #[test]
    fn a_missing_program_is_reported_as_missing() {
        let dir = tempfile::tempdir().unwrap();
        let system = System {
            program: dir.path().join("no-such-luarocks").into_os_string(),
        };

        let error = system.query(&[]).unwrap_err();

        assert!(matches!(error, LuarocksError::Missing), "{error:?}");
    }

    #[cfg(unix)]
    #[test]
    #[expect(
        clippy::panic,
        reason = "a wrong error variant is the intended failure signal"
    )]
    fn a_non_executable_file_is_a_spawn_failure() {
        use std::io::ErrorKind;

        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("not-executable");
        std::fs::write(&program, "#!/bin/sh\n").unwrap();
        let system = System {
            program: program.into_os_string(),
        };

        let error = system.query(&[]).unwrap_err();

        let LuarocksError::Spawn { source } = error else {
            panic!("expected a spawn failure, got {error:?}");
        };
        assert_eq!(source.kind(), ErrorKind::PermissionDenied);
    }

    #[cfg(unix)]
    #[test]
    fn a_killed_child_has_no_exit_code() {
        let system = System {
            program: "/bin/sh".into(),
        };

        let captured = system.query(&["-c", "kill -9 $$"]).unwrap();
        let outcome = system.run(&["-c", "kill -9 $$"]).unwrap();

        assert_eq!(captured.code, None);
        assert_eq!(outcome.code, None);
    }

    #[test]
    fn query_captures_stdout_and_the_exit_code() {
        let system = System {
            program: std::env::current_exe().unwrap().into_os_string(),
        };

        let captured = system.query(&["--list"]).unwrap();

        assert!(!captured.stdout.is_empty());
        assert_eq!(captured.code, Some(0));
    }

    /// Set in the child process that [`the_child_gets_a_null_stdin_and_the_inherited_stderr`]
    /// starts, which tells it to be the probe instead of the test.
    #[cfg(unix)]
    const PROBE_ENV: &str = "AIRSL_LUAROCKS_STDIO_PROBE";

    /// The probe's side of the stdio test: runs a shell through [`System`] and reports what that
    /// shell saw on its stdin, once as captured output and once as inherited output.
    #[cfg(unix)]
    fn probe_stdio() {
        let system = System {
            program: "/bin/sh".into(),
        };
        let script = "read -r line; echo \"stdin:$line\"; echo child-stderr >&2";

        let captured = system.query(&["-c", script]).unwrap();
        println!(
            "captured={}",
            String::from_utf8_lossy(&captured.stdout).trim_end()
        );
        system.run(&["-c", script]).unwrap();
    }

    /// Stdio is not observable from a call's result, so a copy of the test binary is started
    /// with its own stdin (holding a line) and stderr (a pipe) and reports what the shell it
    /// runs saw.
    #[cfg(unix)]
    #[test]
    fn the_child_gets_a_null_stdin_and_the_inherited_stderr() {
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        const NAME: &str = "the_child_gets_a_null_stdin_and_the_inherited_stderr";
        if std::env::var_os(PROBE_ENV).is_some() {
            probe_stdio();
            return;
        }
        // The test harness names a test by its module path without the crate, and `--exact`
        // keeps a later test whose name merely contains this one out of the child.
        let path = module_path!().split_once("::").unwrap().1;
        let mut probe = Command::new(std::env::current_exe().unwrap())
            .args([&format!("{path}::{NAME}"), "--exact", "--nocapture"])
            .env(PROBE_ENV, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        probe.stdin.take().unwrap().write_all(b"typed\n").unwrap();

        let output = probe.wait_with_output().unwrap();

        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stdout.contains("captured=stdin:\n"), "{stdout}");
        assert!(stdout.contains("\nstdin:\n"), "{stdout}");
        assert!(!stdout.contains("typed"), "stdin was not null: {stdout}");
        assert_eq!(stderr.matches("child-stderr").count(), 2, "{stderr}");
    }
}
