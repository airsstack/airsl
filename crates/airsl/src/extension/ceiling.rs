//! The host's statement of maximum authority, which no manifest may exceed.
//!
//! Its own type rather than a bare [`Policy`] because a ceiling has to *be* a bound: a policy
//! with a full language surface would hand every extension native `require` and `io`, and one
//! with unrestricted grants would make the intersection equal the request — a ceiling only in
//! name. Refusing those at construction means every later intersection can assume the ceiling
//! is meaningful.
//!
//! Responsibilities: [`Ceiling`] and its [`Ceiling::new`] check.
//!
//! Non-responsibilities: the intersection itself ([`mod@crate::extension::negotiate`]).

use crate::error::{Error, Result};
use crate::sandbox::{LanguageSurface, Policy};

/// A validated maximum: `Restricted` or `Minimal` language, declared grants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ceiling(Policy);

impl Ceiling {
    /// Accepts `policy` as a ceiling.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CeilingUnbounded`] when the language surface is
    /// [`LanguageSurface::Full`] or the grants are unrestricted.
    pub fn new(policy: Policy) -> Result<Self> {
        if policy.language() == LanguageSurface::Full {
            return Err(Error::CeilingUnbounded {
                reason: "language surface is full",
            });
        }
        if policy.grants().is_unrestricted() {
            return Err(Error::CeilingUnbounded {
                reason: "grants are unrestricted",
            });
        }
        Ok(Self(policy))
    }

    /// The policy that is the bound.
    #[must_use]
    pub const fn policy(&self) -> &Policy {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Ceiling;
    use crate::{GrantSet, LanguageSurface, Policy};

    #[test]
    fn confined_and_pure_are_ceilings() {
        assert!(Ceiling::new(Policy::confined()).is_ok());
        assert!(Ceiling::new(Policy::pure()).is_ok());
    }

    #[test]
    fn trusted_is_refused_for_its_grants_and_its_language() {
        let err = Ceiling::new(Policy::trusted()).unwrap_err();
        assert!(err.to_string().contains("language surface"), "{err}");

        let declared_but_full = Policy::trusted().with_grants(GrantSet::declared());
        let err = Ceiling::new(declared_but_full).unwrap_err();
        assert!(err.to_string().contains("language surface"), "{err}");

        let restricted_but_unrestricted = Policy::confined()
            .with_language(LanguageSurface::Restricted)
            .with_grants(GrantSet::unrestricted());
        let err = Ceiling::new(restricted_but_unrestricted).unwrap_err();
        assert!(err.to_string().contains("unrestricted"), "{err}");
    }

    #[test]
    fn a_ceiling_hands_back_the_policy_it_was_built_from() {
        let policy = Policy::confined();
        assert_eq!(Ceiling::new(policy.clone()).unwrap().policy(), &policy);
    }
}
