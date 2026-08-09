//! The three independent axes of what a script is allowed to do.
//!
//! Grouped here because they are only meaningful together: a language surface without ceilings
//! stops a script reaching the filesystem but not hanging the host, and ceilings without a surface
//! bound what a script spends but not what it touches. [`Policy`] composes all three, and the
//! presets on it are what most callers should reach for.
//!
//! Responsibilities:
//!
//! - [`policy`] — [`Policy`] and the `trusted`/`confined`/`pure` presets.
//! - [`language_surface`] — which of Lua's own libraries a script sees.
//! - [`grant_set`] — what the host modules it reaches may touch.
//! - [`resource_limits`] — how much memory and execution it may consume.
//!
//! Non-responsibilities: what a caller does about a script that failed. That is
//! [`crate::FailurePolicy`], which is the caller's reaction rather than the script's permissions.

pub mod grant_set;
pub mod language_surface;
pub mod policy;
pub mod resource_limits;

#[doc(inline)]
pub use grant_set::GrantSet;
#[doc(inline)]
pub use language_surface::LanguageSurface;
#[doc(inline)]
pub use policy::Policy;
#[doc(inline)]
pub use resource_limits::{InstructionLimit, MemoryLimit, ResourceLimits};
