//! Embeddable Lua runtime with a host standard library.
//!
//! `airsl` runs sandboxed Lua from Rust and gives those scripts the capabilities a shell or Python
//! script would otherwise reach for — JSON, filesystem access, subprocesses, real regular
//! expressions — through host modules implemented in Rust. Everything a script can do arrives
//! under a single `airsstack` global, so the host decides the surface rather than the Lua
//! standard library.
//!
//! ```no_run
//! use airsl::{Engine, FailurePolicy, Sandbox, Script};
//!
//! let engine = Engine::builder().sandbox(Sandbox::Restricted).build()?;
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

#![forbid(unsafe_code)]

mod builder;
mod convert;
mod engine;
mod error;
mod policy;
mod script;

pub mod modules;
pub mod types;

pub use builder::{EngineBuilder, Missing, Present};
pub use engine::{Engine, ROOT_TABLE};
pub use error::{Error, Result};
pub use modules::{HostModule, ModuleSet};
pub use policy::{FailurePolicy, Sandbox};
pub use script::Script;
pub use types::{ChunkName, ModuleName};
