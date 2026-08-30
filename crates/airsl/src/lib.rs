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
//!
//! Supported targets are Linux, macOS, and `x86_64-pc-windows-msvc`. Where a platform difference
//! exists it is decided in one place rather than restated here: `src/paths/rules.rs` holds the
//! lexical rules (what a path spells like, `/`- vs `\`-separated, which spellings are refused), and
//! `src/paths/containment.rs` holds the root-comparison predicate every containment check shares.

#![forbid(unsafe_code)]

mod builder;
mod convert;
mod engine;
mod error;
mod failure_policy;
mod instruction_budget;
mod paths;
mod require_loader;
mod script;
#[cfg(test)]
mod test_support;

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
pub use types::{
    ChunkName, EnvName, EventName, ExtensionName, ModuleName, RequireTarget, RootTable,
};
