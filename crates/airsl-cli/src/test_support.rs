//! Fixture helpers shared by this crate's command tests.
//!
//! Its own module because more than one command's tests build a fixture by interpolating a temp
//! directory into a TOML or Lua literal, and the rule for doing that safely is one rule rather
//! than one per command. Written here rather than borrowed from the library: `airsl` normalises
//! the same way for its own tests, but that helper is `pub(crate)` there, and widening the
//! library's public API to serve test code in this crate would put a permanent item on its
//! surface for no runtime caller.
//!
//! Responsibilities: [`script_literal`], the spelling a fixture may safely interpolate, and
//! [`abs`], the absolute path a fixture needs when it only cares that the path is absolute, not
//! what it is.
//!
//! Non-responsibilities: rendering a path a *command* prints. That is the command's own concern,
//! and a fixture helper must never become the thing under test.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::Path;

/// Renders `path` so a TOML basic string or a Lua quoted string can carry it verbatim.
///
/// A temp path on Windows carries backslashes, which both literal kinds read as escape sequences —
/// `C:\Users\…` opens a `\U` escape — so a fixture built from a raw `Path::display` fails to parse
/// instead of exercising whatever the test is for. Forward slashes are accepted wherever this
/// runtime takes a path, so the normalised spelling means the same thing on both platforms and the
/// expectation does not move.
pub(crate) fn script_literal(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// Turns a unix-spelled absolute path literal like `"/repo"` into whatever counts as absolute on
/// the platform actually running the test: unchanged on unix, prefixed with a drive letter on
/// Windows.
///
/// A bare `/repo` has a root but no drive, so `Path::is_absolute` is false for it on Windows and
/// the manifest validator this crate's fixtures exercise (`Manifest::from_dir`, in `airsl`) refuses
/// it before a negotiation or a grant is ever built. A fixture that only needs "some absolute
/// path" builds it here instead of hardcoding a spelling that is only absolute on one platform.
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
    use std::path::{Path, PathBuf};

    use super::{abs, script_literal};

    #[test]
    fn script_literal_replaces_every_backslash_with_a_forward_slash() {
        let rendered = script_literal(&PathBuf::from(r"C:\Users\me\extension.toml"));
        assert_eq!(rendered, "C:/Users/me/extension.toml");
    }

    #[test]
    fn script_literal_leaves_a_path_with_no_backslashes_unchanged() {
        // Spelled out rather than derived: an expectation built by applying this function's own
        // replacement to the input restates the implementation and cannot fail, whatever the
        // function does.
        assert_eq!(
            script_literal(Path::new("/tmp/fixture/main.lua")),
            "/tmp/fixture/main.lua"
        );
    }

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
}
