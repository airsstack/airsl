//! Validated name of the single global table every host module is installed under.
//!
//! A distinct type from [`ModuleName`] rather than a reuse of it, because it carries an invariant a
//! module name does not need: this name becomes a *global*, so it can collide with Lua's own. A
//! module named `os` is unremarkable — it lives at `airsstack.os` — while a root table named `os`
//! silently shadows the standard library, and one named `end` makes every script that uses it a
//! syntax error.
//!
//! Responsibilities: [`RootTable`], its constructor, and the names it refuses.
//!
//! Non-responsibilities: installing anything. [`crate::EngineBuilder`] reads the name; this type
//! only decides whether it is usable.

use crate::error::{Error, Result};
use crate::types::ModuleName;

/// The root table an engine uses unless told otherwise.
const DEFAULT: &str = "airsstack";

/// Lua 5.4's reserved words, which cannot appear where a name is expected.
///
/// Only the lowercase ones can reach here; [`ModuleName`] already requires a lowercase first
/// letter, so nothing else in the grammar is reachable.
const RESERVED_WORDS: [&str; 22] = [
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

/// Globals a root table would shadow: the standard library tables, and the base functions a script
/// is entitled to assume are there.
const SHADOWED_GLOBALS: [&str; 29] = [
    "arg",
    "assert",
    "collectgarbage",
    "coroutine",
    "debug",
    "dofile",
    "error",
    "getmetatable",
    "io",
    "ipairs",
    "load",
    "loadfile",
    "loadstring",
    "math",
    "next",
    "os",
    "package",
    "pairs",
    "pcall",
    "print",
    "rawequal",
    "rawget",
    "rawlen",
    "rawset",
    "require",
    "select",
    "setmetatable",
    "string",
    "table",
];

/// A usable name for the root table host modules are installed under.
///
/// Accepts what [`ModuleName`] accepts, minus Lua's reserved words and the globals it would
/// shadow.
///
/// # Examples
///
/// ```
/// use airsl::RootTable;
///
/// assert_eq!(RootTable::default().as_str(), "airsstack");
/// assert!(RootTable::new("myapp").is_ok());
/// assert!(RootTable::new("os").is_err());
/// # Ok::<(), airsl::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RootTable(ModuleName);

impl RootTable {
    /// Validates `raw` and wraps it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] when `raw` is not a well-formed name, is a Lua reserved
    /// word, or would shadow a standard global.
    pub fn new(raw: impl Into<String>) -> Result<Self> {
        let raw = raw.into();
        let name = ModuleName::new(raw).map_err(|error| match error {
            Error::InvalidName { value, reason, .. } => Error::InvalidName {
                kind: "root table name",
                value,
                reason,
            },
            other => other,
        })?;

        let invalid = |reason: &'static str| Error::InvalidName {
            kind: "root table name",
            value: name.as_str().to_owned(),
            reason,
        };

        if RESERVED_WORDS.contains(&name.as_str()) {
            return Err(invalid("must not be a Lua reserved word"));
        }
        if SHADOWED_GLOBALS.contains(&name.as_str()) {
            return Err(invalid("must not shadow a Lua standard global"));
        }
        Ok(Self(name))
    }

    /// The name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Default for RootTable {
    fn default() -> Self {
        Self::new(DEFAULT)
            .unwrap_or_else(|_| unreachable!("`airsstack` is a valid root table name"))
    }
}

impl core::fmt::Display for RootTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{DEFAULT, RootTable};

    #[test]
    fn the_default_root_table_is_the_workspace_name() {
        assert_eq!(RootTable::default().as_str(), DEFAULT);
    }

    #[test]
    fn an_ordinary_name_is_accepted() {
        for name in ["myapp", "redis_tools", "x1"] {
            assert!(RootTable::new(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn a_reserved_word_is_rejected_because_it_would_not_parse() {
        for name in ["end", "local", "function", "return", "do", "then"] {
            let err = RootTable::new(name).unwrap_err();
            assert!(err.to_string().contains("reserved word"), "{name}: {err}");
        }
    }

    #[test]
    fn a_standard_library_table_is_rejected_because_it_would_shadow() {
        for name in [
            "os",
            "string",
            "table",
            "math",
            "io",
            "coroutine",
            "package",
        ] {
            let err = RootTable::new(name).unwrap_err();
            assert!(err.to_string().contains("shadow"), "{name}: {err}");
        }
    }

    #[test]
    fn a_base_global_a_script_relies_on_is_rejected() {
        for name in ["print", "pcall", "require", "arg", "setmetatable"] {
            let err = RootTable::new(name).unwrap_err();
            assert!(err.to_string().contains("shadow"), "{name}: {err}");
        }
    }

    #[test]
    fn a_malformed_name_is_rejected_and_reported_as_a_root_table_name() {
        let err = RootTable::new("Not-A-Name").unwrap_err();
        assert!(err.to_string().contains("root table name"), "{err}");
    }

    #[test]
    fn an_empty_name_is_rejected() {
        assert!(RootTable::new("").is_err());
    }

    #[test]
    fn a_root_table_renders_as_its_name() {
        assert_eq!(RootTable::new("myapp").unwrap().to_string(), "myapp");
    }
}
