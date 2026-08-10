//! Turning a path a script wrote into one a grant can be checked against.
//!
//! Its own module because this is the single place the filesystem containment rule is written, and
//! it is the piece most worth reading twice: every `fs` function funnels through it, so a mistake
//! here is not one bug but the whole boundary. Keeping it apart from the module that opens files
//! also means it can be tested against paths that do not exist, which is most of the interesting
//! cases.
//!
//! Responsibilities: [`PathGuard`], which resolves a path and answers whether the policy permits
//! reading or writing it.
//!
//! Non-responsibilities: performing the operation, and deciding which of read or write an
//! operation needs. Only the calling function knows that.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::error::{Error, Result};
use crate::sandbox::GrantSet;

/// Checks paths against the filesystem grants of one policy.
///
/// Cheap to clone: the grants sit behind an [`Arc`] so every host function installed by a module
/// can hold one without copying the allowlists.
#[derive(Debug, Clone)]
pub(crate) struct PathGuard {
    grants: Arc<GrantSet>,
    module: &'static str,
}

impl PathGuard {
    /// A guard enforcing `grants`, reporting refusals against `module`.
    pub(crate) const fn new(grants: Arc<GrantSet>, module: &'static str) -> Self {
        Self { grants, module }
    }

    /// Resolves `raw` and returns it if the policy permits reading it.
    ///
    /// # Errors
    ///
    /// [`Error::Denied`] when no read grant covers the resolved path, or [`Error::UncheckablePath`]
    /// when it cannot be resolved to something checkable.
    pub(crate) fn read(&self, operation: &'static str, raw: &str) -> Result<PathBuf> {
        let resolved = Self::resolve(raw)?;
        if self.grants.is_unrestricted() || self.grants.fs().allows_read(&resolved) {
            return Ok(resolved);
        }
        Err(self.deny(operation, "read", &resolved))
    }

    /// Resolves `raw` and returns it if the policy permits writing it.
    ///
    /// # Errors
    ///
    /// As [`PathGuard::read`], against the write roots.
    pub(crate) fn write(&self, operation: &'static str, raw: &str) -> Result<PathBuf> {
        let resolved = Self::resolve(raw)?;
        if self.grants.is_unrestricted() || self.grants.fs().allows_write(&resolved) {
            return Ok(resolved);
        }
        Err(self.deny(operation, "write", &resolved))
    }

    /// Builds the refusal, naming the roots that *were* granted.
    ///
    /// Listing them turns "denied" into something actionable — the usual cause is a grant one
    /// directory too deep, and without the list that is invisible from the message.
    fn deny(&self, operation: &'static str, direction: &str, resolved: &Path) -> Error {
        let roots = if direction == "read" {
            self.grants.fs().read_roots()
        } else {
            self.grants.fs().write_roots()
        };

        let granted = if roots.is_empty() {
            format!("no {direction} roots are granted")
        } else {
            let names: Vec<_> = roots.iter().map(|r| r.display().to_string()).collect();
            format!("granted {direction} roots are {}", names.join(", "))
        };

        Error::Denied {
            module: self.module,
            operation,
            detail: format!("`{}` is outside them — {granted}", resolved.display()),
        }
    }

