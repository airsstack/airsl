//! Validated name of a Lua identifier a host describes: a module, a function, a parameter, a record
//! or a record field.
//!
//! A separate type from [`ModuleName`](crate::ModuleName) because the two answer different
//! questions. A module name is something the host picks and constrains tightly so it stays
//! reachable with dot syntax; a Lua name is any identifier Lua itself would accept in a name
//! position, including capitalised and mixed-case ones such as `hookEventName`. Owning the Lua
//! reserved-word list here lets [`RootTable`](crate::RootTable) and this type share one copy.
//!
//! Responsibilities:
//!
//! - [`LuaName`] and its [`LuaName::new`] constructor.
//! - `RESERVED_WORDS`, the lowercase words Lua 5.4 refuses as names.
//!
//! Non-responsibilities: uniqueness within a module or record, and any claim that the name exists
//! in a running VM. A `LuaName` says the text is a usable identifier, nothing more.

use crate::error::{Error, Result};

/// Lua 5.4's reserved words, which cannot appear where a name is expected.
///
/// All of them are lowercase, so a capitalised spelling such as `End` is an ordinary identifier.
pub(crate) const RESERVED_WORDS: [&str; 22] = [
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

/// A well-formed Lua identifier that is not a reserved word.
///
/// Valid names are non-empty, start with an ASCII letter or `_`, and otherwise contain only ASCII
/// letters, digits and underscores. Case is preserved and not restricted.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LuaName(String);

impl LuaName {
    /// Validates `raw` and wraps it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] with kind `Lua name` when `raw` is empty, does not start with
    /// an ASCII letter or underscore, contains anything other than ASCII letters, digits and
    /// underscores, or is one of Lua's reserved words.
    ///
    /// # Examples
    ///
    /// ```
    /// use airsl::LuaName;
    ///
    /// assert_eq!(LuaName::new("hookEventName")?.as_str(), "hookEventName");
    /// assert!(LuaName::new("end").is_err());
    /// # Ok::<(), airsl::Error>(())
    /// ```
    pub fn new(raw: &str) -> Result<Self> {
        let invalid = |reason: &'static str| Error::InvalidName {
            kind: "Lua name",
            value: raw.to_owned(),
            reason,
        };

        let mut chars = raw.chars();
        let Some(first) = chars.next() else {
            return Err(invalid("must not be empty"));
        };
        if !(first.is_ascii_alphabetic() || first == '_') {
            return Err(invalid("must start with an ASCII letter or underscore"));
        }
        if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(invalid(
                "must contain only ASCII letters, digits and underscores",
            ));
        }
        if RESERVED_WORDS.contains(&raw) {
            return Err(invalid("must not be a Lua reserved word"));
        }
        Ok(Self(raw.to_owned()))
    }

    /// The name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for LuaName {
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

    use super::{LuaName, RESERVED_WORDS};

    #[test]
    fn accepts_identifiers_with_mixed_case_and_underscores() {
        for name in ["End", "_x", "hookEventName"] {
            assert_eq!(LuaName::new(name).unwrap().as_str(), name);
        }
    }

    #[test]
    fn refuses_an_empty_name() {
        let err = LuaName::new("").unwrap_err();
        assert_eq!(err.to_string(), "invalid Lua name ``: must not be empty");
    }

    #[test]
    fn refuses_a_leading_digit() {
        let err = LuaName::new("9x").unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid Lua name `9x`: must start with an ASCII letter or underscore"
        );
    }

    #[test]
    fn refuses_a_hyphen() {
        let err = LuaName::new("a-b").unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid Lua name `a-b`: must contain only ASCII letters, digits and underscores"
        );
    }

    #[test]
    fn refuses_every_reserved_word() {
        // Spelled out here rather than read from `RESERVED_WORDS`, so a mistyped entry in the
        // shared list fails this test instead of agreeing with itself.
        let words = [
            "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto",
            "if", "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until",
            "while",
        ];
        let mut shared = RESERVED_WORDS.to_vec();
        shared.sort_unstable();
        assert_eq!(shared, words);
        for word in words {
            let err = LuaName::new(word).unwrap_err();
            assert_eq!(
                err.to_string(),
                format!("invalid Lua name `{word}`: must not be a Lua reserved word")
            );
        }
    }

    #[test]
    fn accepts_a_capitalised_reserved_word() {
        for name in ["End", "Nil"] {
            assert!(LuaName::new(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn a_lua_name_renders_as_its_text() {
        assert_eq!(LuaName::new("run").unwrap().to_string(), "run");
    }
}
