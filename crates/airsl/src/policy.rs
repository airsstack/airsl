//! What a script is allowed to touch, and what happens when it fails.
//!
//! These two decisions are separated from the engine because they are the caller's to make and
//! must be visible in the type system rather than buried in a convention. The plugin suite this
//! crate was built for has a rule that a hook script must never propagate a non-zero exit — a
//! `PreToolUse` hook that exits 2 blocks the tool call, and the matcher covers `Read`, so a
//! propagated failure blocks every file read in the session. Encoding that as [`FailurePolicy`]
//! makes the choice explicit at every call site instead of relying on each script to remember.
//!
//! Responsibilities:
//!
//! - [`Sandbox`], the set of Lua standard libraries a script may see.
//! - [`FailurePolicy`], how a caller reacts to a script that raised.
//!
//! Non-responsibilities: neither type performs the action it describes. The engine reads
//! [`Sandbox`] while building the Lua state; the caller reads [`FailurePolicy`] after
//! [`crate::Engine::eval`] returns.

/// Which Lua standard libraries a script can reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Sandbox {
    /// The default: `base` without the loaders, plus `string`, `table`, `math`, `coroutine`, and
    /// the pure-computation parts of `os`.
    ///
    /// Withholds `io`, `debug`, `package`, `os.execute`, `os.exit`, `os.getenv`, `os.remove`,
    /// `os.rename`, `os.tmpname`, `os.setlocale`, and the chunk loaders `load`, `loadstring`,
    /// `dofile` and `loadfile`. Every side effect a script needs arrives through the `airsstack`
    /// host modules instead, so the host stays in control of it.
    ///
    /// Withholding `os.setlocale` also keeps string comparison byte-ordered: Lua compares strings
    /// with `strcoll`, which follows the process locale, and nothing sets a locale at startup.
    #[default]
    Restricted,

    /// Every Lua standard library, including `io`, `os` and `debug`.
    ///
    /// For trusted first-party scripts only. A script running under this policy can read and write
    /// arbitrary files and spawn processes without going through a host module, so none of the
    /// containment guarantees the host modules provide apply.
    Full,
}

impl Sandbox {
    /// Whether this policy withholds the chunk loaders and the unsafe standard libraries.
    #[must_use]
    pub const fn is_restricted(self) -> bool {
        matches!(self, Self::Restricted)
    }
}

/// How a caller reacts to a script that failed to load or raised while running.
///
/// This is a description, not an action: [`crate::Engine::eval`] always returns a
/// [`Result`](crate::Result). The policy tells the caller — normally the `airsl` binary — whether
/// to surface that error or discard it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum FailurePolicy {
    /// Report the error on stderr and exit non-zero.
    ///
    /// The default, and correct for anything a person invoked directly: a failure that produces no
    /// output and no message is worse than a loud one.
    #[default]
    Report,

    /// Discard the error, write nothing, and exit zero.
    ///
    /// For scripts run as editor or agent hooks, where a non-zero exit is interpreted as a signal
    /// rather than a diagnostic and can block unrelated work. Diagnostics are still available on
    /// stderr when the caller enables debug output.
    FailOpen,
}

impl FailurePolicy {
    /// Whether a failure under this policy should leave the process exit status at zero.
    #[must_use]
    pub const fn swallows_errors(self) -> bool {
        matches!(self, Self::FailOpen)
    }

    /// The process exit code to use for a script that failed under this policy.
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::FailOpen => 0,
            Self::Report => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FailurePolicy, Sandbox};

    #[test]
    fn restricted_is_the_default_sandbox() {
        assert_eq!(Sandbox::default(), Sandbox::Restricted);
        assert!(Sandbox::default().is_restricted());
        assert!(!Sandbox::Full.is_restricted());
    }

    #[test]
    fn report_is_the_default_failure_policy() {
        assert_eq!(FailurePolicy::default(), FailurePolicy::Report);
    }

    #[test]
    fn fail_open_swallows_errors_and_exits_zero() {
        assert!(FailurePolicy::FailOpen.swallows_errors());
        assert_eq!(FailurePolicy::FailOpen.exit_code(), 0);
    }

    #[test]
    fn report_surfaces_errors_and_exits_nonzero() {
        assert!(!FailurePolicy::Report.swallows_errors());
        assert_ne!(FailurePolicy::Report.exit_code(), 0);
    }
}
