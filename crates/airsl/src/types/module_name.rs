//! Validated name of a host module, as it appears under the `airsstack` table in Lua.
//!
//! A separate type because module names become Lua table keys and are compared for uniqueness at
//! registration. Validating once at construction means [`crate::modules::HostModule`]
//! implementations and the registry can both assume a usable identifier instead of re-checking a
//! `&str` at every use.
//!
//! Responsibilities: [`ModuleName`] and its [`ModuleName::new`] constructor.
//!
//! Non-responsibilities: uniqueness. A `ModuleName` says the name is *well formed*, not that it is
//! unregistered — that is the registry's job.

use crate::error::{Error, Result};

/// A well-formed host-module name, such as `fs` or `json`.
///
/// Valid names are non-empty, at most 32 bytes, start with a lowercase ASCII letter, and otherwise
/// contain only lowercase ASCII letters, digits and underscores. The restriction keeps every module
/// reachable with Lua's dot syntax — `airsstack.fs.read` rather than `airsstack["fs-utils"].read`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleName(String);

impl ModuleName {
    /// Longest accepted name, in bytes.
    const MAX_LEN: usize = 32;

    /// Validates `raw` and wraps it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] when `raw` is empty, longer than 32 bytes, does not start
    /// with a lowercase ASCII letter, or contains anything other than lowercase ASCII letters,
    /// digits and underscores.
    pub fn new(raw: impl Into<String>) -> Result<Self> {
        let raw = raw.into();
        let invalid = |reason: &'static str| Error::InvalidName {
            kind: "module name",
            value: raw.clone(),
            reason,
        };

        let mut chars = raw.chars();
        let Some(first) = chars.next() else {
            return Err(invalid("must not be empty"));
        };
        if raw.len() > Self::MAX_LEN {
            return Err(invalid("must be at most 32 bytes"));
        }
        if !first.is_ascii_lowercase() {
            return Err(invalid("must start with a lowercase ASCII letter"));
        }
        if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return Err(invalid(
                "must contain only lowercase ASCII letters, digits and underscores",
            ));
        }
        Ok(Self(raw))
    }

    /// The name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for ModuleName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ModuleName {
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

    use super::ModuleName;

    #[test]
    fn accepts_the_names_the_standard_library_uses() {
        for name in [
            "fs", "json", "path", "proc", "re", "hash", "time", "rand", "glob", "log",
        ] {
            assert!(ModuleName::new(name).is_ok(), "{name} should be valid");
        }
    }

    #[test]
    fn accepts_digits_and_underscores_after_the_first_character() {
        assert_eq!(ModuleName::new("base64_v2").unwrap().as_str(), "base64_v2");
    }

    #[test]
    fn rejects_empty_names() {
        assert!(ModuleName::new("").is_err());
    }

    #[test]
    fn rejects_names_that_do_not_start_with_a_lowercase_letter() {
        for name in ["1fs", "_fs", "Fs"] {
            assert!(ModuleName::new(name).is_err(), "{name} should be rejected");
        }
    }

    #[test]
    fn rejects_characters_that_break_lua_dot_syntax() {
        for name in ["fs-utils", "fs.utils", "fs utils", "fsé"] {
            assert!(ModuleName::new(name).is_err(), "{name} should be rejected");
        }
    }

    #[test]
    fn rejects_names_longer_than_the_limit() {
        assert!(ModuleName::new("a".repeat(32)).is_ok());
        assert!(ModuleName::new("a".repeat(33)).is_err());
    }

    #[test]
    fn error_message_names_the_offending_value() {
        let err = ModuleName::new("Fs").unwrap_err();
        assert!(err.to_string().contains("`Fs`"), "{err}");
    }
}
