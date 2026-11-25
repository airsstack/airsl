//! The host capabilities a script may reach without naming a path, a variable or a program.
//!
//! Its own module, rather than a qualifier file of [`super::grants`], because these members are one
//! vocabulary read by Rust callers, the CLI, `doctor` and every refusal, and none of them carries a
//! parameter. `grants` documents itself as the parameterised allowlists, so an unparameterised
//! member has no home there; the seven names sit beside the language surface and the resource
//! ceilings instead.
//!
//! Responsibilities:
//!
//! - [`HostAccess`], the seven members.
//! - Their stable lowercase names, and the parse of a name back into a member.
//! - Their declaration-order listing, [`HostAccess::ALL`].
//!
//! Non-responsibilities: holding a set of members (that is [`super::GrantSet`]) and checking one
//! (that is each host module, before the operation it guards).

use core::fmt;
use core::str::FromStr;

use crate::error::{Error, Result};

/// One unparameterised host capability a [`super::GrantSet`] can hold.
///
/// Host access bounds the host modules and the confined `require`; Lua's own globals belong to the
/// [`super::LanguageSurface`]. Members are ordered by declaration, and every listing uses that
/// order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum HostAccess {
    /// Reading the process's standard input.
    Stdin,
    /// Writing to standard output, and asking whether it is a terminal.
    Stdout,
    /// Writing to standard error.
    Stderr,
    /// Reading the wall clock and the monotonic clock.
    Clock,
    /// Resolving a relative path against the working directory.
    Cwd,
    /// Loading modules through `require` from the script's folder.
    Require,
    /// Parsing times with a pattern that names an IANA time zone.
    Zones,
}

impl HostAccess {
    /// Every member, in declaration order.
    pub const ALL: [Self; 7] = [
        Self::Stdin,
        Self::Stdout,
        Self::Stderr,
        Self::Clock,
        Self::Cwd,
        Self::Require,
        Self::Zones,
    ];

    /// Every member's bit at once, as [`GrantSet`](super::GrantSet) stores them.
    pub(crate) const ALL_BITS: u8 = (1 << Self::ALL.len()) - 1;

    /// The member's bit in a [`GrantSet`](super::GrantSet)'s set: its declaration index.
    pub(crate) const fn bit(self) -> u8 {
        1 << (self as u8)
    }

    /// The member's lowercase name, as the CLI and `doctor` spell it.
    ///
    /// # Examples
    ///
    /// ```
    /// use airsl::HostAccess;
    ///
    /// assert_eq!(HostAccess::Clock.as_str(), "clock");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stdin => "stdin",
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
            Self::Clock => "clock",
            Self::Cwd => "cwd",
            Self::Require => "require",
            Self::Zones => "zones",
        }
    }
}

impl fmt::Display for HostAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for HostAccess {
    type Err = Error;

    /// Parses one of the seven lowercase names.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] for any other input, including the empty string and a
    /// differently cased name, with the accepted names in the reason.
    ///
    /// # Examples
    ///
    /// ```
    /// use airsl::HostAccess;
    ///
    /// assert_eq!("stdout".parse::<HostAccess>().unwrap(), HostAccess::Stdout);
    /// assert!("STDOUT".parse::<HostAccess>().is_err());
    /// ```
    fn from_str(raw: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|member| member.as_str() == raw)
            .ok_or_else(|| Error::InvalidName {
                kind: "host access",
                value: raw.to_owned(),
                reason: "expected one of stdin, stdout, stderr, clock, cwd, require, zones",
            })
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::HostAccess;
    use crate::Error;

    #[test]
    fn every_member_round_trips_through_its_name() {
        for member in HostAccess::ALL {
            assert_eq!(member.as_str().parse::<HostAccess>().unwrap(), member);
            assert_eq!(member.to_string(), member.as_str());
        }
    }

    #[test]
    fn parsing_rejects_an_unknown_name_with_the_expected_list() {
        for input in ["bogus", "", "STDOUT"] {
            let err = input.parse::<HostAccess>().unwrap_err();
            assert!(matches!(err, Error::InvalidName { .. }), "{input:?}: {err}");
        }
        assert_eq!(
            "bogus".parse::<HostAccess>().unwrap_err().to_string(),
            "invalid host access `bogus`: expected one of stdin, stdout, stderr, clock, cwd, require, zones"
        );
    }

    #[test]
    fn all_lists_the_members_in_declaration_order() {
        assert_eq!(
            HostAccess::ALL,
            [
                HostAccess::Stdin,
                HostAccess::Stdout,
                HostAccess::Stderr,
                HostAccess::Clock,
                HostAccess::Cwd,
                HostAccess::Require,
                HostAccess::Zones,
            ]
        );
        assert!(HostAccess::ALL.is_sorted());
    }
}
