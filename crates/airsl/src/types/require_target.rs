//! Validated module name a script may pass to `require`.
//!
//! Its own type because the containment guarantee starts here rather than at the filesystem. A
//! name that cannot contain a path separator or a parent reference cannot describe a file outside
//! the script's directory, so `require("../secrets")` is rejected as unrepresentable instead of
//! being caught later by a path check. What remains for the filesystem to decide is the one case
//! it alone can see: a symlink inside the root that points out of it.
//!
//! Responsibilities: [`RequireTarget`], its constructor, and the file names it resolves to.
//!
//! Non-responsibilities: opening anything, and deciding whether the result is inside the root.
//! Both belong to the loader.

use std::path::PathBuf;

use crate::error::{Error, Result};

/// Longest accepted target, in bytes.
const MAX_LEN: usize = 128;

/// A well-formed `require` target, such as `lib.index`.
///
/// Valid targets are non-empty, at most 128 bytes, and contain only ASCII letters, digits,
/// underscores, hyphens and dots. A dot separates path components. Leading and trailing dots and
/// any empty component are rejected, which is what makes `..` unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequireTarget(String);

impl RequireTarget {
    /// Validates `raw` and wraps it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] when `raw` is empty, too long, contains a character outside
    /// the accepted set — notably a path separator — or has an empty dot-separated component.
    pub fn new(raw: impl Into<String>) -> Result<Self> {
        let raw = raw.into();
        let invalid = |reason: &'static str| Error::InvalidName {
            kind: "require target",
            value: raw.clone(),
            reason,
        };

        if raw.is_empty() {
            return Err(invalid("must not be empty"));
        }
        if raw.len() > MAX_LEN {
            return Err(invalid("must be at most 128 bytes"));
        }
        if !raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(invalid(
                "must contain only ASCII letters, digits, underscores, hyphens and dots",
            ));
        }
        if raw.split('.').any(str::is_empty) {
            return Err(invalid(
                "must not begin or end with a dot, or contain an empty component",
            ));
        }
        Ok(Self(raw))
    }

    /// The target as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The paths this target may name, relative to the script root, in search order.
    ///
    /// A directory module resolves through `init.lua`, which is the convention the wider Lua
    /// ecosystem uses and what lets a multi-file extension keep its parts in a subdirectory.
    #[must_use]
    pub(crate) fn candidates(&self) -> [PathBuf; 2] {
        let stem = self.0.replace('.', "/");
        [
            PathBuf::from(format!("{stem}.lua")),
            PathBuf::from(format!("{stem}/init.lua")),
        ]
    }
}

impl core::fmt::Display for RequireTarget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::RequireTarget;
    use std::path::Path;

    #[test]
    fn an_ordinary_target_is_accepted() {
        for name in ["index", "lib.index", "front_matter", "a-b", "x1"] {
            assert!(RequireTarget::new(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn a_path_separator_is_rejected_so_an_escape_cannot_be_spelled() {
        for name in ["../secrets", "/etc/passwd", "lib/index", "a\\b"] {
            assert!(RequireTarget::new(name).is_err(), "{name}");
        }
    }

    #[test]
    fn a_parent_reference_is_rejected_as_an_empty_component() {
        for name in ["..", "a..b", ".hidden", "trailing."] {
            let err = RequireTarget::new(name).unwrap_err();
            assert!(err.to_string().contains("dot"), "{name}: {err}");
        }
    }

    #[test]
    fn an_empty_target_is_rejected() {
        assert!(RequireTarget::new("").is_err());
    }

    #[test]
    fn an_overlong_target_is_rejected() {
        assert!(RequireTarget::new("a".repeat(129)).is_err());
        assert!(RequireTarget::new("a".repeat(128)).is_ok());
    }

    #[test]
    fn a_nul_byte_is_rejected() {
        assert!(RequireTarget::new("a\0b").is_err());
    }

    #[test]
    fn a_dot_becomes_a_directory_separator() {
        let target = RequireTarget::new("lib.index").unwrap();
        assert_eq!(target.candidates()[0], Path::new("lib/index.lua"));
    }

    #[test]
    fn a_directory_module_resolves_through_init() {
        let target = RequireTarget::new("lib").unwrap();
        assert_eq!(target.candidates()[1], Path::new("lib/init.lua"));
    }

    #[test]
    fn a_target_renders_as_it_was_written() {
        assert_eq!(
            RequireTarget::new("lib.index").unwrap().to_string(),
            "lib.index"
        );
    }
}
