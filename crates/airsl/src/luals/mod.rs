//! Everything specific to the Lua language server: checking that a host module's description
//! matches what it installs, and rendering the descriptions as a type file.
//!
//! Its own layer, between `modules` and a host, because the checks here talk about a module from
//! the outside. They run a module against a scratch Lua state and compare the result with the
//! description it gave, which neither the module nor the engine needs to do while a script runs.
//!
//! Responsibilities:
//!
//! - [`verify`] and [`Drift`] — install one module into a scratch state and compare its keys with
//!   its description.
//! - [`TypeFile`] — the text of the type file for a set of modules, and the name it belongs under.
//! - [`setup`], [`SetupReport`], [`FileReport`] and [`FileChange`] — write the type file and list
//!   its directory in a project's luarc, checking every module first and writing nothing when a check
//!   fails.
//!
//! Non-responsibilities: running scripts, and describing a module. A module describes itself
//! through [`crate::HostModule::describe`].

mod luarc;
mod render;
mod setup;
mod verify;

#[doc(inline)]
pub use render::TypeFile;
#[doc(inline)]
pub use setup::{FileChange, FileReport, SetupReport, setup};
#[doc(inline)]
pub use verify::{Drift, verify};
