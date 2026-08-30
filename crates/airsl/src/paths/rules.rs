//! Lexical path rules, parameterised by which platform's spelling they apply to.
//!
//! Its own module because these rules must be testable on a platform they do not target.
//! A rule written as `#[cfg(windows)] fn ...` compiles only on Windows, so its correctness is
//! unverified everywhere else until a Windows runner happens to exercise it. Taking the platform
//! as an explicit [`PathFlavor`] argument instead means the Windows rules run, and can fail, on
//! this crate's Linux and macOS test hosts too. None of these functions touch the filesystem: they
//! reason about path *spelling*, never about what exists on disk.
//!
//! Responsibilities:
//!
//! - [`PathFlavor`] and [`Unrepresentable`], the vocabulary the rules share.
//! - The five flavour-taking rule functions: [`reject_unrepresentable`], [`to_script_string`],
//!   [`strip_verbatim`], [`executable_candidates`], [`has_separator`].
//! - [`native`], thin wrappers over the same rules bound to the compile-time platform, for callers
//!   that never need to name a flavour explicitly.
//!
//! Non-responsibilities: resolving a path against the filesystem (symlinks, existence, absoluteness
//! beyond string shape) and comparing a path against a granted root — both belong to sibling
//! modules.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::{Path, PathBuf};

/// Which platform's path spelling rules apply.
///
/// Explicit and passed by value, rather than read from `cfg!(windows)` inside each rule, so that
/// every rule in this module can be exercised for both platforms from a single test host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathFlavor {
    /// Unix path spelling: `/`-separated, no reserved names, backslash is an ordinary character.
    Posix,
    /// Windows path spelling: `/` or `\` separators, verbatim (`\\?\`) and device-namespace
    /// (`\\.\`) prefixes, and a set of names reserved regardless of directory or extension.
    Windows,
}

/// Why a raw path string cannot be reasoned about at all, and must be refused before any
/// resolution is attempted.
///
/// A caller that receives this error has not been denied by policy — the string does not name a
/// path this runtime understands, on the flavour it was checked against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unrepresentable {
    /// A `\\?\` (or `\\?\UNC\`) verbatim prefix. Verbatim paths skip the normal Win32 path parser
    /// — `/` stops being a separator inside one — so a `..` written after the prefix does not mean
    /// what it means everywhere else in this crate.
    Verbatim,
    /// A device namespace: the `\\.\` prefix, or a reserved device name such as `NUL` or `COM1`
    /// appearing as a path component. These name hardware or kernel objects, not files, on
    /// Windows only.
    DeviceNamespace,
    /// A `\0` byte inside the string. No syscall on either platform family this crate targets can
    /// open a path containing one, so this variant is common to both flavours.
    InteriorNul,
}

/// The Windows-reserved device names, checked case-insensitively and independent of any
/// extension: Windows treats `NUL.txt` as the `NUL` device exactly as it treats `NUL`, because the
/// device namespace is resolved before the extension is considered. Checked per path component
/// (split on both `/` and `\`, since input may arrive in either spelling per this crate's
/// accept-either-separator rule) rather than against the whole string, so `a/NUL/b` is refused even
/// though the full string is not itself one of these names.
///
/// Includes `COM0`/`LPT0` and the superscript forms (`COM¹`, `COM²`, `COM³`, `LPT¹`, `LPT²`,
/// `LPT³`) Microsoft documents alongside `COM1`–`COM9`: Windows resolves all of these to a device
/// exactly as it resolves the plain-digit spellings, so a list that stopped at `COM9` would let a
/// device through under a name Windows still opens as one.
const RESERVED_DEVICE_NAMES: [&str; 30] = [
    "CON",
    "PRN",
    "AUX",
    "NUL",
    "COM0",
    "COM1",
    "COM2",
    "COM3",
    "COM4",
    "COM5",
    "COM6",
    "COM7",
    "COM8",
    "COM9",
    "COM\u{b9}",
    "COM\u{b2}",
    "COM\u{b3}",
    "LPT0",
    "LPT1",
    "LPT2",
    "LPT3",
    "LPT4",
    "LPT5",
    "LPT6",
    "LPT7",
    "LPT8",
    "LPT9",
    "LPT\u{b9}",
    "LPT\u{b2}",
    "LPT\u{b3}",
];

