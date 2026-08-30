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

use std::path::Component;
use std::sync::Arc;

use crate::error::{Error, Result};
use crate::paths::ResolvedPath;
use crate::paths::rules::Unrepresentable;
use crate::paths::rules::native;
use crate::sandbox::GrantSet;

/// Which set of roots a refusal was measured against.
///
/// A type rather than the `&str` it replaces because the refusal message and the choice of
/// allowlist are driven by the same value: spelling it once as data means a message can never
/// name one direction while the check consulted the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    Read,
    Write,
}

impl Access {
    /// The word this access reads as in a refusal.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

impl core::fmt::Display for Access {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

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
    pub(crate) fn read(&self, operation: &'static str, raw: &str) -> Result<ResolvedPath> {
        let resolved = Self::resolve(raw)?;
        if self.grants.is_unrestricted() || self.grants.fs().allows_read(resolved.as_path()) {
            return Ok(resolved);
        }
        Err(self.deny(operation, Access::Read, &resolved))
    }

    /// Resolves `raw` and returns it if the policy permits writing it.
    ///
    /// # Errors
    ///
    /// As [`PathGuard::read`], against the write roots.
    pub(crate) fn write(&self, operation: &'static str, raw: &str) -> Result<ResolvedPath> {
        let resolved = Self::resolve(raw)?;
        if self.grants.is_unrestricted() || self.grants.fs().allows_write(resolved.as_path()) {
            return Ok(resolved);
        }
        Err(self.deny(operation, Access::Write, &resolved))
    }

    /// Builds the refusal, naming the roots that *were* granted.
    ///
    /// Listing them turns "denied" into something actionable — the usual cause is a grant one
    /// directory too deep, and without the list that is invisible from the message.
    ///
    /// Both branches share the phrase "is outside the granted read roots", so the sentence names
    /// what it is comparing against before it says anything about the comparison. An earlier
    /// wording opened with "is outside them", which reads as a continuation of a clause that is
    /// not there: the roots were introduced *after* the pronoun that referred to them, and when
    /// none were granted they were never introduced at all.
    fn deny(&self, operation: &'static str, access: Access, resolved: &ResolvedPath) -> Error {
        let roots = match access {
            Access::Read => self.grants.fs().read_roots(),
            Access::Write => self.grants.fs().write_roots(),
        };

        let granted = if roots.is_empty() {
            String::from("none are granted")
        } else {
            let names: Vec<_> = roots.iter().map(|r| native::to_script_string(r)).collect();
            names.join(", ")
        };

        Error::Denied {
            module: self.module,
            operation,
            detail: format!(
                "`{}` is outside the granted {access} roots: {granted}",
                resolved.to_script_string()
            ),
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
    /// The [`Component::ParentDir`] arm below is reachable on unix only. On Windows,
    /// `std::path::absolute` is `GetFullPathNameW`, which collapses `..` out of the string lexically
    /// before this function ever sees it, so a `..` component below the deepest existing ancestor
    /// simply cannot occur there. This looks like the same unsound lexical collapse the paragraph
    /// above warns against, but it is not: Win32 normalises `..` out of the path string *before* the
    /// object manager resolves what remains, so the check performed here and the open the operating
    /// system later performs are working from the same already-collapsed string and necessarily
    /// agree. Soundness holds on both platforms, by different arguments — this is documented and
    /// pinned by tests rather than left to be rediscovered.
    ///
    /// Two more checks happen before any of this: [`native::reject_unrepresentable`] refuses a
    /// spelling this runtime will not reason about at all — a verbatim (`\\?\`) prefix, a device
    /// namespace, a reserved device name — and the verbatim prefix `canonicalize()` adds on Windows
    /// is stripped from its result before the suffix is re-appended, so nothing verbatim is ever
    /// stored in the [`ResolvedPath`] this function returns.
    ///
    /// # Errors
    ///
    /// [`Error::UncheckablePath`] when the path is a spelling this runtime cannot reason about, when
    /// it cannot be made absolute, or when it ends in an unresolvable `..`.
    fn resolve(raw: &str) -> Result<ResolvedPath> {
        native::reject_unrepresentable(raw).map_err(|unrepresentable| Error::UncheckablePath {
            path: raw.to_owned(),
            reason: match unrepresentable {
                Unrepresentable::Verbatim => {
                    r"it is a verbatim \\?\ path, which this runtime refuses as input"
                }
                Unrepresentable::DeviceNamespace => {
                    "it names a reserved device rather than an ordinary file"
                }
                Unrepresentable::InteriorNul => "it contains a nul byte",
            },
        })?;

        let absolute = std::path::absolute(raw).map_err(|_| Error::UncheckablePath {
            path: raw.to_owned(),
            reason: "the working directory could not be read",
        })?;

        let mut suffix: Vec<std::ffi::OsString> = Vec::new();
        let mut probe = absolute;

        loop {
            if let Ok(canonical) = probe.canonicalize() {
                let mut resolved = native::strip_verbatim(canonical);
                for name in suffix.iter().rev() {
                    resolved.push(name);
                }
                return Ok(ResolvedPath::new(resolved));
            }

            match probe.components().next_back() {
                // An ordinary name that does not exist yet: remember it and look higher up.
                Some(Component::Normal(name)) => suffix.push(name.to_owned()),
                // `..` below the deepest existing directory has no filesystem meaning, and
                // inventing one lexically is exactly the bypass this function exists to prevent.
                // Unix-reachable only — see the doc comment above.
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
    #![expect(
        clippy::panic,
        reason = "a wrong Error variant is the intended failure signal"
    )]

    use super::PathGuard;
    use crate::paths::rules::native;
    use crate::sandbox::GrantSet;
    use crate::test_support::script_path;
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
                .read("read", &format!("{}/a.txt", script_path(&root)))
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
        crate::test_support::link_file(&outside.path().join("secret"), &root.join("link")).unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.read(&root)));
        let err = guard
            .read("read", &format!("{}/link", script_path(&root)))
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
        crate::test_support::link_dir(&outside_root.join("sub"), &root.join("link")).unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.read(&root)));
        // `script_path`, not `root.display()`: on Windows, `root` came from `canonicalize()` and
        // is `\\?\`-prefixed, which the door check in `resolve` refuses before this test's `..`
        // ever gets a chance to matter.
        let attack = format!("{}/link/../secret", script_path(&root));
        let result = guard.read("read", &attack);

