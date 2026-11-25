//! Turning a path a script wrote into one a grant can be checked against.
//!
//! Its own module because this is the single place the filesystem containment rule is written, and
//! it is the piece most worth reading twice: every `fs` function funnels through it, so a mistake
//! here is not one bug but the whole boundary. Keeping it apart from the module that opens files
//! also means it can be tested against paths that do not exist, which is most of the interesting
//! cases.
//!
//! Responsibilities: [`PathGuard`], which answers whether the policy permits reading or writing a
//! path. It refuses an unrepresentable spelling first, then an access direction with no roots
//! granted and a relative path without the `cwd` member (both naming the path as the script wrote
//! it), and only then resolves the path against the filesystem and compares it with the roots.
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
use crate::modules::access::{self, Subject};
use crate::paths::ResolvedPath;
use crate::paths::rules::Unrepresentable;
use crate::paths::rules::native;
use crate::sandbox::{GrantSet, HostAccess};

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
    /// Refuses in this order, and only the last step touches the filesystem: a spelling this
    /// runtime will not reason about, then a read axis with no roots at all, then a relative path
    /// when `cwd` is not granted, then resolution and the root comparison. In the two middle cases
    /// nothing could be allowed, so the refusal names `raw` as the script wrote it rather than a
    /// resolved path the script never asked about.
    ///
    /// # Errors
    ///
    /// [`Error::Denied`] when no read grant covers the path, including when none are granted at
    /// all or when the path is relative and `cwd` is not granted, or [`Error::UncheckablePath`]
    /// when it is an unrepresentable spelling or cannot be resolved to something checkable.
    pub(crate) fn read(&self, operation: &'static str, raw: &str) -> Result<ResolvedPath> {
        self.refuse_before_resolving(operation, Access::Read, raw)?;
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
        self.refuse_before_resolving(operation, Access::Write, raw)?;
        let resolved = Self::resolve(raw)?;
        if self.grants.is_unrestricted() || self.grants.fs().allows_write(resolved.as_path()) {
            return Ok(resolved);
        }
        Err(self.deny(operation, Access::Write, &resolved))
    }

    /// Runs every refusal that can be decided from the spelling of `raw` alone.
    ///
    /// The order is the contract: the spelling check comes first, so a nul byte or device name is
    /// reported as uncheckable whichever grants are held rather than as merely outside them; then
    /// the no-roots refusal; then the `cwd` refusal. An unrestricted set skips all three, as it is
    /// never refused by the root comparison either; [`PathGuard::resolve`] still starts with the
    /// spelling check for it.
    fn refuse_before_resolving(
        &self,
        operation: &'static str,
        access: Access,
        raw: &str,
    ) -> Result<()> {
        if self.grants.is_unrestricted() {
            return Ok(());
        }
        reject_lexically(raw)?;
        self.refuse_if_no_roots(operation, access, raw)?;
        self.refuse_relative_without_cwd(operation, raw)
    }

    /// Refuses `raw` outright when the policy grants no root for `access` at all.
    ///
    /// Refuses by the raw spelling before any resolution touches the filesystem. Resolving first
    /// would only let a refusal name a path the script did not write, and would let the existence
    /// of files outside every root show through the failure it chose.
    fn refuse_if_no_roots(&self, operation: &'static str, access: Access, raw: &str) -> Result<()> {
        if self.roots(access).is_empty() {
            return Err(self.refusal(operation, access, raw));
        }
        Ok(())
    }

    /// Refuses a relative `raw` when the policy does not grant `cwd`.
    ///
    /// A relative path means "relative to the working directory", so using one is reading the
    /// working directory. The refusal is on the spelling, before resolution consults the process
    /// directory, so what it says does not depend on where the host happens to run. An absolute
    /// path never needs the member, and an unrestricted set holds every member.
    fn refuse_relative_without_cwd(&self, operation: &'static str, raw: &str) -> Result<()> {
        if std::path::Path::new(raw).is_absolute() {
            return Ok(());
        }
        access::check_subject(
            &self.grants,
            self.module,
            operation,
            HostAccess::Cwd,
            Subject::RelativePath(raw),
        )
    }

    /// The granted roots for one direction of access.
    fn roots(&self, access: Access) -> &[std::path::PathBuf] {
        match access {
            Access::Read => self.grants.fs().read_roots(),
            Access::Write => self.grants.fs().write_roots(),
        }
    }

    /// Builds the refusal for a resolved path, naming the roots that *were* granted.
    fn deny(&self, operation: &'static str, access: Access, resolved: &ResolvedPath) -> Error {
        self.refusal(operation, access, &resolved.to_script_string())
    }

    /// Builds the refusal sentence for `shown`, naming the roots that *were* granted.
    ///
    /// Listing them turns "denied" into something actionable — the usual cause is a grant one
    /// directory too deep, and without the list that is invisible from the message. `shown` is the
    /// resolved path when a root comparison failed and the raw spelling when there was nothing to
    /// compare against; both go through here so the two sentences cannot drift.
    ///
    /// Both cases share the phrase "is outside the granted read roots", so the sentence names
    /// what it is comparing against before it says anything about the comparison. An earlier
    /// wording opened with "is outside them", which reads as a continuation of a clause that is
    /// not there: the roots were introduced *after* the pronoun that referred to them, and when
    /// none were granted they were never introduced at all.
    fn refusal(&self, operation: &'static str, access: Access, shown: &str) -> Error {
        let roots = self.roots(access);

        let granted = if roots.is_empty() {
            String::from("none are granted")
        } else {
            let names: Vec<_> = roots.iter().map(|r| native::to_script_string(r)).collect();
            names.join(", ")
        };

        Error::Denied {
            module: self.module,
            operation,
            detail: format!("`{shown}` is outside the granted {access} roots: {granted}"),
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
    /// [`PathGuard::read`] and [`PathGuard::write`] call this only after the spelling, no-roots and
    /// relative-path refusals, so a path no root could allow never reaches the filesystem. It
    /// still starts with the spelling check itself, which is what covers an unrestricted set.
    ///
    /// That check, [`reject_lexically`] (which runs [`native::reject_unrepresentable`]), refuses a
    /// spelling this runtime will not reason about at all: a verbatim (`\\?\`) prefix, a device
    /// namespace or a reserved device name. The verbatim prefix `canonicalize()` adds on Windows is
    /// stripped from its result before the suffix is re-appended, so nothing verbatim is ever
    /// stored in the [`ResolvedPath`] this function returns.
    ///
    /// # Errors
    ///
    /// [`Error::UncheckablePath`] when the path is a spelling this runtime cannot reason about, when
    /// it cannot be made absolute, or when it ends in an unresolvable `..`.
    fn resolve(raw: &str) -> Result<ResolvedPath> {
        reject_lexically(raw)?;

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

/// Refuses a spelling this runtime will not reason about, before any path is built from it.
///
/// A free function rather than a step buried in [`PathGuard::resolve`] because the guard runs it
/// ahead of the no-roots refusal too: a nul byte or device name is uncheckable whichever grants
/// are held, and must not be reported as merely outside them.
///
/// # Errors
///
/// [`Error::UncheckablePath`] naming which unrepresentable spelling `raw` is.
fn reject_lexically(raw: &str) -> Result<()> {
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
    })
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
    use crate::sandbox::{GrantSet, HostAccess};
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
    fn no_roots_refuses_before_touching_the_filesystem() {
        let target = tempfile::tempdir().unwrap();
        let links = tempfile::tempdir().unwrap();
        let link = links.path().join("link");
        crate::test_support::link_dir(target.path(), &link).unwrap();

        let guard = guard(|g| g);
        // Spelled as the script would, so the expectation is the raw string and not a resolved one.
        let raw = script_path(&link);
        let err = guard.read("read", &raw).unwrap_err();
        assert!(
            matches!(err, crate::error::Error::Denied { .. }),
            "expected Denied, got {err:?}"
        );
        assert!(
            err.to_string().contains(&format!(
                "`{raw}` is outside the granted read roots: none are granted"
            )),
            "{err}"
        );
        let target = script_path(&target.path().canonicalize().unwrap());
        assert!(
            !err.to_string().contains(&target),
            "the refusal must name the path as written, not the symlink target: {err}"
        );

        let err = guard.read("read", "/nonexistent/x").unwrap_err();
        assert!(
            matches!(err, crate::error::Error::Denied { .. }),
            "expected Denied, got {err:?}"
        );
        assert!(err.to_string().contains("`/nonexistent/x`"), "{err}");
    }

    #[test]
    fn no_roots_refuses_writes_with_the_write_wording() {
        let guard = guard(|g| g);
        let err = guard.write("write", "/some/where/x").unwrap_err();
        assert!(
            matches!(err, crate::error::Error::Denied { .. }),
            "expected Denied, got {err:?}"
        );
        assert!(
            err.to_string()
                .contains("`/some/where/x` is outside the granted write roots: none are granted"),
            "{err}"
        );
    }

    #[test]
    fn a_nul_path_is_still_uncheckable_before_either_rule() {
        let guard = guard(|g| g);
        for err in [
            guard.read("read", "a\0b").unwrap_err(),
            guard.write("write", "a\0b").unwrap_err(),
        ] {
            match err {
                crate::error::Error::UncheckablePath { reason, .. } => {
                    assert_eq!(reason, "it contains a nul byte");
                }
                other => panic!("expected UncheckablePath, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_nul_relative_path_is_uncheckable_even_with_roots_and_no_cwd() {
        // Roots are granted and `cwd` is withheld, so a relative spelling would otherwise be
        // refused as needing `cwd`; the nul byte must be reported first.
        let dir = tempfile::tempdir().unwrap();
        let root = crate::test_support::canonical(dir.path());

        let guard = sealed_guard(&root, false);
        for err in [
            guard.read("read", "a\0b").unwrap_err(),
            guard.write("write", "a\0b").unwrap_err(),
        ] {
            match err {
                crate::error::Error::UncheckablePath { reason, .. } => {
                    assert_eq!(reason, "it contains a nul byte");
                }
                other => panic!("expected UncheckablePath, got {other:?}"),
            }
        }
    }

    /// A guard over a sealed set that holds `stderr` (so a refusal has something to list), one read
    /// root and, when `with_cwd`, the `cwd` member.
    fn sealed_guard(root: &std::path::Path, with_cwd: bool) -> PathGuard {
        let mut grants = GrantSet::sealed()
            .with_access(HostAccess::Stderr)
            .with_fs(|fs| fs.read(root).write(root));
        if with_cwd {
            grants = grants.with_access(HostAccess::Cwd);
        }
        PathGuard::new(Arc::new(grants), "fs")
    }

    #[test]
    fn a_relative_path_without_cwd_is_refused_before_resolution() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();

        let guard = sealed_guard(&root, false);
        assert_eq!(
            guard.read("read", "p.lua").unwrap_err().to_string(),
            "fs.read denied: the relative path `p.lua` needs `cwd`, which is not granted — the \
             granted access is stderr"
        );
        assert_eq!(
            guard.write("write", "p.lua").unwrap_err().to_string(),
            "fs.write denied: the relative path `p.lua` needs `cwd`, which is not granted — the \
             granted access is stderr"
        );
    }

    #[test]
    fn a_dot_slash_path_without_cwd_is_refused_by_its_spelling() {
        // The refusal is on the spelling, not on where it would land, so it must not depend on
        // the process working directory at all.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();

        let guard = sealed_guard(&root, false);
        let err = guard.read("read", "./a.txt").unwrap_err();
        assert!(
            err.to_string()
                .contains("the relative path `./a.txt` needs `cwd`"),
            "{err}"
        );
    }

    #[test]
    fn no_roots_wins_over_the_relative_rule() {
        let guard = PathGuard::new(
            Arc::new(GrantSet::sealed().with_access(HostAccess::Stderr)),
            "fs",
        );
        assert_eq!(
            guard.read("read", "p.lua").unwrap_err().to_string(),
            "fs.read denied: `p.lua` is outside the granted read roots: none are granted"
        );
        assert_eq!(
            guard.write("write", "p.lua").unwrap_err().to_string(),
            "fs.write denied: `p.lua` is outside the granted write roots: none are granted"
        );
    }

    #[test]
    fn an_absolute_path_does_not_need_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();

        let guard = sealed_guard(&root, false);
        assert!(
            guard
                .read("read", &format!("{}/a.txt", script_path(&root)))
                .is_ok()
        );
    }

    #[test]
    fn a_relative_path_with_cwd_resolves_as_before() {
        // The process working directory is shared by every test, so it is not changed; the
        // current directory itself is the root, and a relative name resolves inside it.
        // `canonical`, not `canonicalize()`: on Windows the latter is `\\?\`-prefixed while
        // `resolve` strips that prefix, so the two would never compare equal.
        let cwd = crate::test_support::canonical(&std::env::current_dir().unwrap());

        let guard = sealed_guard(&cwd, true);
        let resolved = guard.read("read", "not-there.lua").unwrap();
        assert_eq!(resolved.as_path(), cwd.join("not-there.lua"));
        assert!(guard.write("write", "not-there.lua").is_ok());
    }

    #[test]
    fn a_relative_path_with_cwd_is_still_checked_against_the_roots() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();

        let guard = sealed_guard(&root, true);
        let err = guard.read("read", "not-there.lua").unwrap_err();
        assert!(
            err.to_string().contains("outside the granted read roots"),
            "{err}"
        );
    }

    #[test]
    fn an_unrestricted_set_skips_both_rules() {
        let guard = PathGuard::new(Arc::new(GrantSet::unrestricted()), "fs");
        // Cargo runs a crate's tests from the crate directory, so this relative path exists.
        assert!(guard.read("read", "Cargo.toml").is_ok());
        assert!(guard.write("write", "p.lua").is_ok());
    }

    #[test]
    fn an_unrestricted_policy_with_no_roots_still_permits() {
        let guard = PathGuard::new(Arc::new(GrantSet::unrestricted()), "fs");
        assert!(guard.write("write", "/etc/hostname").is_ok());
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
