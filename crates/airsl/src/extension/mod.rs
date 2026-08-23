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
//! - [`ceiling`] — the host's maximum, validated to be a bound.
//! - [`mod@negotiate`] — the intersection of a request with a ceiling.
//! - [`approver`] — who decides whether a negotiated extension loads.
//! - [`load_report`] — the result shapes `load_dir` and `broadcast` hand back without
//!   short-circuiting on the first failure.
//! - [`loaded`] — [`Extension`], the loaded unit, and the type-state ([`Approved`]) that proves a
//!   denial ran before any extension code does.
//! - [`host`] — [`ExtensionHost`], the registry that loads a whole directory of extensions and
//!   fans events out to every one it holds.
//!
//! Non-responsibilities: running anything. The engine does that.

pub mod api_version;
pub mod approver;
pub mod ceiling;
pub mod host;
pub mod load_report;
pub mod loaded;
pub mod manifest;
pub mod memory_size;
pub mod negotiate;
pub mod variables;

#[doc(inline)]
pub use api_version::{ApiVersion, SUPPORTED_API};
#[doc(inline)]
pub use approver::{ApprovalRequest, Approver, Decision, DenyAll, ManifestApprover};
#[doc(inline)]
pub use ceiling::Ceiling;
#[doc(inline)]
pub use host::{ExtensionHost, HostBuilder, ModuleFactory, NoCeiling, Stdlib, WithCeiling};
#[doc(inline)]
pub use load_report::{Dispatch, LoadReport};
#[doc(inline)]
pub use loaded::{Approved, Extension, LoadContext};
#[doc(inline)]
pub use manifest::{CapabilityRequest, LimitRequest, MANIFEST_FILE, Manifest, RawManifest};
#[doc(inline)]
pub use memory_size::parse_memory_size;
#[doc(inline)]
pub use negotiate::{Capability, Denial, Negotiation, Reduction, negotiate};
#[doc(inline)]
pub use variables::Variables;
