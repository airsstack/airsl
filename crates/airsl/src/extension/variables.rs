//! The `$VAR` values a host supplies for paths in a manifest.
//!
//! Its own module because *who* expands a variable is a security property, not a
//! convenience: if an extension could expand `$APP_HOME` from the process environment it could
//! set that variable and widen its own grant. Expansion therefore reads only this map, which
//! the host fills from its own configuration, and never `std::env`.
//!
//! Responsibilities: [`Variables`] and [`Variables::expand`].
//!
//! Non-responsibilities: deciding whether the expanded path is acceptable. That is the
//! manifest validator's job, and then the negotiator's.

use std::collections::BTreeMap;

use crate::error::{Error, Result};

/// Host-supplied variable values, looked up by bare name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Variables {
    values: BTreeMap<String, String>,
}

impl Variables {
    /// No variables: every `$VAR` in a manifest is then an error.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    /// Builds a map from `(name, value)` pairs; later pairs override earlier ones.
    pub fn from_pairs<I, K, V>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self {
            values: pairs
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }

    /// Adds or replaces one variable.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(name.into(), value.into());
        self
    }

    /// The value of `name`, if supplied.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// Expands every `$NAME` and `${NAME}` in `text`; `$$` is a literal `$`.
    ///
    /// A name is one or more ASCII letters, digits or underscores, and the braced form validates
    /// `NAME` against that same charset. A `$` followed by anything else — including a brace
    /// whose contents are empty or fall outside the charset, such as `${}` or `${a-b}` — is left
    /// as written, not looked up.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManifestVariable`] for a name this map does not hold.
    pub fn expand(&self, text: &str) -> Result<String> {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;

        while let Some(index) = rest.find('$') {
            out.push_str(&rest[..index]);
            let after = &rest[index + 1..];

            if let Some(tail) = after.strip_prefix('$') {
                out.push('$');
                rest = tail;
                continue;
            }

            let (name, tail) = if let Some(inner) = after.strip_prefix('{') {
                let Some(close) = inner.find('}') else {
                    out.push('$');
                    rest = after;
                    continue;
                };
                let candidate = &inner[..close];
                if candidate.is_empty() || !candidate.chars().all(is_name_char) {
                    out.push('$');
                    rest = after;
                    continue;
                }
                (candidate, &inner[close + 1..])
            } else {
                let end = after.find(|c| !is_name_char(c)).unwrap_or(after.len());
                (&after[..end], &after[end..])
            };

            if name.is_empty() {
                out.push('$');
                rest = after;
                continue;
            }

            let value = self.get(name).ok_or_else(|| Error::ManifestVariable {
                name: name.to_owned(),
            })?;
            out.push_str(value);
            rest = tail;
        }

        out.push_str(rest);
        Ok(out)
    }
}

/// Whether `c` may appear in a `$NAME` or `${NAME}` variable name.
const fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Variables;

    fn vars() -> Variables {
        Variables::none()
            .with("APP_HOME", "/home/x/app")
            .with("N", "7")
    }

    #[test]
    fn expands_bare_and_braced_names() {
        assert_eq!(
            vars().expand("$APP_HOME/journal").unwrap(),
            "/home/x/app/journal"
        );
        assert_eq!(vars().expand("${APP_HOME}/x").unwrap(), "/home/x/app/x");
        assert_eq!(vars().expand("v${N}x$N").unwrap(), "v7x7");
    }

    #[test]
    fn a_doubled_dollar_is_a_literal() {
        assert_eq!(vars().expand("cost: $$5").unwrap(), "cost: $5");
    }

    #[test]
    fn a_dollar_that_starts_no_name_is_left_alone() {
        assert_eq!(vars().expand("a$ b$").unwrap(), "a$ b$");
        assert_eq!(vars().expand("${unterminated").unwrap(), "${unterminated");
    }

    #[test]
    fn an_unknown_name_is_an_error_naming_it() {
        let err = vars().expand("$NOPE/x").unwrap_err();
        assert!(err.to_string().contains("`$NOPE`"), "{err}");
    }

    #[test]
    fn the_process_environment_is_never_consulted() {
        // HOME is set in every test environment; it must still be unknown here.
        assert!(Variables::none().expand("$HOME").is_err());
    }

    #[test]
    fn later_pairs_override_earlier_ones() {
        let v = Variables::from_pairs([("A", "1"), ("A", "2")]);
        assert_eq!(v.get("A"), Some("2"));
    }

    #[test]
    fn a_trailing_dollar_with_nothing_after_it_is_left_alone() {
        assert_eq!(vars().expand("x$").unwrap(), "x$");
    }

    #[test]
    fn empty_braces_are_left_alone_rather_than_treated_as_a_name() {
        assert_eq!(vars().expand("${}rest").unwrap(), "${}rest");
    }

    #[test]
    fn a_braced_name_outside_the_charset_is_left_alone_rather_than_looked_up() {
        assert_eq!(vars().expand("${a-b}rest").unwrap(), "${a-b}rest");
        assert_eq!(vars().expand("${APP HOME}rest").unwrap(), "${APP HOME}rest");
    }

    #[test]
    fn adjacent_variables_with_no_separator_both_expand() {
        let v = Variables::from_pairs([("A", "1"), ("B", "2")]);
        assert_eq!(v.expand("$A$B").unwrap(), "12");
        assert_eq!(v.expand("${A}${B}").unwrap(), "12");
    }

    #[test]
    fn multibyte_text_around_a_variable_expands_without_panicking() {
        assert_eq!(
            vars().expand("héllo $APP_HOME 世界").unwrap(),
            "héllo /home/x/app 世界"
        );
    }
}
