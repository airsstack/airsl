//! Which `require` a script is given, decided before an engine installs anything.
//!
//! Its own module because the decision is a pure function of the language surface, the grant set
//! and the script's root, and the engine acts on the answer; keeping it apart from the loader that
//! carries out one of the four outcomes lets it be tested without a VM.
//!
//! Responsibilities:
//!
//! - [`RequireDisposition`], which of the four possible `require` globals a script should get.
//!
//! Non-responsibilities: installing any of them, which is [`super::RequireLoader`], and applying
//! the disposition, which [`crate::Engine`] does because it holds the state.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::Path;

use crate::sandbox::{GrantSet, HostAccess, LanguageSurface};

/// Which `require` a script should be given, before an engine goes and installs it.
///
/// [`LanguageSurface::Full`] keeps Lua's own loader whatever the grant or the root. Below it, the
/// `require` grant decides: not granted gives a function that refuses every call but the root
/// table's; granted with a directory to resolve against gives the confined loader; granted with no
/// directory gives a function that answers only the root table's name.
///
/// An enum rather than a predicate because those are four different outcomes, and "should this
/// script get a *confined* `require`" would fold three of them onto one `false`. A refusing
/// function, a root-only one and Lua's own loader each tell the script something different, and
/// the engine has to act differently for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequireDisposition<'a> {
    /// Leave Lua's own `require` exactly as `luaopen_package` installed it.
    Native,

    /// Install a `require` resolving under this directory and nowhere else.
    Confined(&'a Path),

    /// Install a `require` that answers the root table's name and refuses every other call, because
    /// the grant set does not hold the `require` member.
    Refused,

    /// Install a `require` that answers only the root table's name and refuses every other with
    /// [`crate::Error::RequireNoDirectory`]. The member is granted but the script has no directory
    /// to resolve against.
    RootOnly,
}

impl<'a> RequireDisposition<'a> {
    /// The disposition for a script with `root`, running on `surface` under `grants`.
    ///
    /// `Full` keeps Lua's own, which resolves through `package.path` and is deliberately left
    /// alone — a first-party script may depend on it, and the surface documents `require` as part
    /// of what it grants — whatever the grant set holds. Below `Full` the answer follows the
    /// [`HostAccess::Require`] member: without it the script gets a `require` that refuses every
    /// call but the root table's; with it the script gets the confined loader, but only where there
    /// is a directory to confine it to, so a script built from source gets the `require` that
    /// answers only the root.
    ///
    /// The match is exhaustive on purpose. A fourth surface should not be able to acquire a
    /// default here by falling into a wildcard; it should fail to compile until someone decides.
    pub(crate) const fn decide(
        surface: LanguageSurface,
        root: Option<&'a Path>,
        grants: &GrantSet,
    ) -> Self {
        match surface {
            LanguageSurface::Full => Self::Native,
            LanguageSurface::Restricted | LanguageSurface::Minimal => {
                match (grants.allows(HostAccess::Require), root) {
                    (false, _) => Self::Refused,
                    (true, Some(root)) => Self::Confined(root),
                    (true, None) => Self::RootOnly,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RequireDisposition;
    use crate::sandbox::{GrantSet, LanguageSurface};
    use std::path::Path;

    #[test]
    fn a_granted_require_is_confined_on_restricted_and_minimal() {
        let root = Path::new("/scripts");
        for surface in [LanguageSurface::Restricted, LanguageSurface::Minimal] {
            assert_eq!(
                RequireDisposition::decide(surface, Some(root), &GrantSet::declared()),
                RequireDisposition::Confined(root),
                "{surface}"
            );
        }
    }

    #[test]
    fn the_full_surface_keeps_lua_s_own_require_whether_or_not_the_script_has_a_root() {
        // `Full` keeps Lua's own `require` whatever the `require` grant and the script's root say:
        // the confined loader and the refusing stub both belong only to the surfaces below it.
        for grants in [GrantSet::sealed(), GrantSet::declared()] {
            for root in [None, Some(Path::new("/scripts"))] {
                assert_eq!(
                    RequireDisposition::decide(LanguageSurface::Full, root, &grants),
                    RequireDisposition::Native,
                    "{root:?}"
                );
            }
        }
    }

    #[test]
    fn a_granted_script_with_no_directory_gets_a_require_that_answers_only_the_root() {
        for surface in [LanguageSurface::Restricted, LanguageSurface::Minimal] {
            assert_eq!(
                RequireDisposition::decide(surface, None, &GrantSet::declared()),
                RequireDisposition::RootOnly,
                "{surface}"
            );
        }
    }

    #[test]
    fn an_ungranted_require_is_refused_on_restricted_and_minimal() {
        for surface in [LanguageSurface::Restricted, LanguageSurface::Minimal] {
            for root in [None, Some(Path::new("/scripts"))] {
                assert_eq!(
                    RequireDisposition::decide(surface, root, &GrantSet::sealed()),
                    RequireDisposition::Refused,
                    "{surface} {root:?}"
                );
            }
        }
    }
}
