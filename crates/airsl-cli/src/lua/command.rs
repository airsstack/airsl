//! `airsl lua`: dispatches a [`LuaCommand`], prints the report and maps failures to an exit code.
//!
//! Its own file because it is the one place that owns the streams and the exit code: the library
//! and the native-flow decisions return values, and this turns them into output. It is a thin
//! caller of [`airsl::luals::setup`]. With `--native` it also runs luarocks, through the
//! [`Luarocks`] boundary, in a fixed order that stops at the first failure: read which Lua
//! luarocks targets, write the type file and `.luarc.json`, print and flush the report, install
//! the native package, then print the `luarocks path` hint. Detection comes first so a luarocks
//! that cannot answer leaves the project untouched, and the report comes before the install so
//! luarocks' own output lands below it. The streams are injected, the shape `ext_fire::run` has,
//! so the tests assert on both.
//!
//! Responsibilities: [`run`], which dispatches a [`LuaCommand`], orders the stages, prints the
//! report and the hint and maps failures to an exit code.
//!
//! Non-responsibilities: deciding what gets written (`airsl::luals`), which package to install
//! and the words of each refusal (`native`), spawning luarocks (`luarocks`) and parsing arguments
//! (`cli`).
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::io::Write;
use std::path::Path;

use airsl::RootTable;
use airsl::luals::SetupReport;
use airsl::modules::stdlib;

use super::luarocks::{Luarocks, System};
use super::native::{Flavour, NativeRequest, hint, install_native, setup_native};
use crate::cli::LuaCommand;

/// Runs one `lua` subcommand, writing its report to `stdout` and any diagnostic to `stderr`, and
/// returns the process exit code.
///
/// Every failure, including a failed write of the report, prints `airsl: {error}` to `stderr` and
/// exits 1. A failed report write comes after both files are written, so it never leaves a
/// half-set-up project behind. Without `--native` luarocks is never run.
pub(crate) fn run(command: LuaCommand, stdout: &mut impl Write, stderr: &mut impl Write) -> i32 {
    run_on_host(command, &System::default(), stdout, stderr)
}

/// [`run`] with the luarocks it uses chosen by the caller: the wording is always the running
/// platform's, [`Flavour::host`].
fn run_on_host<L: Luarocks>(
    command: LuaCommand,
    luarocks: &L,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    run_with(command, luarocks, Flavour::host(), stdout, stderr)
}

/// [`run`] with the luarocks it uses and the wording it prints chosen by the caller.
fn run_with<L: Luarocks>(
    command: LuaCommand,
    luarocks: &L,
    flavour: Flavour,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    let LuaCommand::Setup { dir, native, lua } = command;
    match native.then_some(NativeRequest { lua }) {
        Some(request) => setup_native_stages(&dir, request, luarocks, flavour, stdout, stderr),
        None => match setup(&dir) {
            Ok(report) => match writeln!(stdout, "{report}") {
                Ok(()) => 0,
                Err(error) => fail(stderr, error),
            },
            Err(error) => fail(stderr, error),
        },
    }
}

