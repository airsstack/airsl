//! The extension api version a manifest pins.
//!
//! Its own module because the versioning rule is a promise to every extension author, and a
//! promise needs one place to be stated: **within one api number the contract is additive** —
//! host modules and events gain names and never lose or change them — and a manifest pinning a
//! number this runtime does not implement is refused at load rather than run against a
//! contract it was not written for.
//!
//! Responsibilities: [`ApiVersion`], [`SUPPORTED_API`], and [`ApiVersion::supported`].
//!
//! Non-responsibilities: what changed between versions. That is the changelog's job.

use crate::error::{Error, Result};

/// An extension api version number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApiVersion(u32);

/// Every api version this runtime implements.
pub const SUPPORTED_API: &[ApiVersion] = &[ApiVersion(1)];

impl ApiVersion {
    /// Wraps `version` if this runtime implements it.
    ///
    /// The only way to obtain an `ApiVersion` — deliberately no unchecked constructor, not even a
    /// crate-private one. `SUPPORTED_API` itself is built with the tuple constructor in this
    /// module, which is the sole place a value can be minted without going through this check, so
    /// the newtype's invariant — every `ApiVersion` a caller holds is one this runtime implements
    /// — always holds outside this file.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnsupportedApi`] naming the supported set otherwise.
    pub fn supported(version: u32) -> Result<Self> {
        let candidate = Self(version);
        if SUPPORTED_API.contains(&candidate) {
            Ok(candidate)
        } else {
            Err(Error::UnsupportedApi {
                requested: version,
                supported: SUPPORTED_API.iter().map(|v| v.0).collect(),
            })
        }
    }

    /// The number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl core::fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{ApiVersion, SUPPORTED_API};

    #[test]
    fn version_one_is_supported() {
        assert_eq!(ApiVersion::supported(1).unwrap().get(), 1);
    }

    #[test]
    fn an_unknown_version_is_refused_and_the_message_lists_the_supported_set() {
        let err = ApiVersion::supported(99).unwrap_err();
        assert!(err.to_string().contains("supports api 1"), "{err}");
    }

    #[test]
    fn the_supported_set_is_never_empty() {
        assert!(!SUPPORTED_API.is_empty());
    }
}
