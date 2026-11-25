//! The check and the refusal text for a [`HostAccess`] member a policy does not grant.
//!
//! Its own module because every host module that gates on a member needs the same check and the
//! same words, and with both written here a refusal cannot name one member while the check tested
//! another. It sits beside `guard.rs` for the same reason: one place for the rule, so a mistake is
//! visible in one file.
//!
//! Responsibilities: [`check`], which passes a granted member and refuses an ungranted one;
//! [`check_subject`], which does the same and names the [`Subject`] that needed the member; and
//! [`detail`], which writes the reason a plain refusal gives.
//!
//! Non-responsibilities: deciding which member a function needs, which is the calling module's
//! knowledge, and performing the operation after the check passes.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use crate::error::{Error, Result};
use crate::sandbox::{GrantSet, HostAccess};

/// What the grant set does hold, as the tail every refusal ends with.
///
/// Either "the granted access is" and the members in [`HostAccess::ALL`] order, or "no access is"
/// when it holds none. The empty tail mirrors what `env` and `proc` say when they hold no entries.
fn granted(grants: &GrantSet) -> String {
    let members: Vec<&str> = grants.access().map(HostAccess::as_str).collect();
    if members.is_empty() {
        String::from("no access is")
    } else {
        format!("the granted access is {}", members.join(", "))
    }
}

/// The reason a refusal gives when `member` is not granted.
///
/// Reads `` `member` is not granted — `` and then what the set does hold.
pub(crate) fn detail(grants: &GrantSet, member: HostAccess) -> String {
    format!("`{member}` is not granted — {}", granted(grants))
}

/// The thing a call asked for that needs a member the policy does not grant.
///
/// A closed set rather than a string so that every refusal naming a subject words it the same way.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Subject<'a> {
    /// A time zone the script named, exactly as it passed it.
    NamedZone(&'a str),
    /// A relative path the script passed, exactly as it wrote it.
    RelativePath(&'a str),
}

impl std::fmt::Display for Subject<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NamedZone(name) => write!(f, "the named time zone `{name}`"),
            Self::RelativePath(raw) => write!(f, "the relative path `{raw}`"),
        }
    }
}

/// Passes when `grants` holds `member`.
///
/// `module` and `operation` name the Lua function that asked, so the refusal reads as that
/// function's own.
///
/// # Errors
///
/// [`Error::Denied`] when the set does not hold `member`.
pub(crate) fn check(
    grants: &GrantSet,
    module: &'static str,
    operation: &'static str,
    member: HostAccess,
) -> Result<()> {
    if grants.allows(member) {
        Ok(())
    } else {
        Err(Error::Denied {
            module,
            operation,
            detail: detail(grants, member),
        })
    }
}

/// Passes when `grants` holds `member`; otherwise refuses, naming the `subject` that needed it.
///
/// `module` and `operation` name the Lua function that asked, as for [`check`].
///
/// # Errors
///
/// [`Error::Denied`] when the set does not hold `member`.
pub(crate) fn check_subject(
    grants: &GrantSet,
    module: &'static str,
    operation: &'static str,
    member: HostAccess,
    subject: Subject<'_>,
) -> Result<()> {
    if grants.allows(member) {
        Ok(())
    } else {
        Err(Error::Denied {
            module,
            operation,
            detail: format!(
                "{subject} needs `{member}`, which is not granted — {}",
                granted(grants)
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{Subject, check, check_subject, detail};
    use crate::sandbox::{GrantSet, HostAccess};

    #[test]
    fn shape_a_with_nothing_granted_says_no_access_is() {
        assert_eq!(
            detail(&GrantSet::sealed(), HostAccess::Stdout),
            "`stdout` is not granted — no access is"
        );
    }

    #[test]
    fn shape_a_lists_the_granted_members_in_order() {
        let forward = GrantSet::sealed()
            .with_access(HostAccess::Stderr)
            .with_access(HostAccess::Require);
        let backward = GrantSet::sealed()
            .with_access(HostAccess::Require)
            .with_access(HostAccess::Stderr);
        for grants in [forward, backward] {
            assert_eq!(
                detail(&grants, HostAccess::Stdout),
                "`stdout` is not granted — the granted access is stderr, require"
            );
        }
    }

    #[test]
    fn check_passes_a_granted_member_and_an_unrestricted_set() {
        check(&GrantSet::declared(), "stdio", "read", HostAccess::Stdin).unwrap();
        check(
            &GrantSet::unrestricted(),
            "stdio",
            "read",
            HostAccess::Stdin,
        )
        .unwrap();
        check(
            &GrantSet::sealed().with_access(HostAccess::Stdin),
            "stdio",
            "read",
            HostAccess::Stdin,
        )
        .unwrap();
    }

    #[test]
    fn check_refuses_an_ungranted_member_under_the_callers_own_name() {
        let err = check(&GrantSet::sealed(), "stdio", "read", HostAccess::Stdin).unwrap_err();
        assert_eq!(
            err.to_string(),
            "stdio.read denied: `stdin` is not granted — no access is"
        );
    }

    #[test]
    fn shape_b_names_the_zone() {
        let err = check_subject(
            &GrantSet::sealed(),
            "time",
            "parse",
            HostAccess::Zones,
            Subject::NamedZone("America/New_York"),
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "time.parse denied: the named time zone `America/New_York` needs `zones`, which is \
             not granted — no access is"
        );
    }

    #[test]
    fn shape_b_names_the_relative_path() {
        let err = check_subject(
            &GrantSet::sealed().with_access(HostAccess::Stderr),
            "path",
            "absolute",
            HostAccess::Cwd,
            Subject::RelativePath("p.lua"),
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "path.absolute denied: the relative path `p.lua` needs `cwd`, which is not granted — \
             the granted access is stderr"
        );
    }

    #[test]
    fn shape_b_lists_the_granted_members_and_passes_a_granted_one() {
        let grants = GrantSet::sealed().with_access(HostAccess::Clock);
        let err = check_subject(
            &grants,
            "time",
            "parse",
            HostAccess::Zones,
            Subject::NamedZone("UTC"),
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "time.parse denied: the named time zone `UTC` needs `zones`, which is not granted — \
             the granted access is clock"
        );
        check_subject(
            &grants.with_access(HostAccess::Zones),
            "time",
            "parse",
            HostAccess::Zones,
            Subject::NamedZone("UTC"),
        )
        .unwrap();
    }
}
