//! `airsl` — runs Lua scripts on the embedded `airsl` runtime.
//!
//! The binary the airsstack plugin hooks invoke. It stays deliberately thin: parse arguments,
//! pick a failure policy and a sandbox, hand off. Every decision it makes is visible in
//! [`cli::Command`], so what a hook gets is readable from the command line it was given.
//!
//! Responsibilities: wiring the subcommands to their implementations and setting the exit code.
//!
//! Non-responsibilities: the runtime itself, which lives in the `airsl` library.

#![forbid(unsafe_code)]

mod cli;
mod doctor;
mod run;

use airsl::{FailurePolicy, Sandbox};
use clap::Parser as _;

fn main() -> std::process::ExitCode {
    let code = match cli::Cli::parse().command {
        cli::Command::Run {
            fail_open,
            unrestricted,
            script,
            args,
        } => {
            let policy = if fail_open {
                FailurePolicy::FailOpen
            } else {
                FailurePolicy::Report
            };
            let sandbox = if unrestricted {
                Sandbox::Full
            } else {
                Sandbox::Restricted
            };
            run::run(&script, &args, policy, sandbox)
        }
        cli::Command::Doctor => {
            print!("{}", doctor::report());
            0
        }
    };
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(1))
}
