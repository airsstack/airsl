//! How a caller reacts to a script that failed to load or raised while running.
//!
//! Separate from the sandbox because it describes the *caller's* behaviour rather than the
//! script's permissions, and the two are read at different times: the engine reads a policy while
//! building a state, and the caller reads this after evaluation has already returned. The plugin
//! suite this crate was built for has a rule that a hook script must never propagate a non-zero
//! exit — a `PreToolUse` hook that exits 2 blocks the tool call, and the matcher covers `Read`, so
//! a propagated failure blocks every file read in the session. Encoding that in a type makes the
//! choice explicit at every call site instead of relying on each script to remember.
//!
//! Responsibilities: [`FailurePolicy`], and the exit code each variant implies.
//!
//! Non-responsibilities: performing the reaction. [`crate::Engine::eval`] always returns a
//! [`Result`](crate::Result); this type only says how to read it.

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
    use super::FailurePolicy;

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
