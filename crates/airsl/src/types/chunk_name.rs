//! Validated name a Lua chunk is compiled under, used in error messages and tracebacks.
//!
//! Its own type because Lua treats the leading byte of a chunk name as a format marker: `@` means
//! "a file path", `=` means "use this text verbatim", and anything else is treated as source code
//! to quote back. Handing a raw string to the VM therefore changes how every traceback reads
//! depending on what the string happens to start with. [`ChunkName`] pins the choice at
//! construction.
//!
//! Responsibilities: [`ChunkName`], its constructors, and the Lua-facing rendering.
//!
//! Non-responsibilities: reading the script. [`crate::Script`] owns the source text.

use crate::error::{Error, Result};

/// The name a chunk is compiled under.
///
/// Rendered for Lua by [`ChunkName::as_lua`], which prefixes `@` so tracebacks show the value as a
/// source location rather than quoting it as a code fragment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkName(String);

impl ChunkName {
    /// Longest accepted name, in bytes. Lua truncates long chunk names in tracebacks; keeping
    /// under the limit means the whole name survives into the message.
    const MAX_LEN: usize = 240;

    /// Validates `raw` and wraps it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] when `raw` is empty, longer than 240 bytes, or contains a
    /// newline or NUL — either of which would corrupt the traceback the name appears in.
    pub fn new(raw: impl Into<String>) -> Result<Self> {
        let raw = raw.into();
        let invalid = |reason: &'static str| Error::InvalidName {
            kind: "chunk name",
            value: raw.clone(),
            reason,
        };

        if raw.is_empty() {
            return Err(invalid("must not be empty"));
        }
        if raw.len() > Self::MAX_LEN {
            return Err(invalid("must be at most 240 bytes"));
        }
        if raw.contains(['\n', '\r', '\0']) {
            return Err(invalid("must not contain newlines or NUL"));
        }
        Ok(Self(raw))
    }

    /// Builds a chunk name from a filesystem path, repairing it rather than refusing it.
    ///
    /// Unlike [`ChunkName::new`] this never fails, so a script can always be compiled under *some*
    /// name. A path too long for the limit keeps its tail — the filename and the directories
    /// nearest it are what identifies a script in a traceback, and collapsing the whole thing to a
    /// placeholder discarded exactly the informative part.
    #[must_use]
    pub fn from_path(path: &std::path::Path) -> Self {
        let sanitised = path
            .display()
            .to_string()
            .replace(['\n', '\r', '\0'], "\u{fffd}");
        Self::new(elide(&sanitised)).unwrap_or_else(|_| Self(String::from("?")))
    }

    /// The name as written, without the Lua format marker.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The name as Lua should receive it: prefixed with `@` so it is reported as a source location.
    #[must_use]
    pub fn as_lua(&self) -> String {
        format!("@{}", self.0)
    }
}

/// Trims an over-long name to its last [`ChunkName::MAX_LEN`] bytes, marking what was dropped.
///
/// Splits on a character boundary, so a multi-byte path component is never cut in half — the
/// result has to survive into a Lua traceback as valid UTF-8.
fn elide(text: &str) -> String {
    const MARK: &str = "…";

    if text.len() <= ChunkName::MAX_LEN {
        return text.to_owned();
    }
    let budget = ChunkName::MAX_LEN - MARK.len();
    let start = text
        .char_indices()
        .map(|(index, _)| index)
        .find(|&index| text.len() - index <= budget)
        .unwrap_or(text.len());
    format!("{MARK}{}", &text[start..])
}

impl core::fmt::Display for ChunkName {
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

    use super::ChunkName;
    use std::path::Path;

    #[test]
    fn keeps_the_name_verbatim_and_marks_it_as_a_path_for_lua() {
        let name = ChunkName::new("hooks/enforce.lua").unwrap();
        assert_eq!(name.as_str(), "hooks/enforce.lua");
        assert_eq!(name.as_lua(), "@hooks/enforce.lua");
    }

    #[test]
    fn rejects_empty_names() {
        assert!(ChunkName::new("").is_err());
    }

    #[test]
    fn rejects_names_that_would_corrupt_a_traceback() {
        for raw in ["a\nb", "a\rb", "a\0b"] {
            assert!(ChunkName::new(raw).is_err(), "{raw:?} should be rejected");
        }
    }

    #[test]
    fn rejects_names_longer_than_the_limit() {
        assert!(ChunkName::new("a".repeat(240)).is_ok());
        assert!(ChunkName::new("a".repeat(241)).is_err());
    }

    #[test]
    fn an_over_long_path_keeps_the_tail_that_identifies_it() {
        let path = format!("/{}/plugins/hooks/enforce.lua", "deep".repeat(90));
        let name = ChunkName::from_path(Path::new(&path));
        assert!(name.as_str().len() <= 240, "{}", name.as_str().len());
        assert!(
            name.as_str().ends_with("/plugins/hooks/enforce.lua"),
            "{}",
            name.as_str()
        );
        assert!(name.as_str().starts_with('…'), "{}", name.as_str());
    }

    #[test]
    fn an_over_long_path_is_never_cut_through_a_character() {
        // Every component is multi-byte, so a byte-wise split would produce invalid UTF-8 — which
        // has to reach a Lua traceback intact.
        let path = format!("/{}/ünïcøde.lua", "påth".repeat(90));
        let name = ChunkName::from_path(Path::new(&path));
        assert!(name.as_str().len() <= 240);
        assert!(name.as_str().ends_with("/ünïcøde.lua"), "{}", name.as_str());
    }

    #[test]
    fn a_path_within_the_limit_is_left_exactly_as_it_is() {
        let name = ChunkName::from_path(Path::new("/plugins/hooks/enforce.lua"));
        assert_eq!(name.as_str(), "/plugins/hooks/enforce.lua");
    }

    #[test]
    fn from_path_uses_the_path_text() {
        let name = ChunkName::from_path(Path::new("plugins/airsstack/hooks/enforce.lua"));
        assert_eq!(name.as_str(), "plugins/airsstack/hooks/enforce.lua");
    }

    #[test]
    fn from_path_falls_back_rather_than_failing() {
        let name = ChunkName::from_path(Path::new(""));
        assert_eq!(name.as_str(), "?");
    }
}
