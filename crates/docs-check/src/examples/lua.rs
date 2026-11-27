//! The Lua examples of the native module against their READMEs.
//!
//! Its own module because these examples are run by a different mechanism from the Rust ones: not
//! through cargo, but by an interpreter that loads the module `cargo make native-load` built into
//! `target/native/<abi>/`. They live in `crates/airsl-lua/examples/<group>/<name>/main.lua`, and the
//! group says which interpreters run an example, because a Lua 5.4 module does not load into
//! `LuaJIT` and the reverse:
//!
//! - `common`: written to behave the same under both, and run under both. The one `## Output`
//!   block is what each prints, so a difference between the two interpreters fails the example.
//! - `lua54`: Lua 5.4 only, for what `LuaJIT` cannot express.
//! - `luajit`: `LuaJIT` only, for what Lua 5.4 does differently.
//! - `neovim`: Neovim's own `LuaJIT`, run with `nvim -l`.
//!
//! The first three run in the gate. Neovim does not: a developer running `cargo make dod` needs no
//! editor, so its test is ignored, and `cargo make nvim-examples` runs it by name with the
//! module built first.
//!
//! Interpreters come from `AIRSL_LUA54` and `AIRSL_LUAJIT`, with the defaults `native-load` uses,
//! and Neovim from `AIRSL_NVIM`. A missing interpreter or a missing built module fails with the
//! command that fixes it and never skips, so a green run means every example ran. The module is
//! put on the interpreter's search path through `LUA_CPATH` rather than by the example, so an
//! example reads as it would for a reader who installed the rock.
//!
//! Responsibilities:
//!
//! - Finding the examples of each group and refusing a layout that would drop one silently.
//! - Running each example under the interpreters of its group and comparing its stdout with its
//!   README's `## Output` block for the current platform.
//! - Refusing a missing interpreter, a missing built module or a checkout path `LUA_CPATH` cannot
//!   carry, naming the fix and reporting it apart from README drift.
//! - Requiring each example to have run under every interpreter of its group.
//!
//! Non-responsibilities: reading a README, which `readme` does; building the module, which
//! `native-load` does; and the Rust examples, which `rust` runs.

use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::readme::{Expectation, Platform, expectation};
use crate::workspace::normalize;

/// What a group of examples is run under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Runtime {
    /// Stock Lua 5.4, with the `lua54` module.
    Lua54,
    /// Stock `LuaJIT`, with the `luajit` module.
    Luajit,
    /// Neovim's embedded `LuaJIT`, with the `luajit` module.
    Neovim,
}

impl Runtime {
    /// What to call it in a message.
    const fn name(self) -> &'static str {
        match self {
            Self::Lua54 => "Lua 5.4",
            Self::Luajit => "LuaJIT",
            Self::Neovim => "Neovim",
        }
    }

    /// The environment variable that names its program.
    const fn variable(self) -> &'static str {
        match self {
            Self::Lua54 => "AIRSL_LUA54",
            Self::Luajit => "AIRSL_LUAJIT",
            Self::Neovim => "AIRSL_NVIM",
        }
    }

    /// The program run when the variable is not set.
    const fn default_program(self) -> &'static str {
        match self {
            Self::Lua54 => "lua5.4",
            Self::Luajit => "luajit",
            Self::Neovim => "nvim",
        }
    }

    /// The directory under `target/native` that holds the module this runtime loads.
    const fn abi(self) -> &'static str {
        match self {
            Self::Lua54 => "lua54",
            Self::Luajit | Self::Neovim => "luajit",
        }
    }

    /// The program to run: what the variable holds, or the default when it is unset or empty.
    fn program(self, configured: Option<OsString>) -> OsString {
        configured
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| OsString::from(self.default_program()))
    }

    /// The program for this process's environment.
    fn program_from_env(self) -> OsString {
        self.program(std::env::var_os(self.variable()))
    }

    /// The command that runs `script` with the module on the search path `cpath` names.
    ///
    /// `cpath` ends in `;;`, which Lua replaces with its default path, so the interpreter's own
    /// libraries stay reachable. Lua 5.4 reads `LUA_CPATH_5_4` first when it is set, so that one
    /// is set to the same value. `LUA_INIT` and `LUA_PATH`, and their 5.4 forms, are removed:
    /// either could run code or add a search path of the developer's own, such as the rock they
    /// installed, and the example would then load that instead of the module under test.
    fn command(self, program: &OsStr, script: &Path, cpath: &str) -> Command {
        let mut command = Command::new(program);
        if self == Self::Neovim {
            // `-u NONE` loads no user configuration, so the run does not depend on the machine's.
            command.args(["--headless", "-u", "NONE", "-l"]);
        }
        command.arg(script).env("LUA_CPATH", cpath);
        if self == Self::Lua54 {
            command.env("LUA_CPATH_5_4", cpath);
        }
        for inherited in ["LUA_INIT", "LUA_INIT_5_4", "LUA_PATH", "LUA_PATH_5_4"] {
            command.env_remove(inherited);
        }
        command
    }

    /// The message for a program that is not there, naming how to supply it.
    fn not_found(self, program: &OsStr) -> String {
        format!(
            "{} was not found as `{}`; install {} or set {} to its program",
            self.name(),
            program.to_string_lossy(),
            self.name(),
            self.variable()
        )
    }
}