/// Runs the `--native` stages in order, stopping at the first failure.
///
/// Detection runs before anything is written, so a luarocks that cannot answer leaves the project
/// as it was. The report is flushed before the install, so luarocks' own output, which goes
/// straight to the terminal, appears below it.
fn setup_native_stages<L: Luarocks>(
    dir: &Path,
    request: NativeRequest,
    luarocks: &L,
    flavour: Flavour,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    let package = match setup_native(luarocks, request, flavour) {
        Ok(package) => package,
        Err(error) => return fail(stderr, error),
    };
    let report = match setup(dir) {
        Ok(report) => report,
        Err(error) => return fail(stderr, error),
    };
    if let Err(error) = writeln!(stdout, "{report}").and_then(|()| stdout.flush()) {
        return fail(stderr, error);
    }
    if let Err(error) = install_native(luarocks, request, package, flavour) {
        return fail(stderr, error);
    }
    match writeln!(stdout, "{}", hint(request, flavour)) {
        Ok(()) => 0,
        Err(error) => fail(stderr, error),
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

    use std::cell::{Cell, RefCell};
    use std::io::{self, Write};
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use super::{NativeRequest, hint, run, run_on_host, run_with};
    use crate::cli::{LuaAbi, LuaCommand};
    use crate::lua::luarocks::scripted::Scripted;
    use crate::lua::luarocks::{Captured, Luarocks, LuarocksError, Outcome};
    use crate::lua::native::Flavour;

    const VERSION: &str = airsl::VERSION;
    const REPORT: &str = "created .airsl/types/airsstack.lua\ncreated .luarc.json\n";

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
        LuaCommand::Setup {
            dir,
            native: false,
            lua: None,
        }
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

    /// A stdout that several owners can hold, to see what was written and flushed so far.
    #[derive(Clone, Default)]
    struct SharedOut {
        bytes: Rc<RefCell<Vec<u8>>>,
        flushes: Rc<Cell<usize>>,
    }

    impl SharedOut {
        fn text(&self) -> String {
            String::from_utf8(self.bytes.borrow().clone()).unwrap()
        }
    }

    impl Write for SharedOut {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.bytes.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flushes.set(self.flushes.get() + 1);
            Ok(())
        }
    }

    /// A stdout that accepts everything except a write containing `needle`.
    struct FailsOn(&'static str);

    impl Write for FailsOn {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if String::from_utf8_lossy(buf).contains(self.0) {
                Err(io::Error::other("no room"))
            } else {
                Ok(buf.len())
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A stdout that takes every write and fails every flush.
    struct FailsToFlush;

    impl Write for FailsToFlush {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("no flush"))
        }
    }

    /// A scripted luarocks that, when asked to install, notes what stdout held and how often it
    /// had been flushed at that moment.
    struct Spy {
        inner: Scripted,
        out: SharedOut,
        at_install: RefCell<Option<(String, usize)>>,
    }

    impl Luarocks for Spy {
        fn query(&self, args: &[&str]) -> Result<Captured, LuarocksError> {
            self.inner.query(args)
        }

        fn run(&self, args: &[&str]) -> Result<Outcome, LuarocksError> {
            *self.at_install.borrow_mut() = Some((self.out.text(), self.out.flushes.get()));
            self.inner.run(args)
        }
    }

    fn native_command(dir: &Path, lua: Option<LuaAbi>) -> LuaCommand {
        LuaCommand::Setup {
            dir: dir.to_path_buf(),
            native: true,
            lua,
        }
    }

    fn lua54_then_install(code: Option<i32>) -> Scripted {
        Scripted::default()
            .then_stdout(b"5.4\n")
            .then_run(Ok(Outcome { code }))
    }

    fn run_native(luarocks: &Scripted, dir: &Path, flavour: Flavour) -> (String, String, i32) {
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let code = run_with(
            native_command(dir, None),
            luarocks,
            flavour,
            &mut stdout,
            &mut stderr,
        );
        (
            String::from_utf8(stdout).unwrap(),
            String::from_utf8(stderr).unwrap(),
            code,
        )
    }

    #[test]
    fn native_detection_failure_writes_nothing() {
        let project = tempfile::tempdir().unwrap();
        let luarocks = Scripted::default().then_query(Ok(Captured {
            stdout: b"5.4\n".to_vec(),
            code: Some(1),
        }));

        let (stdout, stderr, code) = run_native(&luarocks, project.path(), Flavour::Unix);

        assert_eq!(
            stderr,
            "airsl: `luarocks config lua_version` failed with exit status 1\n"
        );
        assert!(stdout.is_empty());
        assert_eq!(code, 1);
        assert!(!project.path().join(".airsl").exists());
        assert!(!project.path().join(".luarc.json").exists());
    }

    #[test]
    fn native_on_a_lua_5_1_without_luajit_writes_and_installs_nothing() {
        let project = tempfile::tempdir().unwrap();
        let luarocks = Scripted::default()
            .then_stdout(b"5.1\n")
            .then_stdout(b"{\n   luajit_version_checked = true\n}\n");

        let (stdout, stderr, code) = run_native(&luarocks, project.path(), Flavour::Unix);

        assert!(
            stderr.starts_with(
                "airsl: `luarocks` found no LuaJIT for Lua 5.1: its Lua 5.1 is not LuaJIT, or it \
                 has none, and `airsstack-luajit` needs LuaJIT\n"
            ),
            "{stderr}"
        );
        assert!(stdout.is_empty());
        assert_eq!(code, 1);
        assert!(!project.path().join(".airsl").exists());
        assert!(!project.path().join(".luarc.json").exists());
        assert_eq!(luarocks.calls(), ["config lua_version", "config cache"]);
    }

    #[test]
    fn native_type_file_failure_installs_nothing() {
        let project = tempfile::tempdir().unwrap();
        let luarocks = lua54_then_install(Some(0));

        let (stdout, stderr, code) =
            run_native(&luarocks, &project.path().join("missing"), Flavour::Unix);

        assert!(
            stderr.starts_with("airsl: open project directory failed on "),
            "{stderr}"
        );
        assert!(stdout.is_empty());
        assert_eq!(code, 1);
        assert_eq!(luarocks.calls(), ["config lua_version"]);
    }

    #[test]
    fn native_prints_the_report_before_installing() {
        let project = tempfile::tempdir().unwrap();
        let mut out = SharedOut::default();
        let spy = Spy {
            inner: lua54_then_install(Some(0)),
            out: out.clone(),
            at_install: RefCell::new(None),
        };
        let mut stderr = Vec::new();

        let code = run_with(
            native_command(project.path(), None),
            &spy,
            Flavour::Unix,
            &mut out,
            &mut stderr,
        );

        let (printed, flushes) = spy.at_install.take().unwrap();
        assert_eq!(printed, REPORT);
        assert!(flushes >= 1, "stdout was not flushed before install ran");
        assert_eq!(code, 0);
    }

    #[test]
    fn native_install_failure_keeps_the_report_and_exits_one() {
        let project = tempfile::tempdir().unwrap();
        let luarocks = lua54_then_install(Some(1));

        let (stdout, stderr, code) = run_native(&luarocks, project.path(), Flavour::Unix);

        assert_eq!(stdout, REPORT);
        assert_eq!(
            stderr,
            format!(
                "airsl: `luarocks install --local airsstack {VERSION}` failed with exit status 1\n"
            )
        );
        assert_eq!(code, 1);
        assert!(project.path().join(".airsl/types/airsstack.lua").is_file());
        assert!(project.path().join(".luarc.json").is_file());
    }

    #[test]
    fn an_install_that_exits_non_one_still_exits_one() {
        for status in [2, 7, 127] {
            let project = tempfile::tempdir().unwrap();
            let luarocks = lua54_then_install(Some(status));

            let (stdout, stderr, code) = run_native(&luarocks, project.path(), Flavour::Unix);

            assert_eq!(stdout, REPORT, "status {status}");
            assert_eq!(
                stderr,
                format!(
                    "airsl: `luarocks install --local airsstack {VERSION}` \
                     failed with exit status {status}\n"
                )
            );
            assert_eq!(code, 1, "luarocks' status {status} must not become airsl's");
        }
    }

    #[test]
    fn an_install_ended_by_a_signal_exits_one() {
        let project = tempfile::tempdir().unwrap();
        let luarocks = lua54_then_install(None);

        let (stdout, stderr, code) = run_native(&luarocks, project.path(), Flavour::Unix);

        assert_eq!(stdout, REPORT);
        assert_eq!(
            stderr,
            format!(
                "airsl: `luarocks install --local airsstack {VERSION}` was stopped by a signal\n"
            )
        );
        assert_eq!(code, 1);
    }

    #[test]
    fn luarocks_missing_at_install_keeps_the_report_and_prints_the_install_lines() {
        let project = tempfile::tempdir().unwrap();
        let luarocks = Scripted::default()
            .then_stdout(b"5.4\n")
            .then_run(Err(LuarocksError::Missing));

        let (stdout, stderr, code) = run_native(&luarocks, project.path(), Flavour::Unix);

        assert_eq!(stdout, REPORT);
        assert!(
            stderr.starts_with(
                "airsl: `luarocks` was not found on PATH\n\
                 \x20      install LuaRocks, then run the line for your Lua:\n"
            ),
            "{stderr}"
        );
        assert!(stderr.contains(&format!(
            "       luarocks --lua-version 5.1 install --local airsstack-luajit {VERSION}\n"
        )));
        assert_eq!(code, 1);
    }

    #[test]
    fn native_success_prints_the_report_then_the_hint() {
        for (flavour, hint) in [
            (
                Flavour::Unix,
                "require(\"airsstack\") works once you run: eval \"$(luarocks path)\"",
            ),
            (
                Flavour::Windows,
                "require(\"airsstack\") works once luarocks' paths are set. In cmd:\n  \
                 luarocks path > lrpath.bat\n  \
                 call lrpath.bat\n\
                 In PowerShell:\n  \
                 $env:LUA_PATH = luarocks path --lr-path\n  \
                 $env:LUA_CPATH = luarocks path --lr-cpath",
            ),
        ] {
            let project = tempfile::tempdir().unwrap();
            let luarocks = lua54_then_install(Some(0));

            let (stdout, stderr, code) = run_native(&luarocks, project.path(), flavour);

            assert_eq!(stdout, format!("{REPORT}{hint}\n"));
            assert!(stderr.is_empty());
            assert_eq!(code, 0);
        }
    }

    #[test]
    fn the_wording_of_a_run_is_the_running_platforms() {
        let project = tempfile::tempdir().unwrap();
        let luarocks = lua54_then_install(Some(0));
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());

        let code = run_on_host(
            native_command(project.path(), None),
            &luarocks,
            &mut stdout,
            &mut stderr,
        );

        let request = NativeRequest { lua: None };
        let this = Flavour::host();
        let stdout = String::from_utf8(stdout).unwrap();
        assert_eq!(stdout, format!("{REPORT}{}\n", hint(request, this)));
        let foreign_line = if cfg!(windows) {
            "eval \"$(luarocks path)\""
        } else {
            "In PowerShell:"
        };
        assert!(!stdout.contains(foreign_line));
        assert!(stderr.is_empty());
        assert_eq!(code, 0);
    }

    #[test]
    fn a_failed_report_write_skips_the_install() {
        let project = tempfile::tempdir().unwrap();
        let luarocks = lua54_then_install(Some(0));
        let mut stderr = Vec::new();

        let code = run_with(
            native_command(project.path(), None),
            &luarocks,
            Flavour::Unix,
            &mut FailingWriter,
            &mut stderr,
        );

        assert_eq!(String::from_utf8(stderr).unwrap(), "airsl: boom\n");
        assert_eq!(code, 1);
        assert_eq!(luarocks.calls(), ["config lua_version"]);
    }

    #[test]
    fn a_failed_flush_of_the_report_skips_the_install() {
        let project = tempfile::tempdir().unwrap();
        let luarocks = lua54_then_install(Some(0));
        let mut stderr = Vec::new();

        let code = run_with(
            native_command(project.path(), None),
            &luarocks,
            Flavour::Unix,
            &mut FailsToFlush,
            &mut stderr,
        );

        assert_eq!(String::from_utf8(stderr).unwrap(), "airsl: no flush\n");
        assert_eq!(code, 1);
        assert_eq!(luarocks.calls(), ["config lua_version"]);
    }

    #[test]
    fn a_failed_hint_write_exits_one() {
        let project = tempfile::tempdir().unwrap();
        let luarocks = lua54_then_install(Some(0));
        let mut stderr = Vec::new();

        let code = run_with(
            native_command(project.path(), None),
            &luarocks,
            Flavour::Unix,
            &mut FailsOn("require("),
            &mut stderr,
        );

        assert_eq!(String::from_utf8(stderr).unwrap(), "airsl: no room\n");
        assert_eq!(code, 1);
        assert_eq!(luarocks.calls().len(), 2, "install ran before the hint");
    }

    #[test]
    fn without_native_luarocks_is_never_run() {
        let project = tempfile::tempdir().unwrap();
        let luarocks = Scripted::default();
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());

        let code = run_with(
            setup_command(project.path().to_path_buf()),
            &luarocks,
            Flavour::Unix,
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(String::from_utf8(stdout).unwrap(), REPORT);
        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        assert!(luarocks.calls().is_empty());
    }
}
