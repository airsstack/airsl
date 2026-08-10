//! The command-line grammar.
//!
//! Separate from `main.rs` so the parsed shape is a value the rest of the binary can be tested
//! against without spawning a process. `clap`'s derive lives here and nowhere else.
//!
//! Responsibilities: [`Cli`], [`Command`] and [`PolicyName`], the complete argument surface; the
//! parsers that turn a ceiling argument into a limit; and [`resolve_policy`], which applies the
//! caller's overrides to the preset they named.
//!
//! Non-responsibilities: acting on any of it. [`crate::run`] and [`crate::doctor`] do that.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::PathBuf;

use airsl::{GrantSet, InstructionLimit, MemoryLimit, Policy};
use clap::{Parser, Subcommand, ValueEnum};

/// The argument that switches a ceiling off entirely.
const UNLIMITED: &str = "none";

/// Runs Lua scripts on the embedded `airsl` runtime.
#[derive(Debug, Parser)]
#[command(name = "airsl", version, about, long_about = None)]
pub(crate) struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// A ceiling the caller named on the command line.
///
/// Distinct from the ceiling itself because "lift this preset's ceiling" is an instruction, not a
/// value: it has to survive as far as the policy so it can override what the preset supplied.
/// Saying nothing at all is the absence of a `Ceiling`, which leaves the preset alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ceiling<T> {
    /// The caller asked for no ceiling on this resource.
    Unlimited,
    /// The caller asked for this ceiling.
    Of(T),
}

impl<T> Ceiling<T> {
    /// The ceiling as the policy wants it, with [`Ceiling::Unlimited`] becoming no ceiling.
    fn into_limit(self) -> Option<T> {
        match self {
            Self::Unlimited => None,
            Self::Of(limit) => Some(limit),
        }
    }
}

/// Which policy preset a script runs under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub(crate) enum PolicyName {
    /// Every safe Lua library, unrestricted grants, no ceilings. First-party scripts only.
    Trusted,
    /// The default: restricted libraries, declared grants only, and both ceilings.
    #[default]
    Confined,
    /// Minimal libraries, no `os` or `coroutine`, and tight ceilings.
    Pure,
}

impl From<PolicyName> for Policy {
    fn from(value: PolicyName) -> Self {
        match value {
            PolicyName::Trusted => Self::trusted(),
            PolicyName::Confined => Self::confined(),
            PolicyName::Pure => Self::pure(),
        }
    }
}

/// The authority a caller grants on the command line.
///
/// Nothing is granted by default, on any preset below `trusted`. That is deliberate: a hook
/// launcher states what its script may reach, in the same place it states the script, so the
/// authority is visible to whoever reads the command rather than buried in a policy file.
#[derive(Debug, Default, clap::Args)]
pub(crate) struct Grants {
    /// Directory the script may read under. Repeat for several.
    #[arg(long = "allow-read", value_name = "DIR")]
    pub read: Vec<PathBuf>,

    /// Directory the script may write under. Repeat for several.
    ///
    /// Not implicitly readable: grant both if the script reads back what it wrote.
    #[arg(long = "allow-write", value_name = "DIR")]
    pub write: Vec<PathBuf>,

    /// Environment variable the script may read. Repeat for several.
    #[arg(long = "allow-env", value_name = "NAME")]
    pub env: Vec<String>,

    /// Executable the script may run. Repeat for several.
    #[arg(long = "allow-exec", value_name = "PROGRAM")]
    pub exec: Vec<String>,
}

impl Grants {
    /// The grant set these flags describe.
    fn into_grant_set(self) -> GrantSet {
        GrantSet::declared()
            .with_fs(|fs| {
                let fs = self.read.into_iter().fold(fs, airsl::FsGrant::read);
                self.write.into_iter().fold(fs, airsl::FsGrant::write)
            })
            .with_env(|env| env.read(self.env))
            .with_proc(|proc| proc.allow(self.exec))
    }
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
        ///
        /// A script stopped for exceeding a ceiling is still reported on stderr, because that is a
        /// fact about the host's resources rather than a diagnostic the script chose to emit.
        #[arg(long)]
        fail_open: bool,

        /// Which policy preset to run under.
        #[arg(long, value_enum, default_value_t = PolicyName::Confined)]
        policy: PolicyName,

        /// Memory ceiling in bytes, or `none` to lift the preset's ceiling.
        #[arg(long, value_name = "BYTES|none", value_parser = parse_memory_limit)]
        memory_limit: Option<Ceiling<MemoryLimit>>,

        /// Instruction ceiling, or `none` to lift the preset's ceiling.
        #[arg(long, value_name = "COUNT|none", value_parser = parse_instruction_limit)]
        instruction_limit: Option<Ceiling<InstructionLimit>>,

        /// What the script may reach.
        #[command(flatten)]
        grants: Grants,

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

