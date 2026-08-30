//! An environment variable name, compared the way the platform itself compares one.
//!
//! Its own module because two different sites in this crate — an allowlist and an overlay map —
//! both need the *same* answer to "are these two names the same variable", and that answer is not
//! a crate-wide constant: the Windows environment block is case-insensitive unconditionally, while
//! a unix one is not. A type that owns the comparison means the allowlist and the overlay cannot
//! quietly drift into disagreeing about it.
//!
//! [`EnvName`] is total rather than validating, which is the opposite of every other newtype this
//! crate builds around a string, and the difference is deliberate rather than an oversight:
//!
//! - Its constructor is infallible. It wraps a [`String`] and rejects nothing — no `Result`, no
//!   error type, no `# Errors` section.
//! - The surrounding API forces that shape. The grant builder this type feeds is `#[must_use]` and
//!   returns `Self`, not a `Result`, and that signature is public and held fixed — a rejecting
//!   constructor would have nowhere to report a failure without breaking it.
//! - Rejecting by panicking is not available either: this workspace denies `panic!` outside test
//!   code.
//! - Rejecting nothing is nonetheless fail-closed. A spelling that can never occur in a real
//!   environment — one containing `=`, or an interior NUL — simply never matches anything: not a
//!   host variable, not an overlay entry, not another legally spelled grant name. The failure mode
//!   of an unvalidated name is that it grants and reveals nothing, which is the outcome a
//!   validating constructor would have produced anyway, without a `Result` anywhere in the path.
//!
//! The fold itself: [`EnvName`]'s `Eq`, `Ord` and `Hash` fold ASCII case on Windows and compare
//! exactly on unix, because the Windows environment block answers "same name" that way regardless
//! of what created it. A name containing anything outside ASCII compares exactly on *both*
//! platforms — narrower than the Unicode fold Windows itself performs, so a non-ASCII name never
//! matches more on this crate's Windows leg than it would on Windows itself, and a grant never
//! becomes wider than intended for want of a fold this crate does not implement. The same
//! narrowness shows up in a `BTreeSet`/`HashMap` keyed on [`EnvName`]: a non-ASCII host name and
//! an overlay spelling that differs only in case stay two separate entries there even on Windows,
//! where the platform's own block would treat them as one. That again can only make this crate
//! see more names than the platform does, never fewer, so it stays on the fail-closed side.
//!
//! Responsibilities:
//!
//! - [`EnvName`], the identity two environment variable names are compared and hashed under.
//! - `CaseRule` and `NATIVE`, the explicit, testable form of that platform choice: a rule taken
//!   as a value rather than read from `cfg!(windows)` inside the comparison itself, so both rules
//!   run — and can fail — on any host this crate is tested from, not only the one CI happens to
//!   reach.
//!
//! Non-responsibilities: validation, and resolving a name against an actual process environment.
//! This type only decides whether two spellings name the same variable.

use core::cmp::Ordering;
use core::hash::{Hash, Hasher};

/// Which platform's identity rule two environment variable names are compared under.
///
/// Explicit and passed by value, rather than read from `cfg!(windows)` inside the comparison
/// itself, so both rules can be exercised — and can fail — from a single, non-Windows test host.
/// Mirrors [`crate::paths::rules::PathFlavor`], which exists for the same reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaseRule {
    /// Byte-for-byte comparison: `PATH` and `Path` are two different names.
    Exact,
    /// ASCII case is folded before comparing: `PATH` and `Path` are the same name. Bytes outside
    /// the ASCII range are left exactly as written, so a non-ASCII name still compares exactly.
    FoldAscii,
}

/// The rule this crate was compiled to use.
///
/// `FoldAscii` on Windows because the environment block there is case-insensitive
/// unconditionally, regardless of how a particular name was set; `Exact` everywhere else.
pub(crate) const NATIVE: CaseRule = if cfg!(windows) {
    CaseRule::FoldAscii
} else {
    CaseRule::Exact
};

