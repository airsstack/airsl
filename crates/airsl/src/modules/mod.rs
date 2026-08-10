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
//! - [`registry`] — the [`HostModule`] trait, [`InstallContext`] and [`ModuleSet`].
//! - [`mod@env`] — environment variables, guarded by a name allowlist.
//! - [`mod@fs`] — filesystem access, guarded by the policy's filesystem grants.
//! - [`mod@proc`] — subprocesses, guarded by an executable allowlist.
//! - [`mod@regex`] — real regular expressions, needing no authority.
//! - [`mod@hash`] — SHA-256 and SHA-1; `hash_file` inherits the filesystem read grants.
//! - [`mod@time`] — timestamps and formatting, needing no authority.
//! - [`mod@glob`] — glob matching; `walk` inherits the filesystem read grants.
//! - [`mod@stdio`] — the process's own standard streams.
//! - [`mod@hook`] — the agent-hook contract, over `stdio` and `json`.
//! - [`json`] — JSON encoding and decoding.
//! - [`mod@path`] — path manipulation, needing no authority.
//! - [`mod@stdlib`] — the default module set the engine installs.
//!
//! Non-responsibilities: sandboxing. Which Lua standard libraries a script sees is
//! [`crate::Policy`]'s decision, applied by the engine before any of these are installed.

mod guard;

pub mod env;
pub mod fs;
pub mod glob;
pub mod hash;
pub mod hook;
pub mod json;
pub mod path;
pub mod proc;
pub mod regex;
pub mod registry;
pub mod stdio;
pub mod stdlib;
pub mod time;

pub use env::Env;
pub use fs::Fs;
pub use glob::Glob;
pub use hash::Hash;
pub use hook::Hook;
pub use json::Json;
pub use path::Path;
pub use proc::Proc;
pub use regex::Regex;
pub use registry::{HostModule, InstallContext, ModuleSet};
pub use stdio::Stdio;
pub use stdlib::stdlib;
pub use time::Time;
