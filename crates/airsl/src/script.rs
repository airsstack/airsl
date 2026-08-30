//! A unit of Lua source the engine can run.
//!
//! Its own type because a script is more than its text: it carries the name errors are reported
//! under, the directory that `require` is confined to, and the arguments it was invoked with.
//! Bundling them means the engine never has to guess a chunk name or infer a root, and a script
//! loaded from memory behaves the same as one loaded from disk.
//!
//! Responsibilities: [`Script`], its constructors, and access to source, name, root and arguments.
//!
//! Non-responsibilities: execution. [`crate::Engine`] runs the script; [`Script::root`] only
//! records where `require` is permitted to look.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::paths::rules::native::to_script_string;
use crate::types::ChunkName;

/// Lua source together with the name it reports, the directory it may `require` from, and the
/// arguments it was invoked with.
#[derive(Debug, Clone)]
pub struct Script {
    source: String,
    name: ChunkName,
    root: Option<PathBuf>,
    args: Vec<String>,
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
            args: Vec::new(),
        })
    }

    /// Reads a script from `path`.
    ///
    /// The chunk name is the path as given, and the root is the path's parent directory — so a
    /// script may `require` its siblings but nothing above them.
    ///
    /// Naming it after the path is right for a script someone just pointed at, and wrong for one
    /// whose diagnostics leave the machine: the chunk name reaches every traceback this script
    /// produces, so an absolute path travels with them. Use [`Script::with_name`] where that
    /// matters.
    ///
    /// A bare filename has an empty parent, which means the current directory rather than no
    /// directory. Reading it as the latter is why `airsl run main.lua` and `airsl run ./main.lua`
    /// used to differ: the same file got `require` under one spelling and not the other.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ScriptRead`] when the file cannot be read.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let source = std::fs::read_to_string(path).map_err(|source| Error::ScriptRead {
            path: to_script_string(path),
            source,
        })?;
        let root = Some(require_root(path));
        Ok(Self {
            source,
            name: ChunkName::from_path(path),
            root,
            args: Vec::new(),
        })
    }

    /// Confines `require` to `root` instead of the directory the script was read from.
    #[must_use]
    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = Some(root.into());
        self
    }

    /// Reports the script under `name` rather than the name it was constructed with.
    ///
    /// Exists for the case [`Script::from_file`] cannot serve on its own. A chunk name is not
    /// internal bookkeeping: it appears in every traceback and every [`Error::Lua`] the script
    /// raises, and a host that forwards those anywhere — a hook writing to stderr, a service
    /// returning an error to a caller — forwards the path with them. Reading the file separately
    /// to reach [`Script::from_source`] works, but gives up the [`Error::ScriptRead`] diagnostic
    /// and the inferred root; this keeps both and changes only the label.
    ///
    /// Returning a [`Result`] rather than `Self` is the same trade [`Script::from_source`] makes:
    /// the name is validated once, here, so nothing downstream has to cope with one that would
    /// corrupt the traceback it lands in.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] when `name` is empty, longer than 240 bytes, or contains a
    /// newline or NUL.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), airsl::Error> {
    /// # let dir = tempfile::tempdir().expect("temp dir");
    /// # let path = dir.path().join("enforce.lua");
    /// # std::fs::write(&path, "return 1").expect("write");
    /// let script = airsl::Script::from_file(&path)?.with_name("enforce.lua")?;
    ///
    /// // The label travels; the root the file was read from does not change.
    /// assert_eq!(script.name().as_str(), "enforce.lua");
    /// assert_eq!(script.root(), Some(dir.path()));
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_name(mut self, name: impl Into<String>) -> Result<Self> {
        self.name = ChunkName::new(name)?;
        Ok(self)
    }

    /// Supplies the arguments the script sees in Lua's global `arg` table.
    ///
    /// Lua's own convention for a standalone script, so a ported shell script reads `arg[1]` where
    /// it previously read `$1`. Carrying them on the script rather than installing them into the
    /// state means a host does not need the raw VM to pass an argument, and an engine running two
    /// scripts gives each the arguments it was built with.
    #[must_use]
    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
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

    /// The arguments this script reports in the global `arg` table.
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }
}

/// The directory a script read from `path` may `require` from.
///
/// A bare filename's parent is the empty path, which denotes the current directory rather than no
/// directory at all. Treating it as the latter meant `main.lua` and `./main.lua` named the same
/// file but only one of them got `require`.
fn require_root(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{Script, require_root};
    use std::io::Write as _;
    use std::path::Path;

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
    fn a_script_has_no_arguments_unless_given_some() {
        assert!(
            Script::from_source("return 1", "inline")
                .unwrap()
                .args()
                .is_empty()
        );
    }

    #[test]
    fn with_args_records_the_arguments_in_order() {
        let script = Script::from_source("return 1", "inline")
            .unwrap()
            .with_args(["one", "two"]);
        assert_eq!(script.args(), ["one", "two"]);
    }

    #[test]
    fn with_args_replaces_rather_than_appends() {
        let script = Script::from_source("return 1", "inline")
            .unwrap()
            .with_args(["one"])
            .with_args(["two", "three"]);
        assert_eq!(script.args(), ["two", "three"]);
    }

    #[test]
    fn with_root_overrides_the_inferred_directory() {
        let script = Script::from_source("return 1", "inline")
            .unwrap()
            .with_root("/scripts");
        assert_eq!(script.root(), Some(Path::new("/scripts")));
    }

    #[test]
    fn with_name_replaces_the_name_inferred_from_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("enforce.lua");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "return 42").unwrap();
        drop(file);

        let script = Script::from_file(&path).unwrap().with_name("hook").unwrap();
        assert_eq!(script.name().as_str(), "hook");
        assert!(
            !script
                .name()
                .as_str()
                .contains(&dir.path().display().to_string()),
            "the absolute path should not survive into the traceback name: {}",
            script.name()
        );
    }

    #[test]
    fn with_name_leaves_the_source_and_the_require_root_alone() {
        // Renaming is about what diagnostics say, not about what the script may reach.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("enforce.lua");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "return 42").unwrap();
        drop(file);

        let script = Script::from_file(&path).unwrap().with_name("hook").unwrap();
        assert_eq!(script.source().trim(), "return 42");
        assert_eq!(script.root(), Some(dir.path()));
    }

    #[test]
    fn with_name_rejects_a_name_that_would_corrupt_a_traceback() {
        let script = Script::from_source("return 1", "inline").unwrap();
        assert!(script.clone().with_name("").is_err());
        assert!(script.with_name("two\nlines").is_err());
    }

    #[test]
    fn a_bare_filename_roots_at_the_current_directory_like_its_dotted_spelling() {
        // Two spellings of the same file used to differ: `main.lua` got no root and therefore no
        // `require`, while `./main.lua` got one.
        assert_eq!(require_root(Path::new("main.lua")), Path::new("."));
        assert_eq!(require_root(Path::new("./main.lua")), Path::new("."));
    }

    #[test]
    fn a_path_with_directories_roots_at_its_parent() {
        assert_eq!(
            require_root(Path::new("/scripts/hooks/enforce.lua")),
            Path::new("/scripts/hooks")
        );
        assert_eq!(require_root(Path::new("/main.lua")), Path::new("/"));
    }
}
