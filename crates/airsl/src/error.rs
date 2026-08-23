//! Error type every fallible `airsl` operation returns.
//!
//! Exists as its own module so the crate has a single error vocabulary spanning three very
//! different failure sources — the Lua VM, the host modules that call into the operating system,
//! and the sandbox policy that refuses a script before it runs. Callers match on one enum instead
//! of unwrapping [`mlua::Error`] alongside [`std::io::Error`].
//!
//! Responsibilities:
//!
//! - [`Error`], the crate-wide error enum, and the [`Result`] alias built on it.
//! - Conversion from [`mlua::Error`], so `?` works across the Lua boundary.
//!
//! Non-responsibilities: this module does not decide what happens *after* a failure. Whether an
//! error is reported or swallowed is [`crate::FailurePolicy`]'s job, applied by the caller.

/// Convenience alias for results carrying an [`Error`].
pub type Result<T> = core::result::Result<T, Error>;

/// Everything that can go wrong loading, configuring, or running a Lua script.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The Lua VM rejected the chunk or raised during execution.
    #[error("lua error in {chunk}: {source}")]
    Lua {
        /// Chunk name the failure was attributed to, as it appears in Lua tracebacks.
        chunk: String,
        /// The underlying VM error.
        #[source]
        source: Box<mlua::Error>,
    },

    /// The Lua state could not be created or configured before any script ran.
    #[error("cannot configure the Lua state ({stage}): {source}")]
    EngineSetup {
        /// Which part of construction failed, for example `memory limit`.
        stage: &'static str,
        /// The underlying VM failure.
        #[source]
        source: Box<mlua::Error>,
    },

    /// A script allocated past the memory ceiling its policy set.
    ///
    /// Distinct from [`Error::Lua`] because a script stopped for consuming the host's memory is an
    /// operational event rather than a defect in the script, and a caller that discards ordinary
    /// script failures usually still wants to hear about this one.
    #[error("script `{chunk}` exceeded its memory limit of {limit} bytes")]
    MemoryLimit {
        /// Chunk name the failure was attributed to.
        chunk: String,
        /// The ceiling that was passed, in bytes.
        limit: usize,
        /// The underlying VM error.
        #[source]
        source: Box<mlua::Error>,
    },

    /// A script executed past the instruction ceiling its policy set.
    ///
    /// Ordinarily means the script did not terminate. As with [`Error::MemoryLimit`], this says
    /// something about the host's resources rather than about the script's logic.
    #[error("script `{chunk}` exceeded its instruction limit of {limit}")]
    InstructionLimit {
        /// Chunk name the failure was attributed to.
        chunk: String,
        /// The ceiling that was passed.
        limit: u64,
    },

    /// A host module could not be installed into the Lua state.
    #[error("failed to install host module `{module}`: {reason}")]
    ModuleInstall {
        /// Name of the module whose installation failed.
        module: String,
        /// Why the installation failed.
        reason: String,
    },

    /// Two host modules claimed the same name.
    #[error("host module `{module}` is already registered")]
    DuplicateModule {
        /// The name that was registered twice.
        module: String,
    },

    /// A module was asked to be replaced in a set that does not contain it.
    #[error("host module `{module}` is not registered")]
    ModuleNotFound {
        /// The name that was looked up.
        module: String,
    },

    /// `ext.on` was called with an event the host never declared.
    ///
    /// Raised while the extension's entry script runs, so it surfaces as a load failure rather
    /// than as a handler that silently never fires.
    #[error(
        "event `{event}` is not one this host dispatches; {}",
        describe_declared(declared)
    )]
    UnknownEvent {
        /// The name the script asked for.
        event: String,
        /// Every event the host did declare, so the refusal names what *was* available.
        declared: Vec<String>,
    },

    /// An evaluation was attempted from the thread already evaluating on this engine.
    ///
    /// Covers every way that can happen: a handler calling a host function that dispatches back
    /// into the same engine, and — beyond `dispatch` — a host function that calls [`crate::Engine::eval`]
    /// or [`crate::Engine::check`] on an engine it is already running on, whether or not the outer
    /// call was itself a dispatched event. `event` names the handler that was running when the
    /// re-entrant call happened; `None` when the outer call was a plain `eval`/`check` rather than
    /// a dispatch. The alternative in every case is a deadlock on the evaluation lock, which this
    /// variant exists to avoid.
    #[error("{}", describe_reentrant(event.as_deref()))]
    Reentrant {
        /// The event whose handler was running when the re-entrant call was refused, if any.
        event: Option<String>,
    },

    /// `extension.toml` could not be read.
    #[error("cannot read manifest `{path}`: {source}")]
    ManifestRead {
        /// The manifest path.
        path: String,
        /// The underlying failure.
        #[source]
        source: std::io::Error,
    },

    /// `extension.toml` is not valid TOML or not the expected shape.
    #[error("cannot parse manifest `{path}`: {reason}")]
    ManifestParse {
        /// The manifest path.
        path: String,
        /// The parser's message, verbatim.
        reason: String,
    },

    /// A manifest field parsed but violates a rule the parser cannot express.
    #[error("invalid manifest field `{field}`: {reason}")]
    ManifestInvalid {
        /// Dotted path of the field, such as `capabilities.fs.read`, or the block name (for
        /// example `capabilities`) when the offending key is itself runtime data.
        field: &'static str,
        /// What was wrong with it.
        reason: String,
    },

    /// A manifest path refers to a `$VAR` the host did not supply.
    #[error("manifest refers to `${name}`, which the host did not supply")]
    ManifestVariable {
        /// The variable name, without the `$`.
        name: String,
    },

    /// The manifest pins an api version this runtime does not implement.
    #[error(
        "manifest declares api {requested}; this runtime supports api {}",
        join_versions(supported)
    )]
    UnsupportedApi {
        /// What the manifest asked for.
        requested: u32,
        /// What this runtime implements.
        supported: Vec<u32>,
    },

    /// A policy offered as a ceiling does not bound anything on one of its axes.
    #[error("the ceiling is not a bound: {reason}")]
    CeilingUnbounded {
        /// Which axis is unbounded.
        reason: &'static str,
    },

    /// An extension asked for something the host would not give it, or the approver refused.
    #[error("extension `{extension}` was not loaded: {detail}")]
    ExtensionDenied {
        /// The extension's declared name.
        extension: String,
        /// Every denial, or the approver's reason.
        detail: String,
    },

    /// A name did not satisfy the rules for its kind.
    #[error("invalid {kind} `{value}`: {reason}")]
    InvalidName {
        /// What sort of name was being validated, for example `module name`.
        kind: &'static str,
        /// The rejected input.
        value: String,
        /// Why it was rejected.
        reason: &'static str,
    },

    /// A script file could not be read.
    #[error("cannot read script `{path}`: {source}")]
    ScriptRead {
        /// Path that could not be read.
        path: String,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },

    /// A `require` resolved outside the directory the script is allowed to load from.
    #[error("module `{module}` resolves outside the script directory `{root}`")]
    RequireEscape {
        /// The requested module name.
        module: String,
        /// The directory `require` is confined to.
        root: String,
    },

    /// A `require` formed a loop, directly or through a chain of modules.
    #[error("module `{module}` requires itself, directly or indirectly, under `{root}`")]
    RequireCycle {
        /// The module that was required while it was still loading.
        module: String,
        /// The directory `require` is confined to.
        root: String,
    },

    /// A host module refused an operation the policy does not grant.
    ///
    /// Names the module, the operation and what was refused, because "permission denied" without
    /// those three is indistinguishable from the operating system's own refusal and sends whoever
    /// reads it looking at file modes instead of at the policy.
    #[error("{module}.{operation} denied: {detail}")]
    Denied {
        /// The module that refused, for example `fs`.
        module: &'static str,
        /// The function that refused, for example `read`.
        operation: &'static str,
        /// What was refused and why.
        detail: String,
    },

    /// An operating-system call made on a script's behalf failed.
    #[error("{operation} failed on `{path}`: {source}")]
    Io {
        /// What was being attempted, for example `read`.
        operation: &'static str,
        /// The path it was attempted on.
        path: String,
        /// The underlying failure.
        #[source]
        source: std::io::Error,
    },

    /// A path could not be resolved to something a grant can be checked against.
    ///
    /// Separate from [`Error::Denied`]: the policy did not refuse this, the path could not be
    /// given a meaning to refuse. A `..` that climbs through a directory which does not exist has
    /// no filesystem answer, and guessing one lexically is how a containment check gets bypassed.
    #[error("cannot resolve `{path}` to check it against the policy: {reason}")]
    UncheckablePath {
        /// The path as the script wrote it.
        path: String,
        /// Why no answer could be given.
        reason: &'static str,
    },

    /// A path could not be expressed relative to the base it was measured against.
    ///
    /// Its own variant rather than a generic message because the caller usually wants to fall back
    /// — reporting the absolute path, say — rather than to abort, and distinguishing "not under
    /// this root" from "the working directory is unreadable" is what makes that possible.
    #[error("path `{path}` is outside `{base}`")]
    PathNotRelative {
        /// The path that was being made relative.
        path: String,
        /// The base it was measured against.
        base: String,
    },

    /// A path could not be resolved against the process working directory.
    #[error("cannot resolve path `{path}`: {source}")]
    PathResolution {
        /// The path that could not be resolved.
        path: String,
        /// The underlying I/O failure, normally an unreadable working directory.
        #[source]
        source: std::io::Error,
    },

    /// A `require` target does not exist under the script directory.
    #[error("module `{module}` not found under `{root}`")]
    RequireNotFound {
        /// The requested module name.
        module: String,
        /// The directory that was searched.
        root: String,
    },
}

