//! Validated name of an extension, as written in its manifest.
//!
//! A separate type because the name is how a host refers to a loaded extension — in its
//! registry, its logs and its error messages — and because it is spelled by a third party.
//! Hyphenated lowercase is the shape package registries use, so `journal-indexer` is valid
//! where a [`crate::ModuleName`] would refuse the hyphen.
//!
//! Responsibilities: [`ExtensionName`] and its [`ExtensionName::new`] constructor.
//!
//! Non-responsibilities: uniqueness among loaded extensions. That is the host's registry.

use crate::error::{Error, Result};

/// A well-formed extension name, such as `journal-indexer`.
///
/// Valid names are 1–64 bytes of lowercase ASCII letters, digits and single hyphens, neither
/// starting nor ending with a hyphen.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtensionName(String);

impl ExtensionName {
    /// Longest accepted name, in bytes.
    const MAX_LEN: usize = 64;

    /// Validates `raw` and wraps it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] when `raw` is empty, longer than 64 bytes, contains
    /// anything other than lowercase ASCII letters, digits and hyphens, starts or ends with a
    /// hyphen, or contains two hyphens in a row.
    pub fn new(raw: impl Into<String>) -> Result<Self> {
        let raw = raw.into();
        let invalid = |reason: &'static str| Error::InvalidName {
            kind: "extension name",
            value: raw.clone(),
            reason,
        };

        if raw.is_empty() {
            return Err(invalid("must not be empty"));
        }
        if raw.len() > Self::MAX_LEN {
            return Err(invalid("must be at most 64 bytes"));
        }
        if !raw
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(invalid(
                "must contain only lowercase ASCII letters, digits and hyphens",
            ));
        }
        if raw.starts_with('-') || raw.ends_with('-') {
            return Err(invalid("must not start or end with a hyphen"));
        }
        if raw.contains("--") {
            return Err(invalid("must not contain consecutive hyphens"));
        }
        Ok(Self(raw))
    }

    /// The name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for ExtensionName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ExtensionName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::ExtensionName;

    #[test]
    fn accepts_registry_style_names() {
        for name in ["journal-indexer", "backlinks", "v2-tool", "a"] {
            assert!(ExtensionName::new(name).is_ok(), "{name} should be valid");
        }
    }

    #[test]
    fn rejects_empty_and_overlong_names() {
        assert!(ExtensionName::new("").is_err());
        assert!(ExtensionName::new("a".repeat(64)).is_ok());
        assert!(ExtensionName::new("a".repeat(65)).is_err());
    }

    #[test]
    fn rejects_uppercase_underscores_and_dots() {
        for name in ["Journal", "journal_indexer", "journal.indexer", "jé"] {
            assert!(
                ExtensionName::new(name).is_err(),
                "{name} should be rejected"
            );
        }
    }

    #[test]
    fn rejects_edge_and_double_hyphens() {
        for name in ["-x", "x-", "a--b"] {
            assert!(
                ExtensionName::new(name).is_err(),
                "{name} should be rejected"
            );
        }
    }

    #[test]
    fn error_message_names_the_kind_and_the_value() {
        let text = ExtensionName::new("Bad").unwrap_err().to_string();
        assert!(
            text.contains("extension name") && text.contains("`Bad`"),
            "{text}"
        );
    }
}