/// Whether any component of `raw` names a reserved Windows device, ignoring case, any extension,
/// and any trailing spaces or dots.
///
/// Windows trims trailing `' '` and `'.'` from a component during path normalisation before
/// resolving it, so `"CON "` and `"CON."` open the `CON` device exactly as `"CON"` does. Comparing
/// the raw component would miss both.
fn names_reserved_device(raw: &str) -> bool {
    raw.split(['/', '\\']).any(|component| {
        let trimmed = component.trim_end_matches([' ', '.']);
        let base = trimmed.split('.').next().unwrap_or(trimmed);
        RESERVED_DEVICE_NAMES
            .iter()
            .any(|reserved| base.eq_ignore_ascii_case(reserved))
    })
}

/// Strips a `\\?\` or `\\?\UNC\` verbatim prefix from a string, leaving everything else
/// unchanged. Pure string manipulation — no [`Path`] component parsing — because on a host that is
/// not Windows, `std::path` parses these bytes with the host's own rules and would not recognise
/// the prefix as a prefix at all.
fn strip_verbatim_prefix(raw: &str) -> String {
    raw.strip_prefix(r"\\?\UNC\").map_or_else(
        || {
            raw.strip_prefix(r"\\?\")
                .map_or_else(|| raw.to_owned(), str::to_owned)
        },
        |rest| format!(r"\\{rest}"),
    )
}

/// Whether `raw` opens with two separators (either `/` or `\`, in any combination) followed by
/// `marker`.
///
/// Windows' own path-type classifier accepts either separator in these four leading bytes — this
/// crate does not try to settle exactly which spellings `std::path` recognises as a prefix on a
/// real Windows host, because that cannot be verified from a unix test runner. Instead the check is
/// widened to match anything Windows *might* treat as the namespace escape, so the door closes
/// regardless of the exact answer: `?` is not a valid server name in a UNC path and `.` in that
/// position is the device namespace by definition, so neither spelling rejects anything that could
/// legitimately be a normal path.
fn has_namespace_prefix(raw: &str, marker: char) -> bool {
    let mut chars = raw.chars();
    matches!(chars.next(), Some('/' | '\\'))
        && matches!(chars.next(), Some('/' | '\\'))
        && chars.next() == Some(marker)
        && matches!(chars.next(), Some('/' | '\\'))
}

/// Refuses path spellings this runtime will not reason about, before any resolution is attempted.
///
/// Interior NUL bytes are refused on both flavours, since no syscall on either platform family can
/// open a path containing one. The remaining checks apply to `Windows` only: refusing a verbatim
/// prefix, a device-namespace prefix, or a reserved device name (`NUL`, `CON`, `COM1`, …) as any
/// path component. Under `Posix` a file named `NUL` or `con` is an ordinary file — refusing it
/// there would regress unix behaviour rather than protect anything.
///
/// The verbatim and device-namespace prefixes are matched by [`has_namespace_prefix`], not by the
/// single `\\?\` / `\\.\` spelling — see its doc comment for why the separator is not fixed.
///
/// # Errors
///
/// The [`Unrepresentable`] variant naming why the string cannot be reasoned about.
pub(crate) fn reject_unrepresentable(raw: &str, flavor: PathFlavor) -> Result<(), Unrepresentable> {
    if raw.contains('\0') {
        return Err(Unrepresentable::InteriorNul);
    }

    if flavor == PathFlavor::Posix {
        return Ok(());
    }

    if has_namespace_prefix(raw, '?') {
        return Err(Unrepresentable::Verbatim);
    }

    if has_namespace_prefix(raw, '.') || names_reserved_device(raw) {
        return Err(Unrepresentable::DeviceNamespace);
    }

    Ok(())
}

/// Converts `path` to the `/`-separated vocabulary a script sees.
///
/// Unchanged under `Posix`: a unix filename may legally contain a literal backslash, so converting
/// it there would corrupt a real name. Under `Windows`, a filename may never contain `/`, so the
/// conversion the other way is unambiguous and reversible — every `\` in a Windows path is a
/// separator, never data. Any verbatim prefix is stripped first, so the result is never `\\?\`-
/// prefixed either. A UNC root (`\\srv\share`) keeps both of its leading separators — rendered as
/// `//srv/share` — so a script can tell it apart from an absolute local path (`/a/b`), which has
/// only one.
pub(crate) fn to_script_string(path: &Path, flavor: PathFlavor) -> String {
    let raw = path.to_string_lossy();
    match flavor {
        PathFlavor::Posix => raw.into_owned(),
        PathFlavor::Windows => strip_verbatim_prefix(&raw).replace('\\', "/"),
    }
}

/// Removes a `\\?\` / `\\?\UNC\` prefix from a `canonicalize()` result.
///
/// A no-op under `Posix`, which has no such prefix. Every `canonicalize()` call site in this crate
/// must apply this on the way in, so a verbatim spelling is never stored or compared against a
/// non-verbatim one.
pub(crate) fn strip_verbatim(path: PathBuf, flavor: PathFlavor) -> PathBuf {
    match flavor {
        PathFlavor::Posix => path,
        PathFlavor::Windows => PathBuf::from(strip_verbatim_prefix(&path.to_string_lossy())),
    }
}

/// The file names to try on `PATH` for a bare program name, in order.
///
/// Under `Posix` this is `[program]` unchanged. Under `Windows` this crate resolves the executable
/// itself rather than delegating to the platform loader's `PATHEXT` search: if `program` already
/// ends in `.exe` (checked case-insensitively, since Windows filenames are case-preserving but not
/// case-sensitive), it is tried as written; otherwise `.exe` is appended. `.bat`/`.cmd` shims are
/// deliberately never produced here, since spawning one routes argv through `cmd.exe`, whose
/// quoting rules differ from the ones this crate's process module is built to avoid entirely.
pub(crate) fn executable_candidates(program: &str, flavor: PathFlavor) -> Vec<String> {
    match flavor {
        PathFlavor::Posix => vec![program.to_owned()],
        PathFlavor::Windows => {
            // `get` rather than indexing: a byte-length check alone does not guarantee the slice
            // point falls on a char boundary, and indexing there would panic on a multi-byte name.
            let already_exe = program
                .len()
                .checked_sub(4)
                .and_then(|start| program.get(start..))
                .is_some_and(|tail| tail.eq_ignore_ascii_case(".exe"));
            if already_exe {
                vec![program.to_owned()]
            } else {
                vec![format!("{program}.exe")]
            }
        }
    }
}

/// Whether `program` names a path rather than a bare name to search `PATH` for.
///
/// Under `Posix`, only `/` separates path components. Under `Windows`, both `/` and `\` do, since
/// the Win32 API accepts either.
#[cfg_attr(
    not(any(windows, test)),
    expect(
        dead_code,
        reason = "exercised directly by this module's own tests on every host, and by \
                  `native::has_separator`'s Windows-only caller in `modules::proc` on Windows; a \
                  non-test unix build has neither, which is genuinely dead rather than forgotten"
    )
)]
pub(crate) fn has_separator(program: &str, flavor: PathFlavor) -> bool {
    match flavor {
        PathFlavor::Posix => program.contains('/'),
        PathFlavor::Windows => program.contains('/') || program.contains('\\'),
    }
}

