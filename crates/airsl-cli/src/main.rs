//! `airsl` — runs Lua scripts on the embedded `airsl` runtime.
//!
//! The binary a hook, a build step or a shell script invokes. It stays deliberately thin: parse arguments,
//! resolve a policy and a failure policy, hand off. Every decision it makes is visible in
//! [`cli::Command`], so what a hook gets is readable from the command line it was given.
//!
//! Responsibilities: wiring the subcommands to their implementations and setting the exit code.
//!
//! Non-responsibilities: the runtime itself, which lives in the `airsl` library.

#![forbid(unsafe_code)]

mod check;
mod cli;
mod doctor;
mod ext;
mod ext_doctor;
mod ext_fire;
mod lua;
mod run;
mod test_runner;
#[cfg(test)]
mod test_support;

use std::io;

use airsl::{FailurePolicy, Policy};
use clap::Parser as _;

fn main() -> std::process::ExitCode {
    // The handles go in unlocked. Stdin's lock is not reentrant, and a script reads stdin through
    // `airsstack.stdio` and `airsstack.hook` on this same thread, so holding the lock across a
    // command would hang every script that reads its input.
    let code = dispatch(
        cli::Cli::parse().command,
        io::stdin(),
        &mut io::stdout(),
        &mut io::stderr(),
    );
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(1))
}

/// Runs one parsed command and returns the process exit code.
///
/// Split from `main` so a test can drive the `ext` and `lua` arms through injected streams, the
/// way their own modules take them. The other arms write to the process's own streams and ignore
/// these.
fn dispatch(
    command: cli::Command,
    stdin: impl io::Read,
    stdout: &mut impl io::Write,
    stderr: &mut impl io::Write,
) -> i32 {
    match command {
        cli::Command::Run {
            fail_open,
            policy,
            memory_limit,
            instruction_limit,
            grants,
            script,
            args,
        } => {
            let failure = if fail_open {
                FailurePolicy::FailOpen
            } else {
                FailurePolicy::Report
            };
            let policy = cli::resolve_policy(policy, memory_limit, instruction_limit, grants);
            run::run(&script, &args, failure, &policy)
        }
        cli::Command::Test {
            policy,
            grants,
            path,
        } => {
            let policy = cli::resolve_policy(policy, None, None, grants);
            test_runner::run(&path, &policy)
        }
        cli::Command::Check { path } => check::run(&path),
        cli::Command::Doctor { policy } => {
            print!("{}", doctor::report(&Policy::from(policy)));
            0
        }
        cli::Command::Ext(command) => ext::run(command, stdin, stdout, stderr),
        cli::Command::Lua(command) => lua::run(command, stdout, stderr),
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{cli, dispatch};
    use clap::Parser as _;

    #[test]
    fn the_lua_setup_command_reports_on_stdout_and_leaves_stderr_empty() {
        let project = tempfile::tempdir().unwrap();
        let dir = project.path().to_str().unwrap();
        let command = cli::Cli::try_parse_from(["airsl", "lua", "setup", dir])
            .unwrap()
            .command;
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());

        let code = dispatch(command, std::io::empty(), &mut stdout, &mut stderr);

        assert_eq!(code, 0);
        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            "created .airsl/types/airsstack.lua\ncreated .luarc.json\n"
        );
        assert!(stderr.is_empty());
    }
}
