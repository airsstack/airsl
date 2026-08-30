//! The one comparison that decides whether a path lies inside a granted root.
//!
//! Its own module because this predicate was written out separately at several call sites across
//! the crate before this module existed, each a chance to get it subtly wrong. A single home means
//! there is exactly one place to read, and exactly one place to fix, if the rule ever needs to
//! change.
//!
//! Responsibilities: [`is_within`] and [`contains_any`], the shared root-comparison predicate.
//!
//! Non-responsibilities: producing the paths being compared. Both arguments are expected to
//! already be absolute and symlink-resolved as far as they exist — see
//! [`crate::paths::resolved::ResolvedPath`] and [`crate::sandbox::grants::resolve_root`] — this
//! module only compares what it is given.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::{Path, PathBuf};

/// Whether `path` lies at or under `root`.
///
/// Compared component-wise via [`Path::starts_with`], never as strings and never case-folded.
///
/// Component-wise matters because a string-prefix test accepts `/repo-extra` as being inside
/// `/repo` — the classic way a containment check turns out to have never contained anything.
///
/// Never case-folding matters because NTFS is not unconditionally case-insensitive: per-directory
/// case sensitivity has existed since Windows 10 version 1803, and WSL turns it on. Folding the
/// comparison would treat two paths as the same root when the filesystem underneath might not, and
/// "might not" here means "an escape", not "a false negative" — folding can only ever make this
/// check *wider* than the directory it is guarding, never narrower. Where a path has already been
/// through `canonicalize()`, the operating system has already normalised its casing to what
/// actually exists; where it has not, a byte comparison is the narrower and therefore the safe
/// choice.
pub(crate) fn is_within(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

/// Whether `path` lies under any of `roots`.
pub(crate) fn contains_any(roots: &[PathBuf], path: &Path) -> bool {
    roots.iter().any(|root| is_within(path, root))
}

#[cfg(test)]
mod tests {
    use super::{contains_any, is_within};
    use std::path::Path;

    #[test]
    fn a_path_under_the_root_is_contained() {
        assert!(is_within(
            Path::new("/repo/src/main.rs"),
            Path::new("/repo")
        ));
        assert!(is_within(Path::new("/repo"), Path::new("/repo")));
    }

    #[test]
    fn a_path_outside_the_root_is_not_contained() {
        assert!(!is_within(Path::new("/elsewhere"), Path::new("/repo")));
        assert!(!is_within(Path::new("/"), Path::new("/repo")));
    }

    #[test]
    fn a_sibling_sharing_a_name_prefix_is_not_contained() {
        // The string test `"/repo-extra".starts_with("/repo")` is true; the component test is not.
        assert!(!is_within(Path::new("/repo-extra"), Path::new("/repo")));
        assert!(!is_within(Path::new("/repo-extra/src"), Path::new("/repo")));
    }

    #[test]
    fn contains_any_matches_if_any_root_contains_the_path() {
        let roots = [Path::new("/a").to_path_buf(), Path::new("/b").to_path_buf()];
        assert!(contains_any(&roots, Path::new("/b/x")));
        assert!(!contains_any(&roots, Path::new("/c/x")));
    }

    #[test]
    fn comparison_is_never_case_folded() {
        // Two spellings of a name that differ only in case are different roots as far as this
        // predicate is concerned; folding them would be wider than a case-sensitive directory
        // allows.
        assert!(!is_within(Path::new("/Repo/x"), Path::new("/repo")));
    }
}
