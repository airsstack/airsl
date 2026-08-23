//! Loading third-party Lua extensions with negotiated capabilities.
//!
//! Exists as its own tree because everything here happens *before* a Lua state exists: a
//! manifest is read, its request is bounded by the host's ceiling, an approver decides, and only
//! then is an ordinary [`crate::Policy`] handed to the engine. Nothing in this tree enforces a
//! grant — enforcement stays in the host modules, where it already is.
//!
//! Responsibilities:
//!
//! - [`manifest`] — the `extension.toml` format and its validated form.
//! - [`api_version`] — the api number a manifest pins and the set this crate supports.
//! - [`memory_size`] — the `"64MB"` spelling of a memory ceiling.
//! - [`variables`] — the `$VAR` values a host supplies for manifest paths.
//!
//! Non-responsibilities: running anything. The engine does that.

pub mod api_version;
pub mod manifest;
pub mod memory_size;
pub mod variables;

#[doc(inline)]
pub use api_version::{ApiVersion, SUPPORTED_API};
#[doc(inline)]
pub use manifest::{CapabilityRequest, LimitRequest, MANIFEST_FILE, Manifest, RawManifest};
#[doc(inline)]
pub use memory_size::parse_memory_size;
#[doc(inline)]
pub use variables::Variables;
