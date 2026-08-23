//! Validated newtypes for the identifiers that cross into the Lua VM.
//!
//! Grouped here because they share a single motivation: a raw `String` reaching the VM changes
//! behaviour in ways the compiler cannot see — a module name becomes a table key that may not be
//! reachable with dot syntax, a root table name becomes a global that may shadow the standard
//! library, and a chunk name is reinterpreted by Lua according to its first byte. Parsing them at
//! construction removes that class of surprise from every call site.
//!
//! Responsibilities:
//!
//! - [`ModuleName`] — the key a host module is installed under in the root table.
//! - [`RootTable`] — the name of the single global those modules are installed under.
//! - [`RequireTarget`] — a module name a confined script may pass to `require`.
//! - [`ChunkName`] — the name a compiled chunk reports in errors and tracebacks.
//! - [`EventName`] — a host event an extension may subscribe to and the engine may dispatch.
//!
//! Non-responsibilities: neither type touches the VM. They are plain values the engine consumes.

pub mod chunk_name;
pub mod event_name;
pub mod module_name;
pub mod require_target;
pub mod root_table;

pub use chunk_name::ChunkName;
pub use event_name::EventName;
pub use module_name::ModuleName;
pub use require_target::RequireTarget;
pub use root_table::RootTable;
