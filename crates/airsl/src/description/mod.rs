//! The description a host gives of the Lua surface one of its modules exposes: the types, the
//! functions and their signatures, and the records those signatures name.
//!
//! A separate layer from `modules` because a description is data about a surface, not the surface
//! itself. A module installs behaviour into a VM; a description says what that behaviour looks
//! like, so tooling can be derived from it without running anything.
//!
//! Responsibilities:
//!
//! - [`LuaType`] — a type expression, and its spelling in the type-annotation syntax of the Lua
//!   language server.
//! - [`Function`], [`Signature`], [`Param`], [`Presence`] — a function's parameters, variadic tail
//!   and returns, and the unnamed function type a callback argument has.
//! - [`Record`], [`Field`] — a named table shape a function can return, and its fields in
//!   declaration order.
//! - [`ModuleDescription`], [`ModuleDescriptionBuilder`] — the whole description of one module, and
//!   the validation that runs when it is built.
//!
//! Non-responsibilities: rendering a file. Builder pieces hold the text they are given; only
//! `ModuleDescriptionBuilder::build` checks it.

mod lua_type;
mod module;
mod record;
mod signature;

#[doc(inline)]
pub use lua_type::LuaType;
pub(crate) use module::Member;
#[doc(inline)]
pub use module::{ModuleDescription, ModuleDescriptionBuilder};
#[doc(inline)]
pub use record::{Field, Record};
#[doc(inline)]
pub use signature::{Function, Param, Presence, Signature};