/// The `LUA_CPATH` value that finds the module in `native`, with the interpreter's defaults kept.
///
/// `;` separates entries and `?` stands for the module name, so a directory holding either would
/// be read as two entries or as a pattern, and one that is not UTF-8 cannot be put in the value
/// at all. Each is refused rather than turned into a path that loads nothing.
fn cpath(native: &Path) -> Result<String, String> {
    let Some(dir) = native.to_str() else {
        return Err(format!(
            "{} is not valid UTF-8, so it cannot go in LUA_CPATH; check the repository out at another path",
            native.display()
        ));
    };
    if dir.contains([';', '?']) {
        return Err(format!(
            "{dir} contains `;` or `?`, which LUA_CPATH reads as separators and patterns; check the repository out at another path"
        ));
    }
    Ok(format!("{dir}/?.so;{dir}/?.dll;;"))
}

/// The directory `cargo make native-load` copies the module for `runtime` into.
fn native_dir(root: &Path, runtime: Runtime) -> PathBuf {
    root.join("target").join("native").join(runtime.abi())
}

/// Why the built module is missing from `native`, or `None` when it is there.
fn missing_module(native: &Path) -> Option<String> {
    let built = ["airsstack.so", "airsstack.dll"]
        .iter()
        .any(|name| native.join(name).is_file());
    (!built).then(|| {
        format!(
            "the built module is not in {}; run `cargo make native-load` to build it",
            native.display()
        )
    })
}

/// One group of examples, a directory under `crates/airsl-lua/examples`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    /// Behaves the same under Lua 5.4 and `LuaJIT`.
    Common,
    /// Lua 5.4 only.
    Lua54,
    /// `LuaJIT` only.
    Luajit,
    /// Neovim only.
    Neovim,
}

impl Group {
    /// Every group, in the order they are checked.
    const ALL: [Self; 4] = [Self::Common, Self::Lua54, Self::Luajit, Self::Neovim];

    /// The groups the gate runs: all but the one that needs an editor.
    const GATED: [Self; 3] = [Self::Common, Self::Lua54, Self::Luajit];

    /// The directory name.
    const fn dir(self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::Lua54 => "lua54",
            Self::Luajit => "luajit",
            Self::Neovim => "neovim",
        }
    }

    /// What its examples are run under.
    const fn runtimes(self) -> &'static [Runtime] {
        match self {
            Self::Common => &[Runtime::Lua54, Runtime::Luajit],
            Self::Lua54 => &[Runtime::Lua54],
            Self::Luajit => &[Runtime::Luajit],
            Self::Neovim => &[Runtime::Neovim],
        }
    }
}

