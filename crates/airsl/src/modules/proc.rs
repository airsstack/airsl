//! The `airsstack.proc` host module.
//!
//! Exists mainly to make one thing unrepresentable: there is no string form of `run`. It takes an
//! argv array, so there is no shell, no word splitting and no quoting bug to have. `io.popen`
//! takes a shell string, and that alone is the strongest reason to prefer this module over it even
//! under a policy where both are available.
//!
//! Responsibilities: [`Proc`], installing `run` and `which`.
//!
//! Non-responsibilities: deciding which executables are allowed ([`crate::ProcGrant`]), and making
//! the allowlist mean more than it does — see the note on `PATH` below.
//!
//! `run`'s `status` field means slightly different things per platform, and both are the raw
//! operating-system value narrowed to a Lua number rather than anything this module interprets. On
//! unix it is the exit code, or `-1` standing in for the case where a signal ended the process
//! instead — a signalled process has no exit code, and reporting `-1` keeps the field a number so
//! `result.status ~= 0` stays the one portable way to ask whether the program worked. On Windows
//! there is no such thing as a signalled process, `ExitStatus::code()` always returns `Some`, and
//! `-1` there is a legitimate exit code a process chose (`ExitProcess(0xFFFFFFFF)` narrowed to
//! `i32`), not this module's sentinel — the raw 32-bit exit code, narrowed the same way.

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::modules::env;
use crate::modules::{HostModule, InstallContext};
use crate::paths::rules::native;
use crate::sandbox::GrantSet;
use crate::types::ModuleName;

/// Installs `airsstack.proc`.
#[derive(Debug)]
pub struct Proc {
    name: ModuleName,
}

impl Proc {
    /// Builds the module.
    ///
    /// # Panics
    ///
    /// Never in practice: the name is a literal that satisfies [`ModuleName`]'s rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: ModuleName::new("proc")
                .unwrap_or_else(|_| unreachable!("`proc` is a valid module name")),
        }
    }
}

impl Default for Proc {
    fn default() -> Self {
        Self::new()
    }
}

/// The refusal for a program the policy does not cover.
fn denied(grants: &GrantSet, operation: &'static str, program: &str) -> Error {
    let allowed: Vec<_> = grants.proc().executables().collect();
    let detail = if allowed.is_empty() {
        format!("`{program}` is not granted — no executables are")
    } else {
        format!(
            "`{program}` is not granted — the allowed executables are {}",
            allowed.join(", ")
        )
    };
    Error::Denied {
        module: "proc",
        operation,
        detail,
    }
}

/// The argv a script passed, with the program separated from its arguments.
fn argv(command: &mlua::Table) -> mlua::Result<(String, Vec<String>)> {
    let mut parts = Vec::new();
    for value in command.sequence_values::<mlua::LuaString>() {
        parts.push(value?.to_str()?.to_owned());
    }
    let mut parts = parts.into_iter();
    let program = parts.next().ok_or_else(|| {
        mlua::Error::from(Error::Denied {
            module: "proc",
            operation: "run",
            detail: String::from("the argv array is empty, so there is no program to run"),
        })
    })?;
    Ok((program, parts.collect()))
}

impl HostModule for Proc {
    fn name(&self) -> &ModuleName {
        &self.name
    }

