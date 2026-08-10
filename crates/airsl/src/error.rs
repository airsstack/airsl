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
            root: "/plugins".into(),
        };
        assert_eq!(
            err.to_string(),
            "module `../secrets` resolves outside the script directory `/plugins`"
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
}