/// The same five rules, bound to the platform this crate was compiled for.
///
/// Every caller outside this module reaches for these, never the flavour-taking functions above
/// directly — naming a flavour explicitly is a test's job, not a caller's.
pub(crate) mod native {
    use super::{PathFlavor, Unrepresentable};
    use std::path::{Path, PathBuf};

    /// The platform this crate was compiled for.
    pub(crate) const FLAVOR: PathFlavor = if cfg!(windows) {
        PathFlavor::Windows
    } else {
        PathFlavor::Posix
    };

    /// See [`super::reject_unrepresentable`].
    ///
    /// # Errors
    ///
    /// As [`super::reject_unrepresentable`].
    pub(crate) fn reject_unrepresentable(raw: &str) -> Result<(), Unrepresentable> {
        super::reject_unrepresentable(raw, FLAVOR)
    }

    /// See [`super::to_script_string`].
    pub(crate) fn to_script_string(path: &Path) -> String {
        super::to_script_string(path, FLAVOR)
    }

    /// See [`super::strip_verbatim`].
    pub(crate) fn strip_verbatim(path: PathBuf) -> PathBuf {
        super::strip_verbatim(path, FLAVOR)
    }

    /// See [`super::executable_candidates`].
    pub(crate) fn executable_candidates(program: &str) -> Vec<String> {
        super::executable_candidates(program, FLAVOR)
    }