/// The example directories of `group` under `examples`: those that hold a `main.lua`, sorted.
fn lua_example_dirs(examples: &Path, group: Group) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(examples.join(group.dir())) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("main.lua").is_file())
        .collect();
    dirs.sort();
    dirs
}

/// What is wrong with the shape of the `examples` directory, one line each; empty when nothing is.
///
/// An example is found by its `main.lua`, so a directory without one, or one outside a group,
/// would be skipped without a word. This is what turns that into a failure.
fn layout_problems(examples: &Path) -> String {
    let mut report = String::new();
    let Ok(entries) = fs::read_dir(examples) else {
        return format!("{} cannot be read\n", examples.display());
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for name in &names {
        if !Group::ALL.iter().any(|group| group.dir() == name) {
            let _ = writeln!(
                report,
                "`{name}` is not a group of examples; the groups are common, lua54, luajit and neovim"
            );
        }
    }
    for group in Group::ALL {
        let dir = examples.join(group.dir());
        let Ok(entries) = fs::read_dir(&dir) else {
            let _ = writeln!(report, "the group `{}` has no directory", group.dir());
            continue;
        };
        let mut stray: Vec<String> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir() && !path.join("main.lua").is_file())
            .filter_map(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        stray.sort();
        for name in stray {
            let _ = writeln!(report, "`{}/{name}` has no main.lua", group.dir());
        }
    }
    report
}

/// Why running a command did not give a stdout to compare.
#[derive(Debug, PartialEq, Eq)]
enum RunError {
    /// The program is not installed; every example of the runtime would fail the same way.
    NotFound(String),
    /// It ran and failed, or could not be started for another reason.
    Failed(String),
}

/// Runs `command` for `runtime` and returns its stdout with line endings folded.
fn execute(runtime: Runtime, mut command: Command) -> Result<String, RunError> {
    let ran = command.output().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            RunError::NotFound(runtime.not_found(command.get_program()))
        } else {
            RunError::Failed(format!("could not run `{command:?}`: {error}"))
        }
    })?;
    if !ran.status.success() {
        return Err(RunError::Failed(format!(
            "`{command:?}` exited with {}\n--- stderr\n{}",
            ran.status,
            String::from_utf8_lossy(&ran.stderr)
        )));
    }
    Ok(normalize(&String::from_utf8_lossy(&ran.stdout)))
}

/// What running a group found.
#[derive(Debug, Default)]
struct Report {
    /// How many runs compared a stdout with a block.
    checked: usize,
    /// One entry per runtime that could not run at all: a missing interpreter, a missing built
    /// module or a checkout path `LUA_CPATH` cannot carry. None of these is drift in a README.
    unrunnable: Vec<String>,
    /// One entry per example and runtime that drifted or failed when run; empty when none did.
    failures: String,
}

/// The text of everything wrong in `reports`, empty when nothing is.
///
/// A runtime two groups need is unrunnable in both, and is listed once, under a heading of its
/// own, so a missing interpreter is not read as a README that drifted.
fn problems(reports: &[Report]) -> String {
    let mut unrunnable: Vec<&str> = Vec::new();
    for why in reports.iter().flat_map(|report| &report.unrunnable) {
        if !unrunnable.contains(&why.as_str()) {
            unrunnable.push(why);
        }
    }
    let mut text = String::new();
    if !unrunnable.is_empty() {
        text.push_str("Examples that could not run:\n");
        for why in unrunnable {
            let _ = writeln!(text, "{why}");
        }
    }
    let drift: String = reports
        .iter()
        .map(|report| report.failures.as_str())
        .collect();
    if !drift.is_empty() {
        let _ = write!(text, "README drift:\n{drift}");
    }
    text
}