/// Orders `a` against `b` under `rule`.
///
/// Allocates nothing in either branch: `FoldAscii` compares the two byte iterators, each mapped
/// through [`u8::to_ascii_uppercase`], directly. Uppercase rather than lowercase because that is
/// the direction the platform this rule exists for folds a name in.
pub(crate) fn compare(a: &str, b: &str, rule: CaseRule) -> Ordering {
    match rule {
        CaseRule::Exact => a.cmp(b),
        CaseRule::FoldAscii => a
            .bytes()
            .map(|byte| byte.to_ascii_uppercase())
            .cmp(b.bytes().map(|byte| byte.to_ascii_uppercase())),
    }
}

/// The name of an environment variable, compared under the platform's own notion of identity.
///
/// See the module documentation for why this type accepts every string rather than validating —
/// unlike this crate's other newtypes, `new` cannot fail.
///
/// # Examples
///
/// ```
/// use airsl::EnvName;
///
/// let name = EnvName::new("PATH");
/// assert_eq!(name.as_str(), "PATH");
///
/// // Nothing is ever rejected. A spelling no real environment can carry — here, one containing
/// // `=` — is still an ordinary value; it simply matches no host variable, overlay entry, or
/// // grant that anyone could spell legally.
/// let impossible = EnvName::new("=C:");
/// assert_eq!(impossible.as_str(), "=C:");
/// ```
#[derive(Debug, Clone)]
pub struct EnvName(String);

impl EnvName {
    /// Wraps `raw` as an environment variable name. Infallible: see the module documentation for
    /// why this constructor has no `Result`.
    #[must_use]
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// The name exactly as given, with no case folding applied.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// `PartialEq`, `Eq`, `PartialOrd`, `Ord` and `Hash` are all hand-written, and all delegate to
// `compare(.., NATIVE)`, rather than deriving any one of them next to the others. Deriving
// `PartialEq` (byte equality) alongside a folded `Ord` or `Hash` would break the agreement
// `BTreeSet`, `BTreeMap` and `HashMap` all assume: two names that a folded `Ord` places at the
// same position, or a folded `Hash` sends to the same bucket, must also report `Eq`, or lookups
// silently stop finding entries that are logically present.

impl PartialEq for EnvName {
    fn eq(&self, other: &Self) -> bool {
        compare(&self.0, &other.0, NATIVE) == Ordering::Equal
    }
}

impl Eq for EnvName {}

impl PartialOrd for EnvName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EnvName {
    fn cmp(&self, other: &Self) -> Ordering {
        compare(&self.0, &other.0, NATIVE)
    }
}

/// Hashes `name` into `state` under `rule`, mirroring [`compare`]'s pairing of comparison and
/// hash: whichever branch `compare` folds under a rule, this folds identically, so two names
/// `compare` reports equal under `rule` also hash equal under it.
pub(crate) fn hash_under<H: Hasher>(name: &str, rule: CaseRule, state: &mut H) {
    match rule {
        CaseRule::Exact => name.hash(state),
        CaseRule::FoldAscii => {
            for byte in name.bytes() {
                state.write_u8(byte.to_ascii_uppercase());
            }
            // A terminator, matching the shape `str`'s own `Hash` impl already gives the
            // `Exact` branch above by delegating to `str::hash`. Without one, a folded struct
            // with more than one string field could hash `("AB", "C")` and `("A", "BC")`
            // identically; writing it here keeps that property true even though this type
            // carries only a single field today.
            state.write_u8(0xFF);
        }
    }
}

impl Hash for EnvName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_under(&self.0, NATIVE, state);
    }
}

