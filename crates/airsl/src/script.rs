//! A unit of Lua source the engine can run.
//!
//! Its own type because a script is more than its text: it carries the name errors are reported
//! under, and the directory that `require` is confined to. Bundling the three means the engine
//! never has to guess a chunk name or infer a root, and a script loaded from memory behaves the
//! same as one loaded from disk.
//!
//! Responsibilities: [`Script`], its constructors, and access to source, name and root.
//!
//! Non-responsibilities: execution. [`crate::Engine`] runs the script; [`Script::root`] only
//! records where `require` is permitted to look.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::types::ChunkName;

/// Lua source together with the name it reports and the directory it may `require` from.
#[derive(Debug, Clone)]
pub struct Script {
    source: String,
    name: ChunkName,
    root: Option<PathBuf>,
}

impl Script {
    /// Builds a script from source text held in memory.
    ///
    /// The result has no root, so `require` is unavailable to it. Use [`Script::from_file`] for a
    /// script that loads siblings.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] when `name` is not a valid [`ChunkName`].
    pub fn from_source(source: impl Into<String>, name: impl Into<String>) -> Result<Self> {
        Ok(Self {
            source: source.into(),
            name: ChunkName::new(name)?,
            root: None,
        })
    }

    /// Reads a script from `path`.
    ///
    /// The chunk name is the path as given, and the root is the path's parent directory — so a
    /// script may `require` its siblings but nothing above them.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ScriptRead`] when the file cannot be read.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let source = std::fs::read_to_string(path).map_err(|source| Error::ScriptRead {
            path: path.display().to_string(),
            source,
        })?;
        let root = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf);
        Ok(Self {
            source,
            name: ChunkName::from_path(path),
            root,
        })
    }

    /// Confines `require` to `root` instead of the directory the script was read from.
    #[must_use]
    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = Some(root.into());
        self
    }

    /// The Lua source text.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The name errors and tracebacks report this script under.
    #[must_use]
    pub const fn name(&self) -> &ChunkName {
        &self.name
    }

    /// The directory `require` may load from, if any.
    #[must_use]
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Script;
    use std::io::Write as _;

    #[test]
    fn a_source_script_keeps_its_text_and_name() {
        let script = Script::from_source("return 1", "inline").unwrap();
        assert_eq!(script.source(), "return 1");
        assert_eq!(script.name().as_str(), "inline");
    }

    #[test]
    fn a_source_script_has_no_require_root() {
        assert!(
            Script::from_source("return 1", "inline")
                .unwrap()
                .root()
                .is_none()
        );
    }

    #[test]
    fn an_invalid_chunk_name_is_rejected() {
        assert!(Script::from_source("return 1", "").is_err());
    }

    #[test]
    fn a_file_script_reads_its_source_and_roots_at_the_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hook.lua");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "return 42").unwrap();
        drop(file);

        let script = Script::from_file(&path).unwrap();
        assert_eq!(script.source().trim(), "return 42");
        assert_eq!(script.name().as_str(), path.display().to_string());
        assert_eq!(script.root(), Some(dir.path()));
    }

    #[test]
    fn a_missing_file_reports_the_path_it_tried() {
        let err = Script::from_file("/nonexistent/hook.lua").unwrap_err();
        assert!(err.to_string().contains("/nonexistent/hook.lua"), "{err}");
    }

    #[test]
    fn with_root_overrides_the_inferred_directory() {
        let script = Script::from_source("return 1", "inline")
            .unwrap()
            .with_root("/plugins");
        assert_eq!(script.root(), Some(std::path::Path::new("/plugins")));
    }
}