    fn install(
        &self,
        lua: &mlua::Lua,
        table: &mlua::Table,
        context: &InstallContext<'_>,
    ) -> Result<()> {
        let fail = |e: mlua::Error| Error::ModuleInstall {
            module: String::from("proc"),
            reason: e.to_string(),
        };
        let grants = Arc::new(context.grants().clone());
        let overlay = env::overlay();

        let (g, o) = (Arc::clone(&grants), Arc::clone(&overlay));
        let run = lua
            .create_function(move |lua, command: mlua::Table| {
                let (program, arguments) = argv(&command)?;
                if !g.is_unrestricted() && !g.proc().allows(&program) {
                    return Err(mlua::Error::from(denied(&g, "run", &program)));
                }

                // Windows only: `Command::new` does not search `PATH` for a bare program name the
                // way `execvp` does on unix, so a bare name is resolved through the same `which`
                // `airsstack.proc.which` uses, before spawning. A name that already contains a
                // separator is passed through unresolved, exactly as it is written.
                //
                // Doing this only on Windows, rather than for both platforms, is the fix rather
                // than a carve-out: pre-resolving on unix would change three behaviours, none of
                // which buys anything there. An unset `PATH` would stop falling back to
                // `_CS_PATH`, which `execvp` does and `which` does not — `which` returns `None`
                // the moment the `PATH` overlay entry is absent. A `PATH` entry carrying some
                // execute bit but none for the calling user would be selected by the mode-bit
                // check rather than skipped by the kernel in favour of a later entry. And a
                // missing program's error would move from spawn-time `Error::Io` to
                // pre-resolution time, changing when a script sees it.
                //
                // The one consequence this shadowing does have on Windows: the `path` field on a
                // spawn-time `Error::Io` below now names the resolved absolute path rather than
                // the name as written. That is an improvement in a case that is close to
                // unreachable — pre-resolution has already proved the file exists, so a spawn-time
                // `NotFound` on Windows means the file was removed between the two calls.
                #[cfg(windows)]
                let program = if native::has_separator(&program) {
                    program
                } else {
                    which(&program).ok_or_else(|| mlua::Error::from(unresolvable(&program)))?
                };

                let mut child = std::process::Command::new(&program);
                child.args(&arguments);
                // The child sees the same environment overlay the script does, so a variable set
                // through `env.set` reaches the process it was set for.
                for (name, value) in overlay_entries(&o) {
                    match value {
                        Some(value) => child.env(name, value),
                        None => child.env_remove(name),
                    };
                }

                let output = child.output().map_err(|source| Error::Io {
                    operation: "run",
                    path: program.clone(),
                    source,
                })?;

                let result = lua.create_table()?;
                result.set("stdout", lua.create_string(&output.stdout)?)?;
                result.set("stderr", lua.create_string(&output.stderr)?)?;
                // `code()` is `None` only where a signal can end a process, which is unix; the
                // `-1` fallback is this module's sentinel there, keeping the field a number so
                // `result.status ~= 0` stays the one portable way to ask whether it worked. On
                // Windows `code()` always returns `Some`, so `unwrap_or(-1)` never fires there,
                // and `-1` on that platform is a legitimate exit code a process chose rather than
                // a sentinel this module reports. See the module doc for the full contract.
                result.set("status", output.status.code().unwrap_or(-1))?;
                Ok(result)
            })
            .map_err(fail)?;
        table.set("run", run).map_err(fail)?;

        let g = grants;
        let which = lua
            .create_function(move |_, program: mlua::LuaString| {
                let program = program.to_str()?;
                if !g.is_unrestricted() && !g.proc().allows(&program) {
                    return Err(mlua::Error::from(denied(&g, "which", &program)));
                }
                Ok(which(&program))
            })
            .map_err(fail)?;
        table.set("which", which).map_err(fail)?;

        Ok(())
    }
}

/// The overlay entries to apply to a child process.
fn overlay_entries(overlay: &env::Overlay) -> Vec<(String, Option<String>)> {
    overlay.child_entries()
}

/// The first executable named `program` on `PATH`, if there is one.
///
/// Deliberately no fallback to the current directory: a `PATH` lookup that quietly also searched
/// `.` is how a script ends up running whatever happens to be beside its input.
///
/// The search is **directory-major**: every candidate name for `program` — on unix there is
/// exactly one, on Windows there may be `program` and `program.exe` — is tried within one `PATH`
/// entry before moving to the next. Candidate-major (all directories for one candidate, then all
/// directories for the next) would let a later directory's match beat an earlier directory's for
/// a different candidate spelling, which inverts what `PATH` order means to the person who set it.
/// The distinction is unobservable today, since [`native::executable_candidates`] returns exactly
/// one name on both flavours, but the ordering is fixed by intent here rather than by whichever
/// loop nesting happened to be typed first.
fn which(program: &str) -> Option<String> {
    let path = env::overlay().get("PATH")?;
    let candidates = native::executable_candidates(program);
    std::env::split_paths(&path)
        .find_map(|directory| {
            candidates
                .iter()
                .map(|name| directory.join(name))
                .find(|candidate| is_executable(candidate))
        })
        .map(|found| native::to_script_string(&found))
}