        // Unix resolves `link` before it sees the `..`, so the escape is caught. Windows collapses
        // `..` out of the string lexically before the guard ever sees `link` — see the doc comment
        // on `resolve` — so the same raw string never reaches through the symlink there at all;
        // both platforms are sound, by different arguments.
        #[cfg(unix)]
        assert!(result.is_err(), "{attack} was permitted");
        #[cfg(windows)]
        assert!(
            result.is_ok(),
            "{attack} should resolve inside the root on Windows: {result:?}"
        );
    }

    #[test]
    fn a_path_that_does_not_exist_yet_is_checked_against_its_parent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.write(&root)));
        let root = script_path(&root);
        // Writing a new file, and creating directories that do not exist yet, both have to work.
        assert!(guard.write("write", &format!("{root}/new.txt")).is_ok());
        assert!(guard.write("mkdir", &format!("{root}/a/b/c")).is_ok());
    }

    #[test]
    fn a_dotdot_below_a_directory_that_does_not_exist_is_refused_rather_than_guessed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.write(&root)));
        // `script_path`, not `root.display()`: see the comment on `script_path` for why.
        let result = guard.write("write", &format!("{}/absent/../ok.txt", script_path(&root)));

        // Unix reaches the `ParentDir` arm, because `absent` never gets created and `..` is never
        // collapsed lexically. Windows collapses `..` out of the string before the guard ever sees
        // it — see the doc comment on `resolve` — so this reaches the ordinary "does not exist yet"
        // path against `root` itself, which is granted, rather than the unreachable `ParentDir`
        // arm.
        #[cfg(unix)]
        {
            let err = result.unwrap_err();
            assert!(err.to_string().contains("cannot resolve"), "{err}");
        }
        #[cfg(windows)]
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn read_and_write_are_checked_against_their_own_roots() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "x").unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.read(&root)));
        let target = format!("{}/a.txt", script_path(&root));
        assert!(guard.read("read", &target).is_ok());
        let err = guard.write("write", &target).unwrap_err();
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
        // `native::to_script_string`, not `root.display()`: the refusal renders the granted
        // roots in the `/`-spelled script vocabulary (`deny`, above), so the expectation has to
        // be built the same way or the two disagree on Windows.
        assert!(
            err.to_string().contains(&native::to_script_string(&root)),
            "the refusal should say what was granted: {err}"
        );
        assert!(
            err.to_string().contains("outside the granted read roots"),
            "the refusal should name what it compared against: {err}"
        );
    }

    #[test]
    fn a_refusal_renders_the_granted_roots_without_backslashes() {
        // The offending path in a refusal renders through `to_script_string` (`resolved`,
        // above), and so does the granted-roots list beside it — both sides of the colon use the
        // same `/`-spelled vocabulary, so a Windows message never mixes separators. On unix
        // `display()` and `to_script_string` already agree, so this assertion cannot distinguish
        // the two renderings on this host; it is the mixed-separator case on Windows that this
        // pins.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "x").unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.read(&root)));
        let err = guard.read("read", "/etc/hostname").unwrap_err();
        assert!(!err.to_string().contains('\\'), "{err}");
    }

    #[test]
    fn a_refusal_measured_against_the_write_roots_says_write() {
        // The message and the allowlist come from one value, so a refusal cannot report a
        // direction the check did not use.
        let guard = guard(|g| g);
        let err = guard.write("write", "/etc/hostname").unwrap_err();
        assert!(
            err.to_string().contains("outside the granted write roots"),
            "{err}"
        );
    }

    #[test]
    fn a_refusal_with_no_roots_at_all_says_so() {
        let guard = guard(|g| g);
        let err = guard.read("read", "/etc/hostname").unwrap_err();
        assert!(
            err.to_string()
                .contains("outside the granted read roots: none are granted"),
            "{err}"
        );
    }

    #[test]
    fn a_path_containing_a_nul_byte_is_refused_before_any_resolution_is_attempted() {
        // Caught by `reject_unrepresentable`, not left to fall through to `canonicalize`'s own
        // failure and the generic "no part of it exists" that would otherwise misreport why.
        let err = PathGuard::resolve("a\0b").unwrap_err();
        match err {
            crate::error::Error::UncheckablePath { reason, .. } => {
                assert_eq!(reason, "it contains a nul byte");
            }
            other => panic!("expected UncheckablePath, got {other:?}"),
        }
    }

    // The rest of this module is exercised on every host this crate builds for. These two are
    // `#[cfg(windows)]` because the spellings they refuse — a verbatim prefix, a reserved device
    // name — are ordinary, meaningful strings on unix (a verbatim prefix is just an unusual
    // filename; `CON` is a real one) and refusing them there would regress unix behaviour rather
    // than protect anything. See `reject_unrepresentable`'s own doc comment.

    #[cfg(windows)]
    #[test]
    fn a_verbatim_path_with_a_hidden_dotdot_is_refused_rather_than_approved() {
        // This exact path is what makes the refusal necessary rather than tidy: `/` is not a
        // separator inside a verbatim spelling, so `a/../../Windows` parses as one
        // `Component::Normal` and the `ParentDir` arm never fires. Resolution would pop to the
        // root, canonicalise, re-append the suffix, and approve a path that leaves the root.
        // Refusing every verbatim spelling at the door closes the class rather than chasing this
        // one instance of it.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap(); // canonicalize() returns a verbatim path
        let attack = format!("{}\\a/../../Windows\\System32\\x", root.display());

        let err = PathGuard::resolve(&attack).unwrap_err();
        match err {
            crate::error::Error::UncheckablePath { reason, .. } => {
                assert_eq!(
                    reason,
                    r"it is a verbatim \\?\ path, which this runtime refuses as input"
                );
            }
            other => panic!("expected UncheckablePath, got {other:?}"),
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_reserved_device_name_is_refused_by_name_not_by_the_misleading_existence_reason() {
        let dir = tempfile::tempdir().unwrap();
        let attack = format!("{}\\CON", dir.path().display());

        let err = PathGuard::resolve(&attack).unwrap_err();
        match err {
            crate::error::Error::UncheckablePath { reason, .. } => {
                assert_eq!(
                    reason,
                    "it names a reserved device rather than an ordinary file"
                );
            }
            other => panic!("expected UncheckablePath, got {other:?}"),
        }
    }

    #[cfg(windows)]
    #[test]
    fn the_guards_verdict_for_a_dotdot_through_a_symlink_matches_what_windows_actually_opens() {
        // `GetFullPathNameW` collapses `..` out of `<root>/link/../secret` before this function,
        // or the operating system's own open, ever sees `link` — so both land on `<root>/secret`.
        // Unix disagrees (see `a_dotdot_through_a_symlink_does_not_escape`): there the same string
        // opens through the symlink to `elsewhere/secret`. Both are sound; they reach the
        // conclusion by different arguments. Proven here by actually opening the file
        // the guard resolved to, not merely by inspecting its verdict.
        let outside = tempfile::tempdir().unwrap();
        let outside_root = outside.path().canonicalize().unwrap();
        std::fs::write(outside_root.join("secret"), "leaked").unwrap();

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("secret"), "granted").unwrap();
        crate::test_support::link_dir(&outside_root, &root.join("link")).unwrap();

        let guard = guard(|g| g.with_fs(|fs| fs.read(&root)));
        // `script_path`, not `root.display()`: see the comment on `script_path` for why.
        let attack = format!("{}/link/../secret", script_path(&root));

        let resolved = guard.read("read", &attack).unwrap();
        let opened = std::fs::read_to_string(resolved.as_path()).unwrap();
        assert_eq!(
            opened, "granted",
            "the guard's verdict must match what the operating system actually opens"
        );
    }
}
