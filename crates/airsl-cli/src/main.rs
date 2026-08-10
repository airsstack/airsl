//! `airsl` — runs Lua scripts on the embedded `airsl` runtime.
//!
//! The binary the airsstack plugin hooks invoke. It stays deliberately thin: parse arguments,
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
mod run;
mod test_runner;

use airsl::{FailurePolicy, Policy};
use clap::Parser as _;

fn main() -> std::process::ExitCode {
    let code = match cli::Cli::parse().command {
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
    };
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(1))
}