    /// Run the Lua test files under a directory.
    ///
    /// A test file is one named `*_test.lua` or `test_*.lua`. It returns a table of named
    /// functions; each is a test, and a test passes by returning without raising — so Lua's own
    /// `assert` is the whole assertion surface and there is nothing new to learn.
    Test {
        /// Which policy preset the tests run under.
        #[arg(long, value_enum, default_value_t = PolicyName::Confined)]
        policy: PolicyName,

        /// What the tests may reach.
        #[command(flatten)]
        grants: Grants,

        /// Directory to search, or a single test file. Defaults to the working directory.
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Compile every Lua file under a path without running any of it.
    ///
    /// `airsl test` covers only what a test file loads, and a hook's entry point is typically
    /// loaded by nothing — so a syntax error there survives a green test run and is then swallowed
    /// by `--fail-open` when the hook fires. This is the check that sees it.
    ///
    /// No policy and no grants: parsing never consults the globals table, and nothing is executed,
    /// so there is no authority to grant.
    Check {
        /// Directory to search, or a single Lua file. Defaults to the working directory.
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Report the runtime version and the policy a script would run under.
    Doctor {
        /// Which policy preset to describe.
        #[arg(long, value_enum, default_value_t = PolicyName::Confined)]
        policy: PolicyName,
    },
}

/// Applies the caller's ceiling overrides to the preset they chose.
///
/// An absent override leaves the preset's ceiling in place; a present one replaces it, including
/// when it lifts the ceiling entirely. "Say nothing" and "say no ceiling" are different
/// instructions, which is why an override is a [`Ceiling`] rather than a bare limit.
pub(crate) fn resolve_policy(
    preset: PolicyName,
    memory: Option<Ceiling<MemoryLimit>>,
    instructions: Option<Ceiling<InstructionLimit>>,
    grants: Grants,
) -> Policy {
    let policy = Policy::from(preset);
    let mut limits = *policy.limits();
    if let Some(memory) = memory {
        limits = limits.with_memory(memory.into_limit());
    }
    if let Some(instructions) = instructions {
        limits = limits.with_instructions(instructions.into_limit());
    }

    // `trusted` waives containment entirely, so adding declared grants to it would narrow nothing
    // and would make `airsl doctor` report a list that means nothing.
    let policy = policy.with_limits(limits);
    if policy.grants().is_unrestricted() {
        policy
    } else {
        policy.with_grants(grants.into_grant_set())
    }
}

/// Parses a memory ceiling in bytes, or `none`.
fn parse_memory_limit(raw: &str) -> Result<Ceiling<MemoryLimit>, String> {
    if raw == UNLIMITED {
        return Ok(Ceiling::Unlimited);
    }
    raw.parse::<usize>()
        .map(|bytes| Ceiling::Of(MemoryLimit::bytes(bytes)))
        .map_err(|_| format!("expected a byte count or `{UNLIMITED}`, got `{raw}`"))
}

/// Parses an instruction ceiling, or `none`.
fn parse_instruction_limit(raw: &str) -> Result<Ceiling<InstructionLimit>, String> {
    if raw == UNLIMITED {
        return Ok(Ceiling::Unlimited);
    }
    raw.parse::<u64>()
        .map(|count| Ceiling::Of(InstructionLimit::count(count)))
        .map_err(|_| format!("expected an instruction count or `{UNLIMITED}`, got `{raw}`"))
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

    use super::{
        Ceiling, Cli, Command, Grants, PolicyName, parse_instruction_limit, parse_memory_limit,
        resolve_policy,
    };
    use airsl::{InstructionLimit, LanguageSurface, MemoryLimit, Policy};
    use clap::Parser as _;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap()
    }

    fn run_command(args: &[&str]) -> Command {
        parse(args).command
    }

    #[test]
    fn run_defaults_to_reporting_errors_and_the_confined_preset() {
        let Command::Run {
            fail_open,
            policy,
            memory_limit,
            instruction_limit,
            script,
            args,
            ..
        } = run_command(&["airsl", "run", "hook.lua"])
        else {
            panic!("expected the run subcommand");
        };
        assert!(!fail_open);
        assert_eq!(policy, PolicyName::Confined);
        assert!(memory_limit.is_none());
        assert!(instruction_limit.is_none());
        assert_eq!(script, std::path::Path::new("hook.lua"));
        assert!(args.is_empty());
    }

    #[test]
    fn fail_open_is_opt_in() {
        let Command::Run { fail_open, .. } = run_command(&["airsl", "run", "--fail-open", "h.lua"])
        else {
            panic!("expected the run subcommand");
        };
        assert!(fail_open);
    }

    #[test]
    fn each_preset_name_selects_its_policy() {
        for (name, expected) in [
            (PolicyName::Trusted, LanguageSurface::Full),
            (PolicyName::Confined, LanguageSurface::Restricted),
            (PolicyName::Pure, LanguageSurface::Minimal),
        ] {
            assert_eq!(Policy::from(name).language(), expected, "{name:?}");
        }
    }

    #[test]
    fn the_policy_flag_selects_a_preset() {
        let Command::Run { policy, .. } =
            run_command(&["airsl", "run", "--policy", "trusted", "h.lua"])
        else {
            panic!("expected the run subcommand");
        };
        assert_eq!(policy, PolicyName::Trusted);
    }

    #[test]
    fn an_unknown_preset_is_a_usage_error() {
        assert!(Cli::try_parse_from(["airsl", "run", "--policy", "wide-open", "h.lua"]).is_err());
    }

    #[test]
    fn a_ceiling_can_be_tightened_or_lifted() {
        let Command::Run { memory_limit, .. } =
            run_command(&["airsl", "run", "--memory-limit", "4096", "h.lua"])
        else {
            panic!("expected the run subcommand");
        };
        assert_eq!(memory_limit, Some(Ceiling::Of(MemoryLimit::bytes(4096))));

        let Command::Run { memory_limit, .. } =
            run_command(&["airsl", "run", "--memory-limit", "none", "h.lua"])
        else {
            panic!("expected the run subcommand");
        };
        assert_eq!(memory_limit, Some(Ceiling::Unlimited));
    }

    #[test]
    fn a_ceiling_that_is_neither_a_number_nor_none_is_a_usage_error() {
        assert!(Cli::try_parse_from(["airsl", "run", "--memory-limit", "lots", "h.lua"]).is_err());
        assert!(
            Cli::try_parse_from(["airsl", "run", "--instruction-limit", "lots", "h.lua"]).is_err()
        );
    }

    #[test]
    fn the_ceiling_parsers_accept_a_count_or_the_unlimited_word() {
        assert_eq!(parse_memory_limit("none").unwrap(), Ceiling::Unlimited);
        assert!(matches!(parse_memory_limit("1").unwrap(), Ceiling::Of(_)));
        assert!(parse_memory_limit("-1").is_err());

        assert_eq!(parse_instruction_limit("none").unwrap(), Ceiling::Unlimited);
        assert!(matches!(
            parse_instruction_limit("1").unwrap(),
            Ceiling::Of(_)
        ));
        assert!(parse_instruction_limit("").is_err());
    }

    #[test]
    fn trailing_arguments_reach_the_script_untouched() {
        let Command::Run { args, .. } =
            run_command(&["airsl", "run", "h.lua", "--verbose", "-x", "value"])
        else {
            panic!("expected the run subcommand");
        };
        assert_eq!(args, ["--verbose", "-x", "value"]);
    }

    #[test]
    fn doctor_describes_the_confined_preset_by_default() {
        let Command::Doctor { policy } = run_command(&["airsl", "doctor"]) else {
            panic!("expected the doctor subcommand");
        };
        assert_eq!(policy, PolicyName::Confined);
    }

    #[test]
    fn doctor_can_describe_another_preset() {
        let Command::Doctor { policy } = run_command(&["airsl", "doctor", "--policy", "pure"])
        else {
            panic!("expected the doctor subcommand");
        };
        assert_eq!(policy, PolicyName::Pure);
    }

    #[test]
    fn no_override_leaves_the_presets_ceilings_alone() {
        let policy = resolve_policy(PolicyName::Confined, None, None, Grants::default());
        assert!(policy.limits().memory().is_some());
        assert!(policy.limits().instructions().is_some());
    }

    #[test]
    fn an_override_replaces_one_ceiling_and_leaves_the_other() {
        let policy = resolve_policy(
            PolicyName::Confined,
            Some(Ceiling::Of(MemoryLimit::bytes(512))),
            None,
            Grants::default(),
        );
        assert_eq!(policy.limits().memory().map(MemoryLimit::get), Some(512));
        assert!(policy.limits().instructions().is_some());
    }

    #[test]
    fn an_override_can_lift_a_ceiling_the_preset_imposed() {
        let policy = resolve_policy(
            PolicyName::Confined,
            Some(Ceiling::Unlimited),
            Some(Ceiling::Unlimited),
            Grants::default(),
        );
        assert!(policy.limits().memory().is_none());
        assert!(policy.limits().instructions().is_none());
    }

    #[test]
    fn an_override_can_impose_a_ceiling_the_preset_lifted() {
        let policy = resolve_policy(
            PolicyName::Trusted,
            None,
            Some(Ceiling::Of(InstructionLimit::count(10))),
            Grants::default(),
        );
        assert_eq!(
            policy.limits().instructions().map(InstructionLimit::get),
            Some(10)
        );
        assert!(policy.limits().memory().is_none());
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
