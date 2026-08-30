//! Platform path rules, the resolved-path guarantee, and the shared containment predicate.
//!
//! Its own module because platform path spelling is a concern that cuts across the filesystem
//! guard, the `require` loader, and the extension manifest and loader — each of those needs the
//! same lexical rules and the same containment comparison, and a copy per call site is exactly how
//! this crate ended up with the duplicate-containment problem this module replaces. Collecting the
//! rules here also lets them be pure and filesystem-free, which is what makes the Windows-only
//! rules testable on a host that is not Windows.
//!
//! Responsibilities:
//!
//! - [`rules`] — the flavour-parameterised lexical rules and their compile-time-bound `native`
//!   wrappers.
//! - [`resolved::ResolvedPath`] — the structural guarantee that a path has been resolved and
//!   carries no native spelling by accident.
//! - [`containment`] — [`containment::is_within`] and [`containment::contains_any`], the one
//!   root-comparison predicate every containment check in this crate shares.
//!
//! Non-responsibilities: resolving a path against the filesystem. The algorithm that walks a path
//! down to its deepest existing ancestor and canonicalises it stays with the one function that
//! needs it, `PathGuard::resolve` in [`crate::modules::guard`] — this module supplies the rules and
//! the type that function's result is expressed in, not the walk itself.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

pub(crate) mod containment;
pub(crate) mod resolved;
pub(crate) mod rules;

pub(crate) use resolved::ResolvedPath;
