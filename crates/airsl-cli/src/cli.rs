//! The command-line grammar.
//!
//! Separate from `main.rs` so the parsed shape is a value the rest of the binary can be tested
//! against without spawning a process. `clap`'s derive lives here and nowhere else.
//!
//! Responsibilities: [`Cli`] and [`Command`], the complete argument surface.
//!
//! Non-responsibilities: doing anything with the arguments. [`crate::run`] and [`crate::doctor`]
//! act on them.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Runs Lua scripts on the embedded `airsl` runtime.
#[derive(Debug, Parser)]
#[command(name = "airsl", version, about, long_about = None)]
pub(crate) struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// The available subcommands.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Run a Lua script.
    Run {
        /// Discard errors and always exit 0.
        ///
        /// For scripts run as editor or agent hooks, where a non-zero exit is read as a signal
        /// rather than a diagnostic and can block unrelated work. The flag lives here rather than
        /// inside the script because a syntax error happens before any in-script setting could
        /// take effect, and that is the case the behaviour exists for.
        #[arg(long)]
        fail_open: bool,

        /// Give the script the full Lua standard library, including `io`, `os` and `debug`.
        ///
        /// For trusted first-party scripts only. None of the containment the host modules provide
        /// applies to a script that can open files directly.
        #[arg(long)]
        unrestricted: bool,

        /// Path to the `.lua` file.
        script: PathBuf,

        /// Arguments passed through to the script.
        ///
        /// `allow_hyphen_values` is required alongside `trailing_var_arg`: without it a script
        /// argument that begins with `-` is parsed as an unknown flag of this binary rather than
        /// handed to the script, which is exactly what the ported shell scripts pass.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Report the runtime version and the installed host modules.
    Doctor,
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]
    #![expect(
        clippy::panic,
        reason = "tests panic to reject an unexpected parse shape; a panic is the intended failure signal"
    )]

    use super::{Cli, Command};
    use clap::Parser as _;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap()
    }

    #[test]
    fn run_defaults_to_reporting_errors_and_a_restricted_sandbox() {
        let Command::Run {
            fail_open,
            unrestricted,
            script,
            args,
        } = parse(&["airsl", "run", "hook.lua"]).command
        else {
            panic!("expected the run subcommand");
        };
        assert!(!fail_open);
        assert!(!unrestricted);
        assert_eq!(script, std::path::Path::new("hook.lua"));
        assert!(args.is_empty());
    }

    #[test]
    fn fail_open_is_opt_in() {
        let Command::Run { fail_open, .. } =
            parse(&["airsl", "run", "--fail-open", "h.lua"]).command
        else {
            panic!("expected the run subcommand");
        };
        assert!(fail_open);
    }

    #[test]
    fn trailing_arguments_reach_the_script_untouched() {
        let Command::Run { args, .. } =
            parse(&["airsl", "run", "h.lua", "--verbose", "-x", "value"]).command
        else {
            panic!("expected the run subcommand");
        };
        assert_eq!(args, ["--verbose", "-x", "value"]);
    }

    #[test]
    fn doctor_takes_no_arguments() {
        assert!(matches!(
            parse(&["airsl", "doctor"]).command,
            Command::Doctor
        ));
    }

    #[test]
    fn a_missing_script_path_is_a_usage_error() {
        assert!(Cli::try_parse_from(["airsl", "run"]).is_err());
    }

    #[test]
    fn an_unknown_subcommand_is_a_usage_error() {
        assert!(Cli::try_parse_from(["airsl", "frobnicate"]).is_err());
    }
}
