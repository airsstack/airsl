//! Embeddable Lua runtime with a host standard library.
//!
//! `airsl` runs sandboxed Lua from Rust and gives those scripts the capabilities a shell or Python
//! script would otherwise reach for — JSON, filesystem access, subprocesses, real regular
//! expressions — through host modules implemented in Rust. Everything a script can do arrives
//! under a single `airsstack` global, so the host decides the surface rather than the Lua
//! standard library.
//!
//! ```no_run
//! use airsl::{Engine, FailurePolicy, Policy, Script};
//!
//! let engine = Engine::builder().policy(Policy::confined()).build()?;
//! let script = Script::from_file("hooks/enforce.lua")?;
//!
//! if let Err(error) = engine.eval(&script) {
//!     // A hook must never block the tool call that triggered it.
//!     if !FailurePolicy::FailOpen.swallows_errors() {
//!         eprintln!("{error}");
//!     }
//! }
//! # Ok::<(), airsl::Error>(())
//! ```
//!
//! Extend the surface by implementing [`HostModule`] and adding it to a [`ModuleSet`]; the module
//! becomes a subtable of `airsstack` alongside the built-ins.
//!
//! Third-party scripts load as [`extension`]s: a directory with its own `extension.toml` manifest,
//! negotiated against the host's ceiling and run through an [`ExtensionHost`] that owns the
//! resulting engines.

#![forbid(unsafe_code)]

// Unix only, and deliberately loud about it. `modules::proc` decides whether a program is
// executable from the mode bits, and the sandbox tests build symlinks through `std::os::unix::fs`.
// Neither has a meaning on Windows, where executability is decided by the file extension and
// `PATHEXT`. Supporting it is a decision about what "executable" should mean there, not a
// portability patch — so the crate refuses to build with a sentence a reader can act on, instead of
// nine resolution errors pointing at std.
#[cfg(not(any(unix, windows)))]
compile_error!(
    "airsl supports unix and Windows targets only: the host modules assume one of those two \
     platform families and this target is neither."
);

mod builder;
mod convert;
mod engine;
mod error;
mod failure_policy;
mod instruction_budget;
mod require_loader;
mod script;

pub mod extension;
pub mod modules;
pub mod sandbox;
pub mod types;

/// The Lua binding this crate is built on.
///
/// Re-exported because it is part of the public contract rather than an implementation detail:
/// [`HostModule::install`] receives an `&mlua::Lua` and an `&mlua::Table`, so a module cannot be
/// written without naming these types. Depending on `airsl::mlua` rather than declaring `mlua`
/// separately keeps a contributor on the same version the engine was built with — a mismatch
/// otherwise produces type errors that never mention the real cause.
pub use mlua;

pub use builder::{EngineBuilder, Missing, Present};
pub use engine::Engine;
pub use error::{Error, ExhaustedLimit, Result};
pub use extension::{Extension, ExtensionHost};
pub use failure_policy::FailurePolicy;
pub use modules::{HostModule, InstallContext, ModuleSet};
pub use sandbox::{
    EnvGrant, FsGrant, GrantSet, InstructionLimit, LanguageSurface, MemoryLimit, Policy, ProcGrant,
    ResourceLimits,
};
pub use script::Script;
pub use types::{ChunkName, EventName, ExtensionName, ModuleName, RequireTarget, RootTable};