impl core::fmt::Display for EnvName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for EnvName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{CaseRule, EnvName, compare, hash_under};
    use core::cmp::Ordering;

    fn hash_of(name: &str, rule: CaseRule) -> u64 {
        use core::hash::Hasher as _;
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        hash_under(name, rule, &mut hasher);
        hasher.finish()
    }

    #[test]
    fn folding_ascii_case_makes_path_and_path_the_same_name() {
        assert_eq!(
            compare("PATH", "Path", CaseRule::FoldAscii),
            Ordering::Equal
        );
    }

    #[test]
    fn exact_comparison_treats_path_and_path_as_different_names() {
        assert_ne!(compare("PATH", "Path", CaseRule::Exact), Ordering::Equal);
    }

    #[test]
    fn folded_ordering_compares_the_uppercased_bytes_not_the_original_ones() {
        assert_eq!(
            compare("PATH", "PATHEXT", CaseRule::FoldAscii),
            Ordering::Less
        );
        // A lowercase-led spelling still folds before it compares, so it orders exactly where its
        // uppercase form would.
        assert_eq!(
            compare("Path", "PATHEXT", CaseRule::FoldAscii),
            Ordering::Less
        );
    }

    #[test]
    fn folding_is_ascii_only_and_leaves_non_ascii_case_pairs_unequal() {
        // Deliberate: Windows' own fold is Unicode-aware, so stopping at ASCII is narrower than
        // the platform's real rule, which fails closed for a grant rather than open.
        assert_ne!(compare("Ä", "ä", CaseRule::FoldAscii), Ordering::Equal);
    }

    #[test]
    fn two_empty_names_compare_equal_under_either_rule() {
        assert_eq!(compare("", "", CaseRule::Exact), Ordering::Equal);
        assert_eq!(compare("", "", CaseRule::FoldAscii), Ordering::Equal);
    }

    #[cfg(windows)]
    #[test]
    fn under_the_native_rule_path_and_path_are_the_same_name_on_windows() {
        assert_eq!(EnvName::new("PATH"), EnvName::new("Path"));
    }

    #[cfg(unix)]
    #[test]
    fn under_the_native_rule_path_and_path_are_different_names_on_unix() {
        assert_ne!(EnvName::new("PATH"), EnvName::new("Path"));
    }

    #[test]
    fn as_str_returns_the_spelling_as_given_never_folded() {
        assert_eq!(EnvName::new("Path").as_str(), "Path");
        assert_eq!(EnvName::new("PATH").as_str(), "PATH");
    }

    #[test]
    fn hashing_agrees_with_comparison_under_either_rule() {
        // Mirrors `compare`'s own rule-parameterised tests, so both rules are exercisable on any
        // host rather than only the one `NATIVE` happens to select here. Under `FoldAscii`,
        // "PATH" and "Path" compare equal (see `folding_ascii_case_makes_path_and_path_the_same_name`
        // above), so `Hash`/`Eq` agreement requires them to hash equal too. Under `Exact` the same
        // pair compares unequal, and this hasher — `DefaultHasher::new()` seeds from fixed keys,
        // not process randomness, so its output is deterministic across runs — actually tells
        // them apart, which is what makes this pair a real discriminator rather than one that
        // happens to pass under any `Hash` impl.
        assert_eq!(
            hash_of("PATH", CaseRule::FoldAscii),
            hash_of("Path", CaseRule::FoldAscii)
        );
        assert_ne!(
            hash_of("PATH", CaseRule::Exact),
            hash_of("Path", CaseRule::Exact)
        );
    }

    #[cfg(windows)]
    #[test]
    fn names_that_fold_equal_on_windows_also_hash_equal_there() {
        fn hash_of(name: &EnvName) -> u64 {
            use core::hash::{Hash as _, Hasher as _};
            use std::collections::hash_map::DefaultHasher;

            let mut hasher = DefaultHasher::new();
            name.hash(&mut hasher);
            hasher.finish()
        }

        assert_eq!(
            hash_of(&EnvName::new("PATH")),
            hash_of(&EnvName::new("Path"))
        );
    }
}
