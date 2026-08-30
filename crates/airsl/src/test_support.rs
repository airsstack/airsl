//! Filesystem fixtures shared by every module's containment and boundary tests.
//!
//! Its own module because the same two problems recur in every test that proves a path cannot
//! escape a granted root: the test needs to create a symlink, and the underlying `std` call
//! differs by platform and by whether the link's target is a file or a directory; and the test
//! needs a temp directory's canonical spelling as a string, which on Windows carries a `\\?\`
//! prefix this runtime's own door check refuses as input. Concentrating both here, rather than
//! writing a platform match at each call site, means there is exactly one Windows arm to get
//! right instead of one per test.
//!
//! Responsibilities:
//!
//! - [`link_file`] and [`link_dir`], the one call a test makes to create a symlink, in place of
//!   reaching for `std::os::unix::fs::symlink` or `std::os::windows::fs::symlink_file`/
//!   `symlink_dir` directly. Getting the file/directory choice wrong produces a link that still
//!   resolves but that Windows will not traverse, which turns a containment assertion into a
//!   false green rather than a real one.
//! - [`script_path`], the string a test interpolates into a fixture in place of a raw
//!   `canonicalize()` result.
//! - [`abs`], the absolute path a test needs when the fixture only cares that the path is
//!   absolute, not what it is.
//!
//! Non-responsibilities: deciding whether a path is inside a grant, and choosing which of read or
//! write an operation needs. Both stay with [`crate::modules::guard`], the module most of these
//! fixtures exist to exercise.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]
#![expect(
    clippy::unwrap_used,
    reason = "test fixtures unwrap known-valid inputs; a panic is the intended failure signal"
)]

use std::io;
use std::path::Path;

/// Creates a symlink at `link` pointing at `target`, a file.
///
/// Unix has one symlink call and does not distinguish the target's kind. Windows fixes the kind
/// at creation time and never repairs a mismatch, so this is `symlink_file` there — `symlink_dir`
/// aimed at a file creates a link that still resolves but that the operating system will not
/// traverse, which makes a containment assertion pass without testing containment.
///
/// # Errors
///
/// Any [`io::Error`] `std`'s own symlink call returns, other than the Windows privilege failure
/// this function panics on instead.
///
/// # Panics
///
/// On Windows, when symlink creation is denied because the process is neither elevated nor
/// running with Developer Mode on. The panic names both, so a maintainer running the suite
/// locally is told what to enable rather than getting a false green from a fixture that was
/// never built.
pub(crate) fn link_file(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).map_err(panic_on_missing_privilege)
    }
}

/// Creates a symlink at `link` pointing at `target`, a directory.
///
/// See [`link_file`] for the platform split and the failure behaviour; this is the same helper
/// for the directory case.
///
/// # Errors
///
/// As [`link_file`].
///
/// # Panics
///
/// As [`link_file`].
pub(crate) fn link_dir(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link).map_err(panic_on_missing_privilege)
    }
}

/// The Windows error code `CreateSymbolicLinkW` returns when the caller is neither elevated nor
/// running with `SeCreateSymbolicLinkPrivilege` via Developer Mode.
#[cfg(windows)]
const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;

/// Turns the one denial a maintainer can actually fix into a panic that says how, and passes
/// every other error through unchanged.
///
/// A silent skip here would convert a containment test into decoration: the property these
/// fixtures exist to prove would stop being checked and nothing would say so.
#[cfg(windows)]
fn panic_on_missing_privilege(err: io::Error) -> io::Error {
    assert!(
        err.raw_os_error() != Some(ERROR_PRIVILEGE_NOT_HELD),
        "creating a symlink was denied: Windows requires Developer Mode (or an elevated \
         process) for unprivileged symlink creation. Enable it with `reg add \
         \"HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AppModelUnlock\" /t REG_DWORD \
         /f /v AllowDevelopmentWithoutDevLicense /d 1`, then re-run the suite. ({err})"
    );
    err
}