/// Runs every example of `group` under each of its runtimes and compares the stdout with the
/// README block for `platform`.
///
/// What stops a runtime from running at all goes to `unrunnable`, once per runtime, and what an
/// example did wrong to `failures`.
fn check_group(root: &Path, examples: &Path, group: Group, platform: Platform) -> Report {
    let dirs = lua_example_dirs(examples, group);
    let mut report = Report::default();
    // A runtime whose program or built module is missing, or whose search path cannot be written,
    // fails every example the same way, so it is reported once and left out of the rest.
    let mut ready: Vec<(Runtime, String)> = Vec::new();
    for &runtime in group.runtimes() {
        let native = native_dir(root, runtime);
        match missing_module(&native).map_or_else(|| cpath(&native), Err) {
            Ok(value) => ready.push((runtime, value)),
            Err(why) => report
                .unrunnable
                .push(format!("{} examples: {why}", runtime.name())),
        }
    }
    for dir in &dirs {
        let name = format!(
            "{}/{}",
            group.dir(),
            dir.file_name().unwrap_or_default().to_string_lossy()
        );
        let expected = match expectation(dir, platform) {
            Expectation::Output(block) => block,
            Expectation::Exempt => continue,
            Expectation::Unstated(why) => {
                let _ = writeln!(report.failures, "example `{name}`: {why}");
                continue;
            }
        };
        if platform == Platform::Windows && dir.join("unix-only").is_file() {
            continue;
        }
        for &runtime in group.runtimes() {
            let Some(at) = ready.iter().position(|(known, _)| *known == runtime) else {
                continue;
            };
            let program = runtime.program_from_env();
            let command = runtime.command(&program, &dir.join("main.lua"), &ready[at].1);
            match execute(runtime, command) {
                Ok(printed) => {
                    report.checked += 1;
                    let printed = printed.trim_end_matches('\n');
                    if printed != expected {
                        let _ = write!(
                            report.failures,
                            "example `{name}` under {}: its output differs from the `## Output` block for {platform:?} in {}\n--- README shows\n{expected}\n--- example printed\n{printed}\n",
                            runtime.name(),
                            dir.join("README.md").display(),
                        );
                    }
                }
                Err(RunError::NotFound(why)) => {
                    report.unrunnable.push(why);
                    ready.remove(at);
                }
                Err(RunError::Failed(why)) => {
                    let _ = writeln!(
                        report.failures,
                        "example `{name}` under {}: {why}",
                        runtime.name()
                    );
                }
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "fixtures and the checkout under test are known to exist; a panic is the failure signal"
    )]

    use super::super::readme::{marked, unlisted_markers};
    use super::*;

    use crate::workspace::root;

    fn examples_dir() -> PathBuf {
        root().join("crates/airsl-lua/examples")
    }

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
    }

    #[test]
    fn a_group_is_run_under_the_interpreters_it_is_written_for() {
        assert_eq!(Group::Common.runtimes(), [Runtime::Lua54, Runtime::Luajit]);
        assert_eq!(Group::Lua54.runtimes(), [Runtime::Lua54]);
        assert_eq!(Group::Luajit.runtimes(), [Runtime::Luajit]);
        assert_eq!(Group::Neovim.runtimes(), [Runtime::Neovim]);
        assert!(!Group::GATED.contains(&Group::Neovim));
    }

    #[test]
    fn neovim_loads_the_luajit_module_and_lua_54_its_own() {
        assert_eq!(Runtime::Neovim.abi(), "luajit");
        assert_eq!(Runtime::Luajit.abi(), "luajit");
        assert_eq!(Runtime::Lua54.abi(), "lua54");
        assert_eq!(
            native_dir(Path::new("/w"), Runtime::Lua54),
            Path::new("/w/target/native/lua54")
        );
    }

    #[test]
    fn the_program_is_the_variable_or_the_default_when_it_is_unset_or_empty() {
        assert_eq!(Runtime::Lua54.program(None), "lua5.4");
        assert_eq!(Runtime::Luajit.program(None), "luajit");
        assert_eq!(Runtime::Neovim.program(None), "nvim");
        assert_eq!(Runtime::Neovim.program(Some(OsString::new())), "nvim");
        assert_eq!(
            Runtime::Luajit.program(Some(OsString::from("/opt/lj/bin/luajit"))),
            "/opt/lj/bin/luajit"
        );
    }

    /// The arguments of `command`, and its environment changes: `Some` for a variable it sets and
    /// `None` for one it removes.
    fn args_and_env(command: &Command) -> (Vec<String>, Vec<(String, Option<String>)>) {
        let args = command
            .get_args()
            .map(|arg| arg.to_str().unwrap().to_owned())
            .collect();
        let envs = command
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_str().unwrap().to_owned(),
                    value.map(|value| value.to_str().unwrap().to_owned()),
                )
            })
            .collect();
        (args, envs)
    }

    /// The variables of `envs` that are set, as a sorted list of pairs.
    fn set_vars(envs: &[(String, Option<String>)]) -> Vec<(&str, &str)> {
        let mut set: Vec<_> = envs
            .iter()
            .filter_map(|(key, value)| Some((key.as_str(), value.as_deref()?)))
            .collect();
        set.sort_unstable();
        set
    }

    /// The variables of `envs` that are removed, sorted.
    fn removed_vars(envs: &[(String, Option<String>)]) -> Vec<&str> {
        let mut removed: Vec<_> = envs
            .iter()
            .filter(|(_, value)| value.is_none())
            .map(|(key, _)| key.as_str())
            .collect();
        removed.sort_unstable();
        removed
    }

    const WANT_CPATH: &str = "/w/target/native/lua54/?.so;/w/target/native/lua54/?.dll;;";

    #[test]
    fn the_command_puts_the_built_module_on_the_search_path_and_keeps_the_defaults() {
        let script = Path::new("/w/ex/main.lua");
        let command = Runtime::Lua54.command(OsStr::new("lua5.4"), script, WANT_CPATH);
        let (args, envs) = args_and_env(&command);
        assert_eq!(args, ["/w/ex/main.lua"]);
        assert_eq!(
            set_vars(&envs),
            [("LUA_CPATH", WANT_CPATH), ("LUA_CPATH_5_4", WANT_CPATH)]
        );

        let command = Runtime::Luajit.command(OsStr::new("luajit"), script, WANT_CPATH);
        let (args, envs) = args_and_env(&command);
        assert_eq!(args, ["/w/ex/main.lua"]);
        assert_eq!(set_vars(&envs), [("LUA_CPATH", WANT_CPATH)]);
    }

    #[test]
    fn an_inherited_init_or_search_path_cannot_redirect_a_run_to_an_installed_module() {
        for runtime in [Runtime::Lua54, Runtime::Luajit, Runtime::Neovim] {
            let command = runtime.command(OsStr::new("x"), Path::new("/w/ex/main.lua"), "c");
            let (_, envs) = args_and_env(&command);
            assert_eq!(
                removed_vars(&envs),
                ["LUA_INIT", "LUA_INIT_5_4", "LUA_PATH", "LUA_PATH_5_4"],
                "{}",
                runtime.name()
            );
        }
    }

    #[test]
    fn the_search_path_names_the_native_directory_and_keeps_the_defaults() {
        assert_eq!(
            cpath(Path::new("/w/target/native/lua54")).as_deref(),
            Ok(WANT_CPATH)
        );
    }

    #[test]
    fn a_checkout_path_that_would_corrupt_the_search_path_is_refused() {
        for bad in ["/w;x/native", "/w?/native"] {
            let why = cpath(Path::new(bad)).unwrap_err();
            assert!(why.contains(bad), "{why}");
            assert!(why.contains("`;` or `?`"), "{why}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_checkout_path_that_is_not_utf_8_is_refused() {
        use std::os::unix::ffi::OsStrExt;
        let path = Path::new(OsStr::from_bytes(b"/w/\xff/native"));
        let why = cpath(path).unwrap_err();
        assert!(why.contains("not valid UTF-8"), "{why}");
    }

    #[test]
    fn neovim_runs_the_script_headless_with_no_user_configuration() {
        let command = Runtime::Neovim.command(OsStr::new("nvim"), Path::new("/w/ex/main.lua"), "c");
        let (args, _) = args_and_env(&command);
        assert_eq!(args, ["--headless", "-u", "NONE", "-l", "/w/ex/main.lua"]);
    }

    #[test]
    fn a_missing_built_module_names_the_task_that_builds_it() {
        let tmp = tempfile::tempdir().unwrap();
        let why = missing_module(tmp.path()).unwrap();
        assert!(why.contains("cargo make native-load"), "{why}");
        assert!(why.contains(&tmp.path().display().to_string()), "{why}");

        touch(&tmp.path().join("airsstack.so"));
        assert_eq!(missing_module(tmp.path()), None);
        fs::remove_file(tmp.path().join("airsstack.so")).unwrap();
        touch(&tmp.path().join("airsstack.dll"));
        assert_eq!(missing_module(tmp.path()), None);
    }

    #[test]
    fn a_missing_interpreter_names_the_variable_that_points_at_one() {
        let why = Runtime::Lua54.not_found(OsStr::new("lua5.4"));
        assert_eq!(
            why,
            "Lua 5.4 was not found as `lua5.4`; install Lua 5.4 or set AIRSL_LUA54 to its program"
        );
        let command = Command::new("airsl-no-such-interpreter");
        assert_eq!(
            execute(Runtime::Neovim, command),
            Err(RunError::NotFound(
                "Neovim was not found as `airsl-no-such-interpreter`; install Neovim or set AIRSL_NVIM to its program"
                    .to_owned()
            ))
        );
    }

    #[test]
    fn a_run_that_exits_non_zero_is_a_failure_that_shows_its_stderr() {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut command = Command::new(cargo);
        command.arg("airsl-no-such-subcommand");
        let error = execute(Runtime::Lua54, command).unwrap_err();
        assert!(
            matches!(&error, RunError::Failed(why) if why.contains("exited with") && why.contains("--- stderr")),
            "{error:?}"
        );
    }

    #[test]
    fn only_directories_with_a_main_lua_are_the_examples_of_a_group() {
        let tmp = tempfile::tempdir().unwrap();
        for rel in [
            "common/b/main.lua",
            "common/a/main.lua",
            "common/notes/README.md",
            "lua54/c/main.lua",
        ] {
            touch(&tmp.path().join(rel));
        }
        let names: Vec<_> = lua_example_dirs(tmp.path(), Group::Common)
            .iter()
            .map(|dir| dir.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a", "b"]);
        assert!(lua_example_dirs(tmp.path(), Group::Neovim).is_empty());
    }

    #[test]
    fn a_layout_with_a_stray_directory_or_a_missing_main_lua_is_reported() {
        let tmp = tempfile::tempdir().unwrap();
        for group in Group::ALL {
            touch(&tmp.path().join(group.dir()).join("ok/main.lua"));
        }
        assert_eq!(layout_problems(tmp.path()), "");

        touch(&tmp.path().join("lua55/x/main.lua"));
        touch(&tmp.path().join("common/renamed/main.luau"));
        fs::remove_dir_all(tmp.path().join("neovim")).unwrap();
        let report = layout_problems(tmp.path());
        assert!(report.contains("`lua55` is not a group"), "{report}");
        assert!(
            report.contains("`common/renamed` has no main.lua"),
            "{report}"
        );
        assert!(
            report.contains("the group `neovim` has no directory"),
            "{report}"
        );
    }

    #[test]
    fn the_examples_directory_holds_the_four_groups_and_nothing_else() {
        let report = layout_problems(&examples_dir());
        assert!(report.is_empty(), "{report}");
        for group in Group::ALL {
            assert!(
                !lua_example_dirs(&examples_dir(), group).is_empty(),
                "the group `{}` has no example",
                group.dir()
            );
        }
    }

    #[test]
    fn no_lua_example_is_excused_from_its_output_or_from_windows() {
        let dirs: Vec<PathBuf> = Group::ALL
            .iter()
            .flat_map(|&group| lua_example_dirs(&examples_dir(), group))
            .collect();
        let report = unlisted_markers(&marked(&dirs), &[], &[]);
        assert!(report.is_empty(), "{report}");
    }

    #[test]
    fn a_group_reports_a_missing_built_module_instead_of_skipping() {
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("examples/lua54/a/main.lua"));
        let report = check_group(
            tmp.path(),
            &tmp.path().join("examples"),
            Group::Lua54,
            Platform::Other,
        );
        assert_eq!(report.checked, 0);
        assert_eq!(report.unrunnable.len(), 1, "{:?}", report.unrunnable);
        assert!(
            report.unrunnable[0].starts_with("Lua 5.4 examples: the built module is not in"),
            "{:?}",
            report.unrunnable
        );
        assert!(report.unrunnable[0].contains("cargo make native-load"));
    }

    #[test]
    fn a_checkout_path_that_corrupts_the_search_path_is_unrunnable_not_drift() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = tmp.path().join("a;b");
        touch(&bad.join("examples/lua54/a/main.lua"));
        touch(&bad.join("target/native/lua54/airsstack.so"));
        let report = check_group(&bad, &bad.join("examples"), Group::Lua54, Platform::Other);
        assert_eq!(report.checked, 0);
        assert_eq!(report.unrunnable.len(), 1, "{:?}", report.unrunnable);
        assert!(report.unrunnable[0].contains("`;` or `?`"));
    }

    fn report(unrunnable: &[&str], failures: &str) -> Report {
        Report {
            checked: 0,
            unrunnable: unrunnable.iter().map(|&why| why.to_owned()).collect(),
            failures: failures.to_owned(),
        }
    }

    #[test]
    fn an_unrunnable_runtime_is_listed_once_under_its_own_heading_apart_from_drift() {
        let both = [
            report(&["Lua 5.4 was not found"], ""),
            report(&["Lua 5.4 was not found"], "example `a`: differs\n"),
        ];
        let text = problems(&both);
        assert_eq!(text.matches("Lua 5.4 was not found").count(), 1, "{text}");
        let heading = text.find("Examples that could not run:").unwrap();
        let drift = text.find("README drift:").unwrap();
        assert!(heading < drift, "{text}");
        assert!(text.contains("example `a`: differs"), "{text}");

        assert_eq!(problems(&[report(&[], "")]), "");
        let only_missing = problems(&[report(&["x"], "")]);
        assert!(!only_missing.contains("README drift:"), "{only_missing}");
    }

    #[test]
    fn a_group_reports_an_example_without_an_output_block() {
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("examples/luajit/a/main.lua"));
        touch(&tmp.path().join("target/native/luajit/airsstack.so"));
        let report = check_group(
            tmp.path(),
            &tmp.path().join("examples"),
            Group::Luajit,
            Platform::Other,
        );
        assert!(
            report
                .failures
                .contains("example `luajit/a`: it has no README.md"),
            "{}",
            report.failures
        );
    }

    /// How many runs `group` must make: each of its examples under each of its interpreters, so a
    /// `common` example that ran under only one of them is short of it.
    fn runs_expected(group: Group) -> usize {
        lua_example_dirs(&examples_dir(), group).len() * group.runtimes().len()
    }

    /// Runs `groups` against the checkout and requires every example to have run under every
    /// interpreter it is for, with its output equal to its README's.
    fn assert_groups_run_and_match(groups: &[Group]) {
        let reports: Vec<Report> = groups
            .iter()
            .map(|&group| check_group(&root(), &examples_dir(), group, Platform::current()))
            .collect();
        let text = problems(&reports);
        assert!(text.is_empty(), "{text}");
        for (&group, report) in groups.iter().zip(&reports) {
            let wanted = runs_expected(group);
            assert!(wanted > 0, "the group `{}` has no example", group.dir());
            assert_eq!(
                report.checked,
                wanted,
                "the group `{}` did not run every example under every interpreter it is for",
                group.dir()
            );
        }
    }

    #[test]
    fn every_lua_example_readme_output_block_equals_what_it_prints() {
        assert_groups_run_and_match(&Group::GATED);
    }

    #[test]
    #[ignore = "needs Neovim; run by `cargo make nvim-examples`"]
    fn every_neovim_example_readme_output_block_equals_what_it_prints() {
        assert_groups_run_and_match(&[Group::Neovim]);
    }

    /// The test path `Makefile.toml`'s `nvim-examples` task selects: the argument after `--exact`
    /// on its `cargo test` line.
    fn nvim_task_test(makefile: &str) -> Option<String> {
        let task = makefile.split("[tasks.nvim-examples]").nth(1)?;
        let task = task.split("\n[tasks.").next()?;
        let line = task
            .lines()
            .find(|line| line.contains("exec ") && line.contains("cargo test"))?;
        let mut words = line.split_whitespace();
        words.find(|&word| word == "--exact")?;
        words.next().map(str::to_owned)
    }

    /// Whether `source` has a `#[test]` function `name` marked `#[ignore]`.
    fn has_ignored_test(source: &str, name: &str) -> bool {
        let lines: Vec<&str> = source.lines().map(str::trim).collect();
        let header = format!("fn {name}(");
        lines.iter().enumerate().any(|(at, line)| {
            let attributes: Vec<&&str> = lines[..at]
                .iter()
                .rev()
                .take_while(|before| before.starts_with("#["))
                .collect();
            line.starts_with(&header)
                && attributes.iter().any(|a| **a == "#[test]")
                && attributes.iter().any(|a| a.starts_with("#[ignore"))
        })
    }

    #[test]
    fn the_nvim_task_path_is_the_argument_after_exact() {
        let makefile = "[tasks.other]\nexec cargo test -- --exact other::x\n\n\
            [tasks.nvim-examples]\ndescription = \"x\"\nscript = '''\n\
            exec --fail-on-error cargo make native-load\n\
            out = exec cargo test --lib -- --ignored --exact a::b::the_test\n'''\n\n\
            [tasks.next]\nexec cargo test -- --exact next::y\n";
        assert_eq!(nvim_task_test(makefile).as_deref(), Some("a::b::the_test"));
        assert_eq!(
            nvim_task_test("[tasks.nvim-examples]\nexec cargo test -- t"),
            None
        );
    }

    #[test]
    fn an_ignored_test_is_found_by_its_name_and_attributes() {
        let source = "    #[test]\n    #[ignore = \"why\"]\n    fn the_one() {\n    }\n\n\
            \x20   #[test]\n    fn not_ignored() {\n    }\n\n    #[ignore]\n    fn no_test() {}\n";
        assert!(has_ignored_test(source, "the_one"));
        assert!(!has_ignored_test(source, "not_ignored"));
        assert!(!has_ignored_test(source, "no_test"));
        assert!(!has_ignored_test(source, "absent"));
    }

    #[test]
    fn the_nvim_examples_task_names_the_ignored_test_it_exists_to_run() {
        let makefile = fs::read_to_string(root().join("Makefile.toml")).unwrap();
        let path = nvim_task_test(&makefile);
        assert!(
            path.is_some(),
            "the `nvim-examples` task of Makefile.toml selects no test with `--exact`"
        );
        let path = path.unwrap();
        let name = path.strip_prefix("examples::lua::tests::");
        assert!(
            name.is_some(),
            "`{path}`, the test `nvim-examples` runs, is not a test of examples::lua::tests"
        );
        let name = name.unwrap();
        let source =
            fs::read_to_string(root().join("crates/docs-check/src/examples/lua.rs")).unwrap();
        assert!(
            has_ignored_test(&source, name),
            "`{name}`, the test `nvim-examples` runs, is not an #[ignore] test of lua.rs; a rename would make the task run nothing"
        );
    }
}