    /// See [`super::has_separator`].
    ///
    /// `#[cfg(windows)]` here, unlike its four siblings above: its one caller
    /// ([`crate::modules::proc`]'s bare-name pre-resolution) is itself Windows-only by design —
    /// unix keeps `execvp`'s own `PATH` search — so a unix build has no call site for this wrapper
    /// at all. The rule it wraps, [`super::has_separator`], stays flavour-parameterised and
    /// unconditionally compiled, so it is still exercised for both flavours from this crate's
    /// unix test hosts; only the compile-time-bound convenience wrapper is native to one platform.
    #[cfg(windows)]
    pub(crate) fn has_separator(program: &str) -> bool {
        super::has_separator(program, FLAVOR)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PathFlavor::{Posix, Windows},
        Unrepresentable, executable_candidates, has_separator, reject_unrepresentable,
        strip_verbatim, to_script_string,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn a_verbatim_prefix_is_rejected_under_windows_but_is_an_ordinary_name_under_posix() {
        assert_eq!(
            reject_unrepresentable(r"\\?\C:\x", Windows),
            Err(Unrepresentable::Verbatim)
        );
        assert_eq!(reject_unrepresentable(r"\\?\C:\x", Posix), Ok(()));
    }

    #[test]
    fn a_device_namespace_prefix_is_rejected_only_under_windows() {
        assert_eq!(
            reject_unrepresentable(r"\\.\PhysicalDrive0", Windows),
            Err(Unrepresentable::DeviceNamespace)
        );
        assert_eq!(reject_unrepresentable(r"\\.\PhysicalDrive0", Posix), Ok(()));
    }

    #[test]
    fn a_verbatim_prefix_is_rejected_regardless_of_which_separator_spells_it() {
        // Windows' own path-type classifier accepts either separator in these four leading bytes;
        // the door check must too, or a mixed-separator spelling walks past it. Under `Posix` all
        // four are ordinary, if odd, filenames.
        for spelling in [r"//?/C:/x", r"\\?/C:/x", r"/\?\C:\x"] {
            assert_eq!(
                reject_unrepresentable(spelling, Windows),
                Err(Unrepresentable::Verbatim),
                "{spelling} should be rejected as verbatim under Windows"
            );
            assert_eq!(
                reject_unrepresentable(spelling, Posix),
                Ok(()),
                "{spelling} is an ordinary filename under Posix"
            );
        }
    }

    #[test]
    fn a_device_namespace_prefix_is_rejected_regardless_of_which_separator_spells_it() {
        assert_eq!(
            reject_unrepresentable(r"//./PhysicalDrive0", Windows),
            Err(Unrepresentable::DeviceNamespace)
        );
        assert_eq!(reject_unrepresentable(r"//./PhysicalDrive0", Posix), Ok(()));
    }

    #[test]
    fn a_reserved_device_name_is_rejected_under_windows_but_is_a_real_file_under_posix() {
        assert_eq!(
            reject_unrepresentable("NUL", Windows),
            Err(Unrepresentable::DeviceNamespace)
        );
        // A real, ordinary file on Linux; refusing it there would regress unix behaviour.
        assert_eq!(reject_unrepresentable("NUL", Posix), Ok(()));
    }

    #[test]
    fn a_reserved_device_name_is_caught_regardless_of_case_extension_or_directory_depth() {
        assert_eq!(
            reject_unrepresentable("con", Windows),
            Err(Unrepresentable::DeviceNamespace)
        );
        assert_eq!(
            reject_unrepresentable("NUL.txt", Windows),
            Err(Unrepresentable::DeviceNamespace)
        );
        assert_eq!(
            reject_unrepresentable(r"a\COM1\b", Windows),
            Err(Unrepresentable::DeviceNamespace)
        );
        assert_eq!(reject_unrepresentable("COM10", Windows), Ok(()));
    }

    #[test]
    fn a_reserved_device_name_is_caught_with_trailing_spaces_or_dots_because_windows_trims_them() {
        assert_eq!(
            reject_unrepresentable("CON ", Windows),
            Err(Unrepresentable::DeviceNamespace)
        );
        assert_eq!(
            reject_unrepresentable("CON.", Windows),
            Err(Unrepresentable::DeviceNamespace)
        );
    }

    #[test]
    fn the_zero_and_superscript_device_names_are_reserved_too() {
        assert_eq!(
            reject_unrepresentable("COM0", Windows),
            Err(Unrepresentable::DeviceNamespace)
        );
        assert_eq!(
            reject_unrepresentable("LPT0", Windows),
            Err(Unrepresentable::DeviceNamespace)
        );
        assert_eq!(
            reject_unrepresentable("COM\u{b9}", Windows),
            Err(Unrepresentable::DeviceNamespace)
        );
    }

    #[test]
    fn an_interior_nul_is_rejected_on_both_flavours() {
        assert_eq!(
            reject_unrepresentable("a\0b", Windows),
            Err(Unrepresentable::InteriorNul)
        );
        assert_eq!(
            reject_unrepresentable("a\0b", Posix),
            Err(Unrepresentable::InteriorNul)
        );
    }

    #[test]
    fn a_normal_path_is_accepted_on_both_flavours() {
        assert_eq!(reject_unrepresentable("a/b/c.txt", Windows), Ok(()));
        assert_eq!(reject_unrepresentable("a/b/c.txt", Posix), Ok(()));
    }

    #[test]
    fn to_script_string_converts_backslashes_only_under_windows() {
        assert_eq!(to_script_string(Path::new(r"a\b"), Windows), "a/b");
        // A legal unix filename containing a literal backslash: must survive unchanged.
        assert_eq!(to_script_string(Path::new(r"a\b"), Posix), r"a\b");
    }

    #[test]
    fn to_script_string_strips_a_verbatim_prefix_under_windows() {
        assert_eq!(
            to_script_string(Path::new(r"\\?\C:\a\b"), Windows),
            "C:/a/b"
        );
    }

    #[test]
    fn to_script_string_renders_a_unc_root_with_both_leading_separators() {
        // Keeping both separators is what lets a script tell a UNC root apart from an absolute
        // local path, which has only one.
        assert_eq!(
            to_script_string(Path::new(r"\\srv\share\x"), Windows),
            "//srv/share/x"
        );
    }

    #[test]
    fn strip_verbatim_removes_the_disk_prefix() {
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\C:\x"), Windows),
            PathBuf::from(r"C:\x")
        );
    }

