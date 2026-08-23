//! Validated name of a host event an extension may subscribe to.
//!
//! A separate type because event names are the contract between a host program and every
//! extension it loads: they are spelled in Lua (`ext.on("note_saved", ...)`) and in Rust
//! (`engine.dispatch(&event, ...)`), and a mismatch in either place is a handler that silently
//! never fires. Validating once at construction means the dispatcher and the `ext` module can
//! compare names rather than re-check strings.
//!
//! Responsibilities: [`EventName`] and its [`EventName::new`] constructor.
//!
//! Non-responsibilities: whether a host declared the event. That is the host's list to keep.

use crate::error::{Error, Result};

/// A well-formed event name, such as `note_saved` or `session_start`.
///
/// Valid names are non-empty, at most 64 bytes, start with a lowercase ASCII letter, and
/// otherwise contain only lowercase ASCII letters, digits and underscores. The restriction keeps
/// a name usable as a plain Lua string key and readable in a manifest or a log line.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventName(String);

impl EventName {
    /// Longest accepted name, in bytes.
    const MAX_LEN: usize = 64;

    /// Validates `raw` and wraps it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] when `raw` is empty, longer than 64 bytes, does not start
    /// with a lowercase ASCII letter, or contains anything other than lowercase ASCII letters,
    /// digits and underscores.
    pub fn new(raw: impl Into<String>) -> Result<Self> {
        let raw = raw.into();
        let invalid = |reason: &'static str| Error::InvalidName {
            kind: "event name",
            value: raw.clone(),
            reason,
        };

        let mut chars = raw.chars();
        let Some(first) = chars.next() else {
            return Err(invalid("must not be empty"));
        };
        if raw.len() > Self::MAX_LEN {
            return Err(invalid("must be at most 64 bytes"));
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

impl core::fmt::Display for EventName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for EventName {
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

    use super::EventName;

    #[test]
    fn accepts_the_shapes_a_host_would_declare() {
        for name in ["session_start", "note_saved", "query", "pre_tool_use2"] {
            assert!(EventName::new(name).is_ok(), "{name} should be valid");
        }
    }

    #[test]
    fn rejects_empty_names() {
        assert!(EventName::new("").is_err());
    }

    #[test]
    fn rejects_names_that_do_not_start_with_a_lowercase_letter() {
        for name in ["1start", "_start", "Start"] {
            assert!(EventName::new(name).is_err(), "{name} should be rejected");
        }
    }

    #[test]
    fn rejects_separators_other_than_underscore() {
        for name in ["note-saved", "note.saved", "note saved", "noteé"] {
            assert!(EventName::new(name).is_err(), "{name} should be rejected");
        }
    }

    #[test]
    fn rejects_names_longer_than_the_limit() {
        assert!(EventName::new("a".repeat(64)).is_ok());
        assert!(EventName::new("a".repeat(65)).is_err());
    }

    #[test]
    fn error_message_names_the_kind_and_the_value() {
        let err = EventName::new("Bad").unwrap_err();
        let text = err.to_string();
        assert!(text.contains("event name"), "{text}");
        assert!(text.contains("`Bad`"), "{text}");
    }
}