/// The tail of an [`Error::UnknownEvent`] message.
fn describe_declared(declared: &[String]) -> String {
    if declared.is_empty() {
        String::from("this runtime dispatches no events")
    } else {
        format!("declared events: {}", declared.join(", "))
    }
}

/// The message for an [`Error::Reentrant`].
fn describe_reentrant(event: Option<&str>) -> String {
    event.map_or_else(
        || String::from("re-entrant evaluation"),
        |event| format!("re-entrant call during handler for event `{event}`"),
    )
}

/// Comma-joined api versions for an [`Error::UnsupportedApi`] message.
fn join_versions(versions: &[u32]) -> String {
    versions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Which resource ceiling a script exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExhaustedLimit {
    /// The memory ceiling.
    Memory,
    /// The instruction ceiling.
    Instructions,
}

impl Error {
    /// Wraps an [`mlua::Error`] with the chunk name it came from.
    #[must_use]
    pub fn lua(chunk: impl Into<String>, source: mlua::Error) -> Self {
        Self::Lua {
            chunk: chunk.into(),
            source: Box::new(source),
        }
    }

    /// Which ceiling this failure exhausted, if it exhausted one.
    ///
    /// Lets a caller that discards ordinary script failures still surface a resource breach, which
    /// is a fact about the host rather than a diagnostic the script chose to emit.
    #[must_use]
    pub const fn exhausted_limit(&self) -> Option<ExhaustedLimit> {
        match self {
            Self::MemoryLimit { .. } => Some(ExhaustedLimit::Memory),
            Self::InstructionLimit { .. } => Some(ExhaustedLimit::Instructions),
            _ => None,
        }
    }
}

impl From<Error> for mlua::Error {
    fn from(value: Error) -> Self {
        Self::external(value)
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn lua_wraps_chunk_name_into_the_message() {
        let err = Error::lua("enforce.lua", mlua::Error::RuntimeError("boom".into()));
        assert!(err.to_string().starts_with("lua error in enforce.lua: "));
    }

    #[test]
    fn require_escape_names_both_module_and_root() {
        let err = Error::RequireEscape {
            module: "../secrets".into(),
            root: "/scripts".into(),
        };
        assert_eq!(
            err.to_string(),
            "module `../secrets` resolves outside the script directory `/scripts`"
        );
    }

    #[test]
    fn converting_into_an_mlua_error_preserves_the_message() {
        let err = Error::DuplicateModule {
            module: "fs".into(),
        };
        let text = err.to_string();
        let lua: mlua::Error = err.into();
        assert!(lua.to_string().contains(&text));
    }

    #[test]
    fn module_not_found_names_the_module() {
        let err = Error::ModuleNotFound {
            module: "ext".into(),
        };
        assert_eq!(err.to_string(), "host module `ext` is not registered");
    }

    #[test]
    fn unknown_event_names_what_was_declared() {
        let err = Error::UnknownEvent {
            event: "tpyo".into(),
            declared: vec!["note_saved".into(), "query".into()],
        };
        assert_eq!(
            err.to_string(),
            "event `tpyo` is not one this host dispatches; declared events: note_saved, query"
        );
    }

    #[test]
    fn unknown_event_with_nothing_declared_says_so() {
        let err = Error::UnknownEvent {
            event: "x".into(),
            declared: Vec::new(),
        };
        assert_eq!(
            err.to_string(),
            "event `x` is not one this host dispatches; this runtime dispatches no events"
        );
    }

    #[test]
    fn reentrant_names_the_handler_s_event_when_one_is_running() {
        let err = Error::Reentrant {
            event: Some("query".into()),
        };
        assert_eq!(
            err.to_string(),
            "re-entrant call during handler for event `query`"
        );
    }

    #[test]
    fn reentrant_with_no_event_describes_a_plain_re_entrant_evaluation() {
        // `eval`/`check` nested inside `eval`/`check` on the same engine has no event of its own
        // to name — the re-entrancy is real, but there was never a handler running.
        let err = Error::Reentrant { event: None };
        assert_eq!(err.to_string(), "re-entrant evaluation");
    }

    #[test]
    fn manifest_invalid_names_the_field() {
        let err = Error::ManifestInvalid {
            field: "capabilities.fs.read",
            reason: "`journal` is not absolute after expansion".into(),
        };
        assert_eq!(
            err.to_string(),
            "invalid manifest field `capabilities.fs.read`: `journal` is not absolute after expansion"
        );
    }

    #[test]
    fn manifest_variable_names_the_variable() {
        let err = Error::ManifestVariable {
            name: "APP_HOME".into(),
        };
        assert_eq!(
            err.to_string(),
            "manifest refers to `$APP_HOME`, which the host did not supply"
        );
    }

    #[test]
    fn unsupported_api_lists_the_supported_set() {
        let err = Error::UnsupportedApi {
            requested: 7,
            supported: vec![1],
        };
        assert_eq!(
            err.to_string(),
            "manifest declares api 7; this runtime supports api 1"
        );
    }

    #[test]
    fn ceiling_unbounded_states_the_reason() {
        let err = Error::CeilingUnbounded {
            reason: "grants are unrestricted",
        };
        assert_eq!(
            err.to_string(),
            "the ceiling is not a bound: grants are unrestricted"
        );
    }

    #[test]
    fn extension_denied_names_the_extension_and_the_detail() {
        let err = Error::ExtensionDenied {
            extension: "journal-indexer".into(),
            detail: "fs.read `/` is outside the ceiling".into(),
        };
        assert_eq!(
            err.to_string(),
            "extension `journal-indexer` was not loaded: fs.read `/` is outside the ceiling"
        );
    }
}
