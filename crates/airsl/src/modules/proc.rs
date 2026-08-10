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

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::modules::env;
use crate::modules::{HostModule, InstallContext};
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
                // A signalled process has no exit code. Reporting -1 keeps the field a number so
                // that `result.status ~= 0` stays the one way to ask whether it worked.
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
fn which(program: &str) -> Option<String> {
    let path = env::overlay().get("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find(|candidate| is_executable(candidate))
        .map(|found| found.to_string_lossy().into_owned())
}

/// Whether `path` is a file the current user could execute.
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
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
    fn run_captures_stdout_and_the_exit_status() {
        let engine = granted(&["echo"]);
        let out: String = eval(
            &engine,
            "local r = airsstack.proc.run({'echo', 'hello'})
             return r.stdout .. ':' .. tostring(r.status)",
        )
        .unwrap();
        assert_eq!(out, "hello\n:0");
    }

    #[test]
    fn run_captures_a_non_zero_status_without_raising() {
        // A command that fails is a result, not an error: the script asked what happened.
        let engine = granted(&["false"]);
        let status: i64 = eval(&engine, "return airsstack.proc.run({'false'}).status").unwrap();
        assert_ne!(status, 0);
    }

    #[test]
    fn run_captures_stderr_separately() {
        let engine = granted(&["sh"]);
        let err: String = eval(
            &engine,
            "return airsstack.proc.run({'sh', '-c', 'echo oops 1>&2'}).stderr",
        )
        .unwrap();
        assert_eq!(err, "oops\n");
    }

    #[test]
    fn arguments_are_passed_without_a_shell() {
        // The whole reason `run` takes an argv array: this argument reaches the program intact
        // rather than being split, globbed or interpreted.
        let engine = granted(&["echo"]);
        let out: String = eval(
            &engine,
            "return airsstack.proc.run({'echo', 'a b; rm -rf *'}).stdout",
        )
        .unwrap();
        assert_eq!(out, "a b; rm -rf *\n");
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
    fn which_finds_a_granted_program_on_the_path() {
        let engine = granted(&["sh"]);
        let found: String = eval(&engine, "return airsstack.proc.which('sh')").unwrap();
        assert!(found.ends_with("/sh"), "{found}");
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
        let engine = Engine::builder().policy(Policy::trusted()).build().unwrap();
        let out: String =
            eval(&engine, "return airsstack.proc.run({'echo', 'ok'}).stdout").unwrap();
        assert_eq!(out, "ok\n");
    }
}
