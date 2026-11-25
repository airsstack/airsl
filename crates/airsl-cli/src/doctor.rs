//! The `airsl doctor` report.
//!
//! Exists because hooks that call this binary fail open when it is missing — a hook
//! whose runtime is absent does nothing and says nothing, which is correct behaviour and terrible
//! diagnostics. `doctor` is the one command that answers "is it installed, and what would a script
//! actually get" without needing a script to run.
//!
//! Responsibilities: [`run`], which resolves the policy the command line asks for exactly as
//! `airsl run` would, and [`report`], which renders a resolved policy and the modules installed
//! under it.
//!
//! Non-responsibilities: fixing anything, and deciding what a preset or flag means — that is
//! [`crate::cli::resolve_policy`]'s. The report is read-only and describes whatever policy it is
//! handed.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use core::fmt::Write as _;

use airsl::{Engine, Policy};

use crate::cli::{Grants, PolicyName, resolve_policy};

/// The report for `policy` with `grants` added, built by the same resolution `airsl run` uses,
/// so `doctor` never describes a policy a script would not get.
#[must_use]
pub(crate) fn run(policy: PolicyName, grants: Grants) -> String {
    report(&resolve_policy(policy, None, None, grants))
}

/// Renders the runtime summary for `policy`.
///
/// Returns the text to print rather than printing it, so the shape is testable. A runtime that
/// cannot be built is reported as a status line rather than an error, because the whole point of
/// the command is to describe a broken installation.
#[must_use]
pub(crate) fn report(policy: &Policy) -> String {
    let mut out = format!("airsl {}\n", env!("CARGO_PKG_VERSION"));

    match Engine::builder().policy(policy.clone()).build() {
        Ok(engine) => {
            let modules: Vec<_> = engine
                .module_names()
                .iter()
                .map(ToString::to_string)
                .collect();
            let limits = policy.limits();
            let _ = writeln!(out, "  lua:          {}", engine.lua_version());
            let _ = writeln!(out, "  language:     {}", policy.language());
            let _ = writeln!(out, "  root table:   {}", engine.root_table());
            let _ = writeln!(out, "  grants:       {}", policy.grants());
            let _ = writeln!(out, "  memory:       {}", describe(limits.memory()));
            let _ = writeln!(out, "  instructions: {}", describe(limits.instructions()));
            let _ = writeln!(out, "  modules:      {}", modules.join(", "));
        }
        Err(error) => {
            let _ = writeln!(out, "  status:       unusable ({error})");
        }
    }
    out
}

/// Renders a ceiling, or the word that says there is none.
///
/// `pub(crate)` so [`crate::ext_doctor`] can render a negotiated policy's limits the same way,
/// rather than duplicating the "no ceiling" wording.
pub(crate) fn describe<T: core::fmt::Display>(limit: Option<T>) -> String {
    limit.map_or_else(|| String::from("unlimited"), |value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::{describe, report, run};
    use crate::cli::{Cli, Command, Grants, PolicyName, resolve_policy};
    use airsl::{HostAccess, MemoryLimit, Policy};
    use clap::Parser as _;

    #[test]
    fn doctor_reports_pure_with_stderr_as_access_stderr() {
        let grants = Grants {
            access: vec![HostAccess::Stderr],
            ..Grants::default()
        };
        let policy = resolve_policy(PolicyName::Pure, None, None, grants);
        assert!(report(&policy).contains("grants:       access stderr\n"));
    }

    #[test]
    fn doctor_reports_the_policy_its_own_command_line_asks_for() {
        let Ok(Cli {
            command: Command::Doctor { policy, grants },
            ..
        }) = Cli::try_parse_from([
            "airsl",
            "doctor",
            "--policy",
            "pure",
            "--allow-access",
            "stderr",
        ])
        else {
            unreachable!("`doctor` parses into `Command::Doctor`");
        };
        assert!(run(policy, grants).contains("grants:       access stderr\n"));
    }

    #[test]
    fn doctor_reports_pure_with_no_flags_as_none() {
        let policy = resolve_policy(PolicyName::Pure, None, None, Grants::default());
        assert!(report(&policy).contains("grants:       none\n"));
    }

    #[test]
    fn the_report_names_the_binary_and_its_version() {
        let text = report(&Policy::confined());
        assert!(text.starts_with("airsl "), "{text}");
        assert!(text.contains(env!("CARGO_PKG_VERSION")), "{text}");
    }

    #[test]
    fn the_report_names_the_lua_version() {
        assert!(report(&Policy::confined()).contains("Lua 5.4"));
    }

    #[test]
    fn the_report_lists_the_installed_modules() {
        let text = report(&Policy::confined());
        assert!(text.contains("modules:"), "{text}");
        assert!(text.contains("json"), "{text}");
    }

    #[test]
    fn the_report_states_the_language_surface_it_resolved() {
        assert!(report(&Policy::confined()).contains("language:     restricted"));
        assert!(report(&Policy::pure()).contains("language:     minimal"));
        assert!(report(&Policy::trusted()).contains("language:     full"));
    }

    #[test]
    fn the_report_names_the_root_table_modules_are_installed_under() {
        assert!(report(&Policy::confined()).contains("root table:   airsstack"));
    }

    #[test]
    fn the_report_states_the_grants_the_policy_extends() {
        assert!(
            report(&Policy::confined())
                .contains("grants:       access stdin,stdout,stderr,clock,cwd,require,zones")
        );
        assert!(report(&Policy::trusted()).contains("grants:       unrestricted"));
    }

    #[test]
    fn the_report_states_both_ceilings() {
        let confined = report(&Policy::confined());
        assert!(
            confined.contains("memory:       67108864 bytes"),
            "{confined}"
        );
        assert!(
            confined.contains("instructions: 100000000 instructions"),
            "{confined}"
        );
    }

    #[test]
    fn a_lifted_ceiling_reports_as_unlimited() {
        let text = report(&Policy::trusted());
        assert!(text.contains("memory:       unlimited"), "{text}");
        assert!(text.contains("instructions: unlimited"), "{text}");
    }

    #[test]
    fn describe_renders_a_ceiling_or_says_there_is_none() {
        assert_eq!(describe(Some(MemoryLimit::bytes(8))), "8 bytes");
        assert_eq!(describe(None::<MemoryLimit>), "unlimited");
    }

    #[test]
    fn the_report_ends_with_a_newline() {
        assert!(report(&Policy::confined()).ends_with('\n'));
    }
}