    #[test]
    fn strip_verbatim_removes_the_unc_prefix_and_restores_the_ordinary_unc_spelling() {
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\UNC\srv\share"), Windows),
            PathBuf::from(r"\\srv\share")
        );
    }

    #[test]
    fn strip_verbatim_is_a_no_op_under_posix() {
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\C:\x"), Posix),
            PathBuf::from(r"\\?\C:\x")
        );
    }

    #[test]
    fn executable_candidates_appends_exe_under_windows_unless_already_present() {
        assert_eq!(executable_candidates("git", Windows), vec!["git.exe"]);
        assert_eq!(executable_candidates("git.exe", Windows), vec!["git.exe"]);
        // The case-insensitive suffix check: an already-cased .EXE is left exactly as written.
        assert_eq!(executable_candidates("GIT.EXE", Windows), vec!["GIT.EXE"]);
    }

    #[test]
    fn executable_candidates_is_unchanged_under_posix() {
        assert_eq!(executable_candidates("git", Posix), vec!["git"]);
    }

    #[test]
    fn has_separator_recognises_only_forward_slash_under_posix() {
        assert!(has_separator("a/b", Posix));
        assert!(!has_separator(r"a\b", Posix));
    }

    #[test]
    fn has_separator_recognises_either_slash_under_windows() {
        assert!(has_separator("a/b", Windows));
        assert!(has_separator(r"a\b", Windows));
    }
}
