//! What the host modules a script reaches are permitted to touch.
//!
//! Its own type, rather than a field on the policy, because a grant is parameterised and a
//! parameterised thing needs somewhere to keep its parameters. The distinction this axis exists to
//! draw is not "may this script touch files" but "may it touch *these* files", and a boolean
//! cannot express that.
//!
//! Responsibilities: [`GrantSet`] and the two reaches it can describe.
//!
//! Non-responsibilities: enforcement. A grant is a promise the host module keeps, checked inside
//! the Rust function before the operation it guards. Nothing here reaches the VM.

/// How far the grants in a [`GrantSet`] reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum GrantReach {
    /// Only what the host declared, and nothing beyond it.
    #[default]
    Declared,
    /// Everything the host itself can reach.
    Unrestricted,
}

/// The authority a policy extends to the host modules a script can reach.
///
/// Currently a policy is either unrestricted or holds no declared grants at all, because no host
/// module yet takes one — the vocabulary of grants is the list of modules that need them, and
/// `airsstack.json` needs none. The type exists ahead of that vocabulary so the shape of a
/// [`Policy`](crate::Policy) is settled: declared grants become a list inside this type rather than
/// a new field on the policy, which is a change no caller has to see.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct GrantSet {
    reach: GrantReach,
}

impl GrantSet {
    /// Only the grants the host declared.
    ///
    /// No module takes a grant yet, so this currently permits nothing beyond what a module needs
    /// no authority for.
    #[must_use]
    pub const fn declared() -> Self {
        Self {
            reach: GrantReach::Declared,
        }
    }

    /// Everything the host process itself can reach.
    ///
    /// For first-party code trusted to the same degree as the host. A module consulting this set
    /// performs no containment check at all.
    #[must_use]
    pub const fn unrestricted() -> Self {
        Self {
            reach: GrantReach::Unrestricted,
        }
    }

    /// Whether this set waives containment entirely.
    #[must_use]
    pub const fn is_unrestricted(&self) -> bool {
        matches!(self.reach, GrantReach::Unrestricted)
    }

    /// Whether this set grants nothing beyond what needs no authority.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        matches!(self.reach, GrantReach::Declared)
    }
}

impl core::fmt::Display for GrantSet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(if self.is_unrestricted() {
            "unrestricted"
        } else {
            "none"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::GrantSet;

    #[test]
    fn declared_is_the_default_reach() {
        assert_eq!(GrantSet::default(), GrantSet::declared());
    }

    #[test]
    fn declared_grants_nothing_and_waives_nothing() {
        let grants = GrantSet::declared();
        assert!(grants.is_empty());
        assert!(!grants.is_unrestricted());
    }

    #[test]
    fn unrestricted_waives_containment() {
        let grants = GrantSet::unrestricted();
        assert!(grants.is_unrestricted());
        assert!(!grants.is_empty());
    }

    #[test]
    fn the_two_reaches_are_distinguishable() {
        assert_ne!(GrantSet::declared(), GrantSet::unrestricted());
    }

    #[test]
    fn each_reach_renders_for_a_report() {
        assert_eq!(GrantSet::declared().to_string(), "none");
        assert_eq!(GrantSet::unrestricted().to_string(), "unrestricted");
    }
}
