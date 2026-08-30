//! The structural guarantee that a path has already been made safe to hand outward.
//!
//! Its own module because the guarantee is only worth anything if nothing can quietly bypass it.
//! A plain `PathBuf` carries no memory of whether it has been resolved; [`ResolvedPath`] makes that
//! fact part of the type, so a function that takes one cannot receive an unresolved path by
//! accident, and a caller cannot reach the native spelling without asking for it explicitly.
//!
//! Responsibilities: [`ResolvedPath`], its constructor, and its two accessors.
//!
//! Non-responsibilities: performing the resolution. Nothing here touches the filesystem or
//! symlinks; the type only carries the result. Nothing here compares against a granted root either
//! — that is [`crate::paths::containment`]'s job, applied by the caller after construction.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::{Path, PathBuf};

use crate::paths::rules::native;

/// A path that has been made absolute, non-verbatim, and symlink-resolved down to the deepest
/// existing ancestor, with no `..` remaining below that point.
///
/// That is the entire invariant — it is exactly what the constructor establishes and no more. In
/// particular it does **not** mean "inside a granted root": whether a resolved path lies under any
/// of a policy's roots is a separate question, asked afterwards with
/// [`crate::paths::containment::is_within`] or [`crate::paths::containment::contains_any`].
///
/// Deliberately has no `Display`, no `AsRef<str>`, no `AsRef<Path>`, and no `Deref`, and the inner
/// field is private. Any of those would let a path reach a log line, an error message, or Lua
/// through an implicit conversion, silently re-opening the native spelling this type exists to
/// close off — every call site that needs the string form has to name
/// [`ResolvedPath::to_script_string`] explicitly instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedPath(PathBuf);

impl ResolvedPath {
    /// Wraps a path a resolution algorithm has already established the invariant for.
    ///
    /// Not itself where the invariant is enforced — the caller (`PathGuard::resolve`, in this
    /// crate) is responsible for having done the work; this constructor only records that it was
    /// done.
    pub(crate) const fn new(path: PathBuf) -> Self {
        Self(path)
    }

    /// The path to hand to the operating system.
    pub(crate) fn as_path(&self) -> &Path {
        &self.0
    }

    /// The path to hand to a script: `/`-separated, never verbatim.
    pub(crate) fn to_script_string(&self) -> String {
        native::to_script_string(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::ResolvedPath;
    use std::path::{Path, PathBuf};

    #[test]
    fn as_path_returns_exactly_what_was_constructed_with() {
        let resolved = ResolvedPath::new(PathBuf::from("/a/b/c"));
        assert_eq!(resolved.as_path(), Path::new("/a/b/c"));
    }

    #[test]
    fn to_script_string_yields_forward_slashes_for_a_constructed_value() {
        let resolved = ResolvedPath::new(PathBuf::from("/a/b/c"));
        assert_eq!(resolved.to_script_string(), "/a/b/c");
    }
}
