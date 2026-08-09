//! Running a script file and turning the outcome into a process exit code.
//!
//! Its own module because this is where the fail-open contract is actually honoured, and that
//! deserves to be readable in one screen. Everything above it decides policy; this is the single
//! place a Lua failure becomes — or does not become — a non-zero exit.
//!
//! Responsibilities: [`run`], which loads a script, evaluates it, and maps the result to an exit
//! code.
//!
//! Non-responsibilities: parsing arguments ([`crate::cli`]) and reporting runtime health
//! ([`crate::doctor`]).
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::Path;

use airsl::{Engine, FailurePolicy, Sandbox, Script};

/// Environment variable that turns on diagnostics for scripts running fail-open.
///
/// Without it a fail-open failure is silent by design, which makes a misbehaving hook hard to
/// diagnose; with it the error still does not affect the exit code.
const DEBUG_VAR: &str = "AIRSL_DEBUG";

/// Loads and runs `script`, returning the process exit code.
///
/// Under [`FailurePolicy::FailOpen`] every failure is swallowed and the code is 0. Under
/// [`FailurePolicy::Report`] a failure is written to stderr and the code is 1.
pub(crate) fn run(script: &Path, args: &[String], policy: FailurePolicy, sandbox: Sandbox) -> i32 {
    match execute(script, args, sandbox) {
        Ok(()) => 0,
        Err(error) => {
            if policy.swallows_errors() {
                if std::env::var_os(DEBUG_VAR).is_some() {
                    eprintln!("airsl: {error}");
                }
            } else {
                eprintln!("airsl: {error}");
            }
            policy.exit_code()
        }
    }
}

/// Builds the engine, loads the script, and evaluates it.
fn execute(script: &Path, args: &[String], sandbox: Sandbox) -> airsl::Result<()> {
    let engine = Engine::builder().sandbox(sandbox).build()?;
    set_script_args(&engine, args)?;
    let script = Script::from_file(script)?;
    engine.eval(&script)
}

/// Exposes the pass-through arguments to the script as the global `arg` table.
///
/// Lua's own convention for a standalone script, so a ported shell script reads `arg[1]` where it
/// previously read `$1`.
fn set_script_args(engine: &Engine, args: &[String]) -> airsl::Result<()> {
    let lua = engine.lua();
    let table = lua
        .create_table()
        .map_err(|e| airsl::Error::lua("<args>", e))?;
    for (index, value) in args.iter().enumerate() {
        table
            .set(index + 1, value.as_str())
            .map_err(|e| airsl::Error::lua("<args>", e))?;
    }
    lua.globals()
        .set("arg", table)
        .map_err(|e| airsl::Error::lua("<args>", e))
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::run;
    use airsl::{FailurePolicy, Sandbox};
    use std::io::Write as _;

    fn script(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.lua");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(body.as_bytes()).unwrap();
        (dir, path)
    }

    #[test]
    fn a_successful_script_exits_zero() {
        let (_dir, path) = script("local x = 1");
        assert_eq!(
            run(&path, &[], FailurePolicy::Report, Sandbox::Restricted),
            0
        );
    }

    #[test]
    fn a_failing_script_exits_nonzero_when_reporting() {
        let (_dir, path) = script("error('boom')");
        assert_ne!(
            run(&path, &[], FailurePolicy::Report, Sandbox::Restricted),
            0
        );
    }

    #[test]
    fn a_failing_script_exits_zero_when_failing_open() {
        let (_dir, path) = script("error('boom')");
        assert_eq!(
            run(&path, &[], FailurePolicy::FailOpen, Sandbox::Restricted),
            0
        );
    }

    #[test]
    fn a_syntax_error_also_fails_open() {
        let (_dir, path) = script("this is not lua");
        assert_eq!(
            run(&path, &[], FailurePolicy::FailOpen, Sandbox::Restricted),
            0
        );
    }

    #[test]
    fn a_missing_script_fails_open_rather_than_blocking() {
        let missing = std::path::Path::new("/nonexistent/s.lua");
        assert_eq!(
            run(missing, &[], FailurePolicy::FailOpen, Sandbox::Restricted),
            0
        );
        assert_ne!(
            run(missing, &[], FailurePolicy::Report, Sandbox::Restricted),
            0
        );
    }

    #[test]
    fn arguments_arrive_in_the_global_arg_table() {
        let (_dir, path) = script("assert(arg[1] == 'one'); assert(arg[2] == 'two')");
        let args = [String::from("one"), String::from("two")];
        assert_eq!(
            run(&path, &args, FailurePolicy::Report, Sandbox::Restricted),
            0
        );
    }

    #[test]
    fn a_restricted_run_cannot_reach_io() {
        let (_dir, path) = script("assert(io == nil)");
        assert_eq!(
            run(&path, &[], FailurePolicy::Report, Sandbox::Restricted),
            0
        );
    }

    #[test]
    fn an_unrestricted_run_can_reach_io() {
        let (_dir, path) = script("assert(type(io) == 'table')");
        assert_eq!(run(&path, &[], FailurePolicy::Report, Sandbox::Full), 0);
    }
}
