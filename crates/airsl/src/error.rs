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

    /// A `require` target does not exist under the script directory.
    #[error("module `{module}` not found under `{root}`")]
    RequireNotFound {
        /// The requested module name.
        module: String,
        /// The directory that was searched.
        root: String,
    },
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
