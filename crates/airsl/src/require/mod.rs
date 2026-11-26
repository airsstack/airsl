//! The `require` a script is given: which one, and how the confined one resolves modules.
//!
//! A folder rather than one file because the concept has two distinct roles that change for
//! different reasons: choosing between the four possible `require` globals, and the loader that
//! installs and runs the confined one.
//!
//! Responsibilities:
//!
//! - [`disposition`], the choice of `require` for a surface, a grant set and a script root.
//! - [`loader`], installing the confined, refusing or root-only `require` and loading modules.
//! - [`root`], where the engine's root table is kept for `require` and how its name is matched.
//!
//! Non-responsibilities: applying the disposition to a Lua state, which [`crate::Engine`] does.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

mod disposition;
mod loader;
pub(crate) mod root;

pub(crate) use disposition::RequireDisposition;
pub(crate) use loader::RequireLoader;
