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

use airsl::{Engine, FailurePolicy, Policy, Script};

/// Environment variable that turns on diagnostics for scripts running fail-open.
///
/// Without it a fail-open failure is silent by design, which makes a misbehaving hook hard to
/// diagnose; with it the error still does not affect the exit code.
const DEBUG_VAR: &str = "AIRSL_DEBUG";

/// Loads and runs `script`, returning the process exit code.
///
/// Under [`FailurePolicy::FailOpen`] every failure is swallowed and the code is 0. Under
/// [`FailurePolicy::Report`] a failure is written to stderr and the code is 1.
///
/// `policy` is what the script may reach and spend; `failure` is what this process does about a
/// script that did not finish.
///
/// A script stopped for exhausting a resource ceiling is reported either way. The exit code still
/// stays at zero under fail-open — that contract is what stops a broken hook blocking the tool call
/// that triggered it — but silence is the wrong answer here: a hook consuming the host's memory or
/// looping until it is killed is a fact about the machine rather than a diagnostic the script
/// chose to emit, and it is otherwise undiscoverable.
pub(crate) fn run(script: &Path, args: &[String], failure: FailurePolicy, policy: &Policy) -> i32 {
    match execute(script, args, policy) {
        Ok(()) => 0,
        Err(error) => {
            let breached = error.exhausted_limit().is_some();
            if !failure.swallows_errors() || breached || std::env::var_os(DEBUG_VAR).is_some() {
                eprintln!("airsl: {error}");
            }
            failure.exit_code()
        }
    }
}

/// Builds the engine, loads the script, and evaluates it.
fn execute(script: &Path, args: &[String], policy: &Policy) -> airsl::Result<()> {
    let engine = Engine::builder().policy(policy.clone()).build()?;
    let script = Script::from_file(script)?.with_args(args.iter().map(String::as_str));
    engine.eval(&script)
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::run;
    use airsl::{FailurePolicy, InstructionLimit, Policy, ResourceLimits};
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
            run(&path, &[], FailurePolicy::Report, &Policy::confined()),
            0
        );
    }

    #[test]
    fn a_failing_script_exits_nonzero_when_reporting() {
        let (_dir, path) = script("error('boom')");
        assert_ne!(
            run(&path, &[], FailurePolicy::Report, &Policy::confined()),
            0
        );
    }

    #[test]
    fn a_failing_script_exits_zero_when_failing_open() {
        let (_dir, path) = script("error('boom')");
        assert_eq!(
            run(&path, &[], FailurePolicy::FailOpen, &Policy::confined()),
            0
        );
    }

    #[test]
    fn a_syntax_error_also_fails_open() {
        let (_dir, path) = script("this is not lua");
        assert_eq!(
            run(&path, &[], FailurePolicy::FailOpen, &Policy::confined()),
            0
        );
    }

    #[test]
    fn a_missing_script_fails_open_rather_than_blocking() {
        let missing = std::path::Path::new("/nonexistent/s.lua");
        assert_eq!(
            run(missing, &[], FailurePolicy::FailOpen, &Policy::confined()),
            0
        );
        assert_ne!(
            run(missing, &[], FailurePolicy::Report, &Policy::confined()),
            0
        );
    }

    #[test]
    fn arguments_arrive_in_the_global_arg_table() {
        let (_dir, path) = script("assert(arg[1] == 'one'); assert(arg[2] == 'two')");
        let args = [String::from("one"), String::from("two")];
        assert_eq!(
            run(&path, &args, FailurePolicy::Report, &Policy::confined()),
            0
        );
    }

    #[test]
    fn a_confined_run_cannot_reach_io() {
        let (_dir, path) = script("assert(io == nil)");
        assert_eq!(
            run(&path, &[], FailurePolicy::Report, &Policy::confined()),
            0
        );
    }

    #[test]
    fn a_runaway_script_fails_open_without_blocking_the_caller() {
        let (_dir, path) = script("while true do end");
        let policy = Policy::confined().with_limits(
            ResourceLimits::none().with_instructions(Some(InstructionLimit::count(100_000))),
        );
        assert_eq!(run(&path, &[], FailurePolicy::FailOpen, &policy), 0);
        assert_ne!(run(&path, &[], FailurePolicy::Report, &policy), 0);
    }

    #[test]
    fn a_trusted_run_can_reach_io() {
        let (_dir, path) = script("assert(type(io) == 'table')");
        assert_eq!(
            run(&path, &[], FailurePolicy::Report, &Policy::trusted()),
            0
        );
    }
}