/// Renders `path` the way a script actually hands a path to the guard: absolute, but never
/// verbatim.
///
/// `canonicalize()` is the only way to get an existing directory's real, symlink-resolved
/// spelling, but on Windows it returns a `\\?\`-prefixed string. Interpolating that directly into
/// a test's input string would make the door check in `native::reject_unrepresentable` refuse the
/// input before the behaviour under test ever runs. The grant **root** may stay canonicalised,
/// because `sandbox::grants::resolve_root` strips it there; it is only the string standing in for
/// what a script would type that must not be verbatim.
pub(crate) fn script_path(path: &Path) -> String {
    crate::paths::rules::native::strip_verbatim(path.to_path_buf())
        .to_str()
        .unwrap()
        .to_owned()
}

/// Turns a unix-spelled absolute path literal like `"/repo"` into whatever counts as absolute on
/// the platform actually running the test: unchanged on unix, prefixed with a drive letter on
/// Windows.
///
/// A bare `/repo` has a root but no drive, so `Path::is_absolute` is false for it on Windows and
/// `std::path::absolute` would resolve it against whichever drive the test happens to run from
/// rather than leaving it alone — the fixture would then mean a different, unpredictable location
/// instead of failing outright. Fixtures in `sandbox::grants` and `extension::manifest` need "some
/// absolute path", not a specific one, so they build it here instead of hardcoding a spelling that
/// is only absolute on one platform.
///
/// # Panics
///
/// When `path` does not start with `/`. Prefixing a drive onto a relative segment yields `C:repo`,
/// which is drive-*relative* rather than absolute: it would resolve against the current directory
/// of that drive, so the fixture would quietly mean somewhere else instead of failing. The one
/// mistake this helper exists to prevent is worth refusing rather than encoding.
pub(crate) fn abs(path: &str) -> String {
    assert!(
        path.starts_with('/'),
        "`abs` takes a rooted, unix-spelled literal; got `{path}`"
    );
    #[cfg(unix)]
    {
        path.to_owned()
    }
    #[cfg(windows)]
    {
        format!("C:{path}")
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use std::path::Path;

    use super::{abs, script_path};

    #[test]
    fn abs_rejects_an_argument_that_is_not_rooted() {
        let unwound = std::panic::catch_unwind(|| abs("relative/path"));
        assert!(
            unwound.is_err(),
            "`abs` must panic on an argument that does not start with `/`"
        );
    }

    #[test]
    fn abs_produces_something_this_platform_calls_absolute() {
        assert!(
            Path::new(&abs("/repo")).is_absolute(),
            "abs(\"/repo\") must satisfy this platform's own `Path::is_absolute`"
        );
    }

    #[test]
    fn script_path_leaves_an_ordinary_absolute_path_unchanged() {
        // `strip_verbatim` is a no-op on unix, so this only proves `script_path` does not corrupt
        // a path that never carried a verbatim prefix; `script_path_strips_a_verbatim_prefix`,
        // below, is what actually exercises the branch this function exists for.
        let dir = tempfile::tempdir().unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        assert_eq!(Path::new(&script_path(&canonical)), canonical.as_path());
    }

    #[cfg(windows)]
    #[test]
    fn panic_on_missing_privilege_passes_an_unrelated_error_through_unchanged() {
        // The classification is specifically the Developer-Mode denial code; every other error —
        // here, an arbitrary unrelated `raw_os_error` — must reach the caller as `Err`, not be
        // swallowed into a panic that has nothing to do with symlink privilege.
        let other = std::io::Error::from_raw_os_error(5);
        let passed = super::panic_on_missing_privilege(other);
        assert_eq!(passed.raw_os_error(), Some(5));
    }

    #[cfg(windows)]
    #[test]
    fn script_path_strips_a_verbatim_prefix() {
        // The one case unix has nothing to prove: a `canonicalize()` result on Windows carries a
        // `\\?\` prefix, and `script_path` exists specifically so that prefix never reaches a
        // fixture string the guard's own door check would otherwise refuse as input.
        let verbatim = std::path::PathBuf::from(r"\\?\C:\repo");
        let rendered = script_path(&verbatim);
        assert!(
            !rendered.starts_with(r"\\?\"),
            "a verbatim prefix must not survive into what a test hands to the guard: {rendered}"
        );
        assert_eq!(rendered, r"C:\repo");
    }
}