/// The refusal for a bare program name that `which` could not resolve on `PATH`, on Windows.
///
/// No new `Error` variant: unix already reports a missing program as `Error::Io` raised by the
/// spawn itself, and the two platforms reporting the same shape is worth more than a variant that
/// exists only to name which platform failed.
#[cfg(windows)]
fn unresolvable(program: &str) -> Error {
    unresolvable_naming(shim_probe(program), program)
}

/// The message-building half of [`unresolvable`], taking the shim probe's result directly rather
/// than calling [`shim_probe`] itself — split out so the refusal's exact wording can be exercised
/// against a value built by hand, with nothing process-wide involved.
#[cfg(windows)]
fn unresolvable_naming(shim: Option<String>, program: &str) -> Error {
    let detail = shim.map_or_else(
        || format!("`{program}.exe` was not found on PATH"),
        |shim| {
            format!(
                "`{program}.exe` was not found on PATH, but `{shim}` was — airsl runs only \
                 `.exe` programs, because spawning a batch file routes its arguments through \
                 `cmd.exe`, whose quoting rules differ from the ones this crate's process module \
                 is built to avoid entirely"
            )
        },
    );
    Error::Io {
        operation: "run",
        path: program.to_owned(),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, detail),
    }
}

/// Looks for a `.bat`/`.cmd` shim named `program` on `PATH`, for [`unresolvable`]'s message only.
///
/// These two extensions are consulted here **only to name what was found**, never to decide what
/// to run: resolution and spawning go exclusively through `native::executable_candidates`, which
/// never proposes them. Omitting this probe would leave the refusal only ever naming a program
/// that is missing outright — not the case anyone actually hits, since `npm`, `npx`, `yarn` and
/// `tsc` all ship as a `.cmd` shim on Windows and are otherwise refused with no clue why.
#[cfg(windows)]
fn shim_probe(program: &str) -> Option<String> {
    let path = env::overlay().get("PATH")?;
    shim_probe_on(&path, program)
}

/// The lookup half of [`shim_probe`], taking `PATH`'s value as a plain string rather than reading
/// it from the process-wide overlay — split out so the lookup itself can be exercised against a
/// value built by hand, with nothing process-wide involved.
#[cfg(windows)]
fn shim_probe_on(path: &str, program: &str) -> Option<String> {
    const SHIM_EXTENSIONS: [&str; 2] = ["cmd", "bat"];
    std::env::split_paths(path).find_map(|directory| {
        SHIM_EXTENSIONS.iter().find_map(|extension| {
            let candidate = directory.join(format!("{program}.{extension}"));
            candidate
                .is_file()
                .then(|| format!("{program}.{extension}"))
        })
    })
}

/// Whether `path` is an existing file this crate will run, on this platform.
///
/// Existence and "is a file" are asked here on both platforms; the execute question itself is
/// answered by [`has_execute_permission`], which differs by platform — unix asks the mode bits,
/// Windows answers unconditionally because existence is already the whole question there. See
/// [`has_execute_permission`] for why.
fn is_executable(path: &std::path::Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && has_execute_permission(&meta))
}

/// Whether `meta` describes a file the current user could execute, on unix.
///
/// The mode-bit check is load-bearing for *ordering*, not only for the verdict: `which` walks
/// `PATH` one directory at a time, and a readable-but-not-executable file early on `PATH` must be
/// **skipped** so a later directory's entry can win. Collapsing this to plain existence — as the
/// Windows arm below does — would let that early, unusable file end the search.
#[cfg(unix)]
fn has_execute_permission(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    meta.permissions().mode() & 0o111 != 0
}

