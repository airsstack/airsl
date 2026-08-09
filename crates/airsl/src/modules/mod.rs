//! The `airsstack` Lua standard library, and the seam for extending it.
//!
//! Exists as a module tree so each capability the plugin scripts need is one file with its own
//! tests, rather than one large surface. Every submodule here implements [`HostModule`] and is
//! installed as a subtable of the single `airsstack` global — `airsstack.json.decode`,
//! `airsstack.fs.read`. Downstream crates add their own the same way, which is what makes this
//! crate a shared Lua integration point instead of a fixed runner.
//!
//! Responsibilities:
//!
//! - [`registry`] — the [`HostModule`] trait and [`ModuleSet`].
//! - [`json`] — JSON encoding and decoding.
//! - [`mod@stdlib`] — the default module set the engine installs.
//!
//! Non-responsibilities: sandboxing. Which Lua standard libraries a script sees is
//! [`crate::Sandbox`]'s decision, applied by the engine before any of these are installed.

pub mod json;
pub mod registry;
pub mod stdlib;

pub use json::Json;
pub use registry::{HostModule, ModuleSet};
pub use stdlib::stdlib;