    /// Resolves `raw` to the absolute, symlink-free path an operation would actually touch.
    ///
    /// The rule that makes this sound: canonicalise the deepest part of the path that **exists**,
    /// and accept only ordinary names below it. Canonicalising is what resolves symlinks, so a
    /// link inside a granted root pointing outside it is caught. What remains below cannot contain
    /// a symlink, because it does not exist.
    ///
    /// A `..` below that point is refused rather than resolved. Resolving it lexically is the
    /// classic way a containment check turns out not to contain anything: with `/granted/link`
    /// pointing at `/elsewhere`, the path `/granted/link/../secret` reads lexically as
    /// `/granted/secret` — inside the root — while the operating system opens `/secret`. The two
    /// disagree, and the lexical answer is the wrong one.
    ///
    /// # Errors
    ///
    /// [`Error::UncheckablePath`] when the path cannot be made absolute or ends in an unresolvable
    /// `..`.
    fn resolve(raw: &str) -> Result<PathBuf> {
        let absolute = std::path::absolute(raw).map_err(|_| Error::UncheckablePath {
            path: raw.to_owned(),
            reason: "the working directory could not be read",
        })?;

        let mut suffix: Vec<std::ffi::OsString> = Vec::new();
        let mut probe = absolute;

        loop {
            if let Ok(canonical) = probe.canonicalize() {
                let mut resolved = canonical;
                for name in suffix.iter().rev() {
                    resolved.push(name);
                }
                return Ok(resolved);
            }

            match probe.components().next_back() {
                // An ordinary name that does not exist yet: remember it and look higher up.
                Some(Component::Normal(name)) => suffix.push(name.to_owned()),
                // `..` below the deepest existing directory has no filesystem meaning, and
                // inventing one lexically is exactly the bypass this function exists to prevent.
                Some(Component::ParentDir) => {
                    return Err(Error::UncheckablePath {
                        path: raw.to_owned(),
                        reason: "it climbs through a directory that does not exist",
                    });
                }
                Some(Component::CurDir) => {}
                // Reached the root without finding anything that exists.
                _ => {
                    return Err(Error::UncheckablePath {
                        path: raw.to_owned(),
                        reason: "no part of it exists",
                    });
                }
            }

            if !probe.pop() {
                return Err(Error::UncheckablePath {
                    path: raw.to_owned(),
                    reason: "no part of it exists",
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::PathGuard;
    use crate::sandbox::GrantSet;
    use std::sync::Arc;

    fn guard(build: impl FnOnce(GrantSet) -> GrantSet) -> PathGuard {
        PathGuard::new(Arc::new(build(GrantSet::declared())), "fs")
    }

    #[test]
    fn a_file_inside_a_read_root_is_permitted() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "x").unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.read(&root)));
        assert!(
            guard
                .read("read", root.join("a.txt").to_str().unwrap())
                .is_ok()
        );
    }

    #[test]
    fn a_file_outside_every_read_root_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret"), "x").unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.read(dir.path())));
        let err = guard
            .read("read", outside.path().join("secret").to_str().unwrap())
            .unwrap_err();
        assert!(err.to_string().contains("fs.read denied"), "{err}");
    }

    #[test]
    fn a_symlink_inside_the_root_pointing_out_of_it_is_refused() {
        // The whole reason resolution canonicalises rather than normalising: the path is spelled
        // entirely inside the granted root and still reaches outside it.
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret"), "leaked").unwrap();

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret"), root.join("link")).unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.read(&root)));
        let err = guard
            .read("read", root.join("link").to_str().unwrap())
            .unwrap_err();
        assert!(err.to_string().contains("denied"), "{err}");
    }

    #[test]
    fn a_dotdot_through_a_symlink_does_not_escape() {
        // `<root>/link/../secret` reads lexically as `<root>/secret`, which is inside the root.
        // The operating system would open `<outside>/secret`, which is not.
        let outside = tempfile::tempdir().unwrap();
        let outside_root = outside.path().canonicalize().unwrap();
        std::fs::create_dir(outside_root.join("sub")).unwrap();
        std::fs::write(outside_root.join("secret"), "leaked").unwrap();

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::os::unix::fs::symlink(outside_root.join("sub"), root.join("link")).unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.read(&root)));
        let attack = format!("{}/link/../secret", root.display());
        assert!(
            guard.read("read", &attack).is_err(),
            "{attack} was permitted"
        );
    }

    #[test]
    fn a_path_that_does_not_exist_yet_is_checked_against_its_parent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.write(&root)));
        // Writing a new file, and creating directories that do not exist yet, both have to work.
        assert!(
            guard
                .write("write", root.join("new.txt").to_str().unwrap())
                .is_ok()
        );
        assert!(
            guard
                .write("mkdir", root.join("a/b/c").to_str().unwrap())
                .is_ok()
        );
    }

    #[test]
    fn a_dotdot_below_a_directory_that_does_not_exist_is_refused_rather_than_guessed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.write(&root)));
        let err = guard
            .write("write", &format!("{}/absent/../ok.txt", root.display()))
            .unwrap_err();
        assert!(err.to_string().contains("cannot resolve"), "{err}");
    }

    #[test]
    fn read_and_write_are_checked_against_their_own_roots() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "x").unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.read(&root)));
        let target = root.join("a.txt");
        assert!(guard.read("read", target.to_str().unwrap()).is_ok());
        let err = guard.write("write", target.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("fs.write denied"), "{err}");
    }

    #[test]
    fn an_unrestricted_policy_checks_nothing() {
        let guard = PathGuard::new(Arc::new(GrantSet::unrestricted()), "fs");
        assert!(guard.read("read", "/etc/hostname").is_ok());
    }

    #[test]
    fn a_refusal_names_the_roots_that_were_granted() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "x").unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.read(&root)));
        let err = guard.read("read", "/etc/hostname").unwrap_err();
        assert!(
            err.to_string().contains(&root.display().to_string()),
            "the refusal should say what was granted: {err}"
        );
    }

    #[test]
    fn a_refusal_with_no_roots_at_all_says_so() {
        let guard = guard(|g| g);
        let err = guard.read("read", "/etc/hostname").unwrap_err();
        assert!(
            err.to_string().contains("no read roots are granted"),
            "{err}"
        );
    }
}