/// Whether `meta` describes a file the current user could execute, on Windows.
///
/// Windows has no execute bit, so `true` here is not a stub standing in for a check that has not
/// been written yet: the question "may this be executed" was already answered by the lexical half
/// ([`crate::paths::rules::native::executable_candidates`]), which only ever proposes names ending
/// `.exe`. Once a candidate with that shape exists as a file, existence is the entire remaining
/// question — there is no permission bit left to consult.
#[cfg(windows)]
const fn has_execute_permission(_meta: &std::fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Proc;
    use crate::{Engine, GrantSet, HostModule as _, Policy, Script};

    fn granted(programs: &[&str]) -> Engine {
        let programs: Vec<String> = programs.iter().map(|s| (*s).to_owned()).collect();
        Engine::builder()
            .policy(
                Policy::confined()
                    .with_grants(GrantSet::declared().with_proc(|proc| proc.allow(programs))),
            )
            .build()
            .unwrap()
    }

    fn eval<T: mlua::FromLuaMulti>(engine: &Engine, source: &str) -> crate::Result<T> {
        engine.eval_to::<T>(&Script::from_source(source, "test").unwrap())
    }

    #[test]
    fn the_module_is_named_proc() {
        assert_eq!(Proc::new().name().as_str(), "proc");
    }

    #[test]
    fn is_executable_is_false_for_a_directory_regardless_of_platform() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!super::is_executable(dir.path()));
    }

    #[test]
    fn is_executable_is_false_for_a_path_that_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!super::is_executable(&dir.path().join("nowhere")));
    }

    #[cfg(unix)]
    #[test]
    fn is_executable_follows_the_mode_bits_on_unix() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().unwrap();

        let readable = dir.path().join("readable");
        std::fs::write(&readable, b"").unwrap();
        std::fs::set_permissions(&readable, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!super::is_executable(&readable));

        let executable = dir.path().join("executable");
        std::fs::write(&executable, b"").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(super::is_executable(&executable));
    }

    // Not runnable on this host: `#[cfg(windows)]` is unparsed on unix, so this asserts what the
    // Windows CI leg must observe rather than anything this run proves. Windows has no execute
    // bit, so a file's mere existence is what `is_executable` answers there — the lexical half
    // (does the name end `.exe`) has already decided whether it may run.
    #[cfg(windows)]
    #[test]
    fn is_executable_is_true_for_any_existing_file_on_windows() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("readable.txt");
        std::fs::write(&file, b"").unwrap();
        assert!(super::is_executable(&file));
    }

    #[test]
    fn run_captures_stdout_and_the_exit_status() {
        // `echo` is a `cmd` builtin on Windows, not an `.exe` on `PATH`, so the two platforms need
        // different fixtures for the same property. The Windows arm is not runnable on this host —
        // `#[cfg(windows)]` is unparsed here — so it asserts what the Windows CI leg must observe.
        #[cfg(unix)]
        {
            let engine = granted(&["echo"]);
            let out: String = eval(
                &engine,
                "local r = airsstack.proc.run({'echo', 'hello'})
                 return r.stdout .. ':' .. tostring(r.status)",
            )
            .unwrap();
            assert_eq!(out, "hello\n:0");
        }
        #[cfg(windows)]
        {
            let engine = granted(&["where"]);
            let out: String = eval(
                &engine,
                "local r = airsstack.proc.run({'where', 'where'})
                 return r.stdout .. ':' .. tostring(r.status)",
            )
            .unwrap();
            let out = out.to_ascii_lowercase();
            assert!(out.contains("where.exe") && out.ends_with(":0"), "{out}");
        }
    }

    #[test]
    fn run_captures_a_non_zero_status_without_raising() {
        // A command that fails is a result, not an error: the script asked what happened.
        #[cfg(unix)]
        {
            let engine = granted(&["false"]);
            let status: i64 = eval(&engine, "return airsstack.proc.run({'false'}).status").unwrap();
            assert_ne!(status, 0);
        }
        // There is no `false.exe` on Windows, so this arm asks a real `.exe` to fail instead
        // (`where` on a name it cannot find). Not runnable on this host; the Windows CI leg is
        // what proves it.
        #[cfg(windows)]
        {
            let engine = granted(&["where"]);
            let status: i64 = eval(
                &engine,
                "return airsstack.proc.run({'where', 'airsl-nonexistent-xyz'}).status",
            )
            .unwrap();
            assert_ne!(status, 0);
        }
    }

    #[test]
    fn run_captures_stderr_separately() {
        // The unix arm spawns `sh` to write to file descriptor 2 directly. Windows has no `sh` on
        // every installation, so its arm asks `where` to look for a name it cannot find, which
        // writes its `INFO:` line to stderr and nothing to stdout; the message text itself is
        // localised, so only non-emptiness is asserted there. Not runnable on this host; the
        // Windows CI leg is what proves it.
        #[cfg(unix)]
        {
            let engine = granted(&["sh"]);
            let err: String = eval(
                &engine,
                "return airsstack.proc.run({'sh', '-c', 'echo oops 1>&2'}).stderr",
            )
            .unwrap();
            assert_eq!(err, "oops\n");
        }
        #[cfg(windows)]
        {
            let engine = granted(&["where"]);
            let (stdout, stderr): (String, String) = eval(
                &engine,
                "local r = airsstack.proc.run({'where', 'airsl-nonexistent-xyz'})
                 return r.stdout, r.stderr",
            )
            .unwrap();
            assert!(stdout.is_empty(), "{stdout}");
            assert!(!stderr.is_empty());
        }
    }

    #[test]
    fn arguments_are_passed_without_a_shell() {
        // The whole reason `run` takes an argv array: this argument reaches the program intact
        // rather than being split, globbed or interpreted.
        #[cfg(unix)]
        {
            let engine = granted(&["echo"]);
            let out: String = eval(
                &engine,
                "return airsstack.proc.run({'echo', 'a b; rm -rf *'}).stdout",
            )
            .unwrap();
            assert_eq!(out, "a b; rm -rf *\n");
        }
        // `where` takes no positional argument to echo back, so the Windows arm uses
        // `find.exe` (present on every installation) to search a file for a payload line
        // containing the four shell metacharacters a real shell would act on: `&` would
        // terminate the command, `|` would open a pipe, `^` would escape the next character and
        // `%FOO%` would expand. Finding the literal line proves none of that happened.
        //
        // Not runnable on this host; the Windows CI leg is what proves it.
        #[cfg(windows)]
        {
            let engine = granted(&["find"]);
            let dir = tempfile::tempdir().unwrap();
            let payload = "a & b | c ^ d %FOO%";
            let file = dir.path().join("payload.txt");
            std::fs::write(&file, format!("{payload}\n")).unwrap();
            // `{file_arg:?}` below interpolates through Rust's `Debug for str`, which already
            // escapes every `\` as `\\` — the same spelling Lua reads back as one literal
            // backslash — so a `windows-latest` temp path is safe to interpolate this way whether
            // or not it has been converted first. The conversion here is for a different reason:
            // `airsstack` accepts `/`-spelled paths on input, so rendering through this crate's
            // own vocabulary before handing the path to `find.exe` keeps the argument in the same
            // spelling a script would have built it in, rather than the process's native one.
            let file_arg = super::native::to_script_string(&file);
            let source =
                format!("return airsstack.proc.run({{'find', {payload:?}, {file_arg:?}}}).stdout");
            let out: String = eval(&engine, &source).unwrap();
            assert!(out.contains(payload), "{out}");
        }
    }

    // Not runnable on this host: `#[cfg(windows)]` is unparsed on unix, so these two assert what
    // the Windows CI leg must observe. `run` pre-resolves a bare program name through `which`
    // before spawning, on Windows only, so the two branches — bare name and already-a-path — must
    // each be exercised on their own.
    #[cfg(windows)]
    #[test]
    fn a_bare_program_name_is_resolved_through_which_before_spawning() {
        let engine = granted(&["where"]);
        let status: i64 = eval(
            &engine,
            "return airsstack.proc.run({'where', 'where'}).status",
        )
        .unwrap();
        assert_eq!(status, 0);
    }

    #[cfg(windows)]
    #[test]
    fn a_program_spelled_as_a_path_is_passed_through_unresolved() {
        // The only way this branch is reachable: `ProcGrant::allows` refuses a path even when the
        // bare name it resolves to is granted (`a_proc_grant_does_not_admit_a_path_to_a_granted_name`
        // in `sandbox::grants`), so a script can only ever get here under a policy that grants
        // everything.
        let engine = Engine::builder().policy(Policy::trusted()).build().unwrap();
        let status: i64 = eval(
            &engine,
            "return airsstack.proc.run(\
                {'C:\\\\Windows\\\\System32\\\\where.exe', 'where'}\
             ).status",
        )
        .unwrap();
        assert_eq!(status, 0);
    }

    // Not runnable on this host, for the same reason as the pair above.
    #[cfg(windows)]
    #[test]
    fn an_unresolvable_program_names_the_program_and_path_in_the_refusal() {
        let engine = granted(&["airsl-not-installed"]);
        let err = eval::<mlua::Value>(
            &engine,
            "return airsstack.proc.run({'airsl-not-installed'})",
        )
        .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("airsl-not-installed"), "{message}");
        assert!(message.contains("PATH"), "{message}");
    }

    #[cfg(windows)]
    #[test]
    fn a_cmd_shim_is_named_in_the_refusal_but_never_run() {
        // Drives `shim_probe_on` and `unresolvable_naming` directly rather than through
        // `airsstack.proc.run`: both `shim_probe` and `unresolvable` read `PATH` from the
        // process-wide overlay every engine shares (`env::overlay()`), so writing a test `PATH`
        // into it — even one restored before the test returns — would be visible to any other
        // test running concurrently in the same process. Calling the pure halves with a `PATH`
        // built by hand, and never calling `run` at all, exercises the same probe and the same
        // refusal wording with nothing process-wide involved and nothing spawned.
        //
        // Proves the probe fires for the case that motivates it — a Node-style shim — rather than
        // only for a file literally named `npm` with no extension.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("npm.cmd"), b"@echo off\r\n").unwrap();

        let path = dir.path().display().to_string();
        let shim = super::shim_probe_on(&path, "npm");
        assert_eq!(shim, Some(String::from("npm.cmd")));

        let message = super::unresolvable_naming(shim, "npm").to_string();
        assert!(message.contains("npm.cmd"), "{message}");
        assert!(message.contains(".exe"), "{message}");

        // No shim on that `PATH` for a program that has none at all.
        assert_eq!(super::shim_probe_on(&path, "airsl-not-installed"), None);
    }

    #[test]
    fn an_ungranted_program_is_refused() {
        let engine = granted(&["echo"]);
        let err = eval::<mlua::Value>(&engine, "return airsstack.proc.run({'curl'})").unwrap_err();
        assert!(err.to_string().contains("proc.run denied"), "{err}");
    }

    #[test]
    fn a_path_to_a_granted_program_is_still_refused() {
        // The grant is on the name as written. Accepting `/bin/echo` because `echo` is granted
        // would make the allowlist mean something different from what it says.
        let engine = granted(&["echo"]);
        let err =
            eval::<mlua::Value>(&engine, "return airsstack.proc.run({'/bin/echo'})").unwrap_err();
        assert!(err.to_string().contains("denied"), "{err}");
    }

    #[test]
    fn an_empty_argv_is_refused_with_a_reason() {
        let engine = granted(&["echo"]);
        let err = eval::<mlua::Value>(&engine, "return airsstack.proc.run({})").unwrap_err();
        assert!(err.to_string().contains("empty"), "{err}");
    }

    #[test]
    fn a_grant_spelled_without_exe_permits_running_the_program_on_both_platforms() {
        // The property this whole module exists to keep: one script plus one grant, spelled
        // without a platform-specific suffix, runs unchanged on both platforms. The `.exe` lives
        // in the candidate list `which` and `run` search with, never in what the grant compares.
        #[cfg(unix)]
        {
            let engine = granted(&["echo"]);
            let status: i64 =
                eval(&engine, "return airsstack.proc.run({'echo', 'ok'}).status").unwrap();
            assert_eq!(status, 0);
        }
        // Not runnable on this host: `#[cfg(windows)]` is unparsed on unix, so this asserts what
        // the Windows CI leg must observe rather than anything this run proves.
        #[cfg(windows)]
        {
            let engine = granted(&["where"]);
            let status: i64 = eval(
                &engine,
                "return airsstack.proc.run({'where', 'where'}).status",
            )
            .unwrap();
            assert_eq!(status, 0);
        }
    }

    #[test]
    fn which_finds_a_granted_program_on_the_path() {
        #[cfg(unix)]
        {
            let engine = granted(&["sh"]);
            let found: String = eval(&engine, "return airsstack.proc.which('sh')").unwrap();
            assert!(found.ends_with("/sh"), "{found}");
        }
        // A grant of `where`, spelled without an extension, must still find `where.exe` — the
        // candidate list is `native::executable_candidates`'s job, not the script's. Not runnable
        // on this host: `#[cfg(windows)]` is unparsed on unix, so this is what the Windows CI leg
        // must observe, not something this run proves.
        #[cfg(windows)]
        {
            let engine = granted(&["where"]);
            let found: String = eval(&engine, "return airsstack.proc.which('where')").unwrap();
            assert!(found.to_ascii_lowercase().ends_with("where.exe"), "{found}");
        }
    }

    #[test]
    fn which_renders_the_found_path_without_backslashes() {
        #[cfg(unix)]
        {
            // `to_script_string` is the identity here and a `PATH` entry never contains `\`
            // regardless of this conversion, so this assertion is green before and after any
            // change on this host — it does not prove the conversion did anything here.
            let engine = granted(&["sh"]);
            let found: String = eval(&engine, "return airsstack.proc.which('sh')").unwrap();
            assert!(!found.contains('\\'), "{found}");
        }
        // Exercises the real conversion: a Windows `PATH` entry is naturally `\`-separated, so a
        // green result here — unlike the unix arm above — is evidence the rendering ran. Not
        // runnable on this host; the Windows CI leg is what proves it.
        #[cfg(windows)]
        {
            let engine = granted(&["where"]);
            let found: String = eval(&engine, "return airsstack.proc.which('where')").unwrap();
            assert!(!found.contains('\\'), "{found}");
        }
    }

    #[test]
    fn which_returns_nil_for_something_that_is_not_installed() {
        let engine = granted(&["airsl-definitely-not-installed"]);
        let kind: String = eval(
            &engine,
            "return type(airsstack.proc.which('airsl-definitely-not-installed'))",
        )
        .unwrap();
        assert_eq!(kind, "nil");
    }

    #[test]
    fn which_is_refused_for_an_ungranted_program() {
        // Otherwise `which` becomes a way to enumerate the host's software without any grant.
        let engine = granted(&["echo"]);
        let err = eval::<mlua::Value>(&engine, "return airsstack.proc.which('curl')").unwrap_err();
        assert!(err.to_string().contains("proc.which denied"), "{err}");
    }

    #[test]
    fn a_policy_granting_nothing_refuses_every_program() {
        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();
        let err = eval::<mlua::Value>(&engine, "return airsstack.proc.run({'echo'})").unwrap_err();
        assert!(err.to_string().contains("no executables"), "{err}");
    }

    #[test]
    fn a_trusted_policy_runs_anything() {
        // `echo` is a `cmd` builtin, not an `.exe`, so the Windows arm asks for `where` instead.
        // Not runnable on this host; the Windows CI leg is what proves it.
        #[cfg(unix)]
        {
            let engine = Engine::builder().policy(Policy::trusted()).build().unwrap();
            let out: String =
                eval(&engine, "return airsstack.proc.run({'echo', 'ok'}).stdout").unwrap();
            assert_eq!(out, "ok\n");
        }
        #[cfg(windows)]
        {
            let engine = Engine::builder().policy(Policy::trusted()).build().unwrap();
            let status: i64 = eval(
                &engine,
                "return airsstack.proc.run({'where', 'where'}).status",
            )
            .unwrap();
            assert_eq!(status, 0);
        }
    }
}
