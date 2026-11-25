//! Project setup: writes the type file and the luarc entry that lets the language server find it.
//!
//! Its own module because it is the only place in [`crate::luals`] that touches the filesystem.
//! Everything it decides is delegated to pure code (`verify`, `TypeFile::render` and the luarc
//! plan), so this file holds only the order of the steps, the resolution of each target and the
//! atomic write, and a failed check writes nothing.
//!
//! Responsibilities:
//!
//! - [`setup`], which checks every module, renders the type file, plans the luarc edit and only
//!   then writes.
//! - [`SetupReport`], [`FileReport`] and [`FileChange`], what each of the two files ended up as.
//! - Resolving a target through symlinks, refusing one that is dangling, read-only or not a regular
//!   file, and writing through a link so it survives.
//! - Writing a file by renaming a temporary file over it, with the mode a created or updated file
//!   should have.
//!
//! Non-responsibilities: describing or verifying a module, rendering the type file and deciding
//! how a luarc is edited.

use std::fmt;
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::luarc::{self, LuarcPlan};
use super::{TypeFile, verify};
use crate::error::{Error, Result};
use crate::modules::ModuleSet;
use crate::types::RootTable;

/// What setup did to one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChange {
    /// The file did not exist and was written.
    Created,
    /// The file existed and its bytes were replaced.
    Updated,
    /// The file already had the right bytes and was not touched.
    Unchanged,
}

impl fmt::Display for FileChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Unchanged => "unchanged",
        })
    }
}

/// What [`setup`] did to one file: where it is and what happened to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReport {
    path: String,
    change: FileChange,
}

impl FileReport {
    /// The file's path relative to the project, with `/` separators.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// What happened to the file.
    #[must_use]
    pub const fn change(&self) -> FileChange {
        self.change
    }
}

/// What [`setup`] did to the type file and to the luarc file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupReport {
    type_file: FileReport,
    luarc: FileReport,
}

impl SetupReport {
    /// The type file: its path relative to the project, with `/` separators, and what happened to
    /// it.
    #[must_use]
    pub const fn type_file(&self) -> &FileReport {
        &self.type_file
    }

    /// The luarc file: its path relative to the project (`.luarc.json` or `.luarc.jsonc`), and what
    /// happened to it.
    #[must_use]
    pub const fn luarc(&self) -> &FileReport {
        &self.luarc
    }
}

impl fmt::Display for SetupReport {
    /// Two lines, the type file first, with no final newline.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}\n{} {}",
            self.type_file.change, self.type_file.path, self.luarc.change, self.luarc.path
        )
    }
}

/// The directory under the project that holds the type file, as the language server is told to
/// find it.
const TYPES_DIR: &str = ".airsl/types";

/// The luarc names the language server reads, in the order it prefers them.
const LUARC_NAMES: [&str; 2] = [".luarc.json", ".luarc.jsonc"];

/// Sets `project` up for the language server: writes the type file for `modules` under
/// `.airsl/types/` and makes sure the project's luarc lists that directory.
///
/// Every module is checked against its description first, the type file is rendered and the luarc
/// edit is planned, all before anything is written, so a refusal leaves the project as it was. The
/// type file is written only when its bytes differ, and the luarc only when the entry is missing.
/// `.luarc.json` is used when it exists, then `.luarc.jsonc`, and `.luarc.json` is created
/// otherwise. A target that is a symlink is written through, and the link survives. A file is
/// written by renaming a temporary file over it: a created file gets mode `0o644` on Unix and an
/// updated one keeps its permissions.
///
/// The rename replaces the file's inode, so an updated file keeps its permissions but not its
/// owner, group, access control lists or extended attributes, and a hard link to it is left
/// pointing at the old contents. Setup does not restore any of these.
///
/// If the luarc write fails after the type file was written, the type file stays. Running setup
/// again completes the luarc and reports the type file as unchanged.
///
/// # Errors
///
/// - [`Error::Io`] with operation `open project directory` when `project` cannot be read as a
///   directory, `read` when a target cannot be inspected or read, `create directory` when
///   `.airsl/types` cannot be created, and `write` when a file cannot be written.
/// - [`Error::DescribeFailed`], the module's own `install` error, [`Error::VerifyFailed`] or
///   [`Error::DescriptionDrift`] from checking a module.
/// - [`Error::LuarcRefused`] when the luarc cannot be edited safely: it is not plain JSON, it is
///   shaped in a way a rewrite could misread, or it is a dangling symlink, not a regular file or
///   read-only.
/// - [`Error::TypeFileRefused`] when the type file's target is a dangling symlink, not a regular
///   file or read-only.
///
/// # Examples
///
/// ```
/// use airsl::RootTable;
/// use airsl::luals::setup;
/// use airsl::modules::stdlib;
///
/// let project = tempfile::tempdir()?;
/// let report = setup(project.path(), &stdlib()?, &RootTable::default())?;
/// assert_eq!(
///     report.to_string(),
///     "created .airsl/types/airsstack.lua\ncreated .luarc.json"
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn setup(project: &Path, modules: &ModuleSet, root: &RootTable) -> Result<SetupReport> {
    fs::read_dir(project).map_err(|source| Error::Io {
        operation: "open project directory",
        path: project.display().to_string(),
        source,
    })?;

    for module in modules.iter() {
        verify(module, root)?;
    }
    let rendered = TypeFile::render(modules, root)?;

    let type_rel = format!("{TYPES_DIR}/{}", rendered.file_name());
    let type_target = Target::resolve(project, &type_rel, Owner::TypeFile)?;
    let luarc_rel = pick_luarc(project);
    let luarc_target = Target::resolve(project, luarc_rel, Owner::Luarc)?;
    let luarc_plan = luarc::plan(luarc_target.existing.as_deref()).map_err(|refusal| {
        luarc_target
            .owner
            .refuse(&luarc_target.shown, refusal.into_reason())
    })?;

    let types_dir = project.join(TYPES_DIR);
    fs::create_dir_all(&types_dir).map_err(|source| Error::Io {
        operation: "create directory",
        path: types_dir.display().to_string(),
        source,
    })?;
    let type_change = type_target.write_if_different(rendered.as_str().as_bytes())?;
    let luarc_change = match luarc_plan {
        LuarcPlan::Unchanged => FileChange::Unchanged,
        LuarcPlan::Write(bytes) => luarc_target.write_if_different(&bytes)?,
    };

    Ok(SetupReport {
        type_file: FileReport {
            path: type_rel,
            change: type_change,
        },
        luarc: FileReport {
            path: luarc_rel.to_owned(),
            change: luarc_change,
        },
    })
}

/// The luarc to edit: `.luarc.json` if an entry of that name exists, else `.luarc.jsonc` if one
/// does, else `.luarc.json`, which is then created. A dangling symlink counts as existing, so it is
/// reported rather than silently replaced.
fn pick_luarc(project: &Path) -> &'static str {
    LUARC_NAMES
        .into_iter()
        .find(|name| fs::symlink_metadata(project.join(name)).is_ok())
        .unwrap_or(LUARC_NAMES[0])
}

/// Which of the two files a target is, which decides the error a refusal becomes.
#[derive(Debug, Clone, Copy)]
enum Owner {
    TypeFile,
    Luarc,
}

impl Owner {
    fn refuse(self, shown: &Path, reason: impl Into<String>) -> Error {
        let path = shown.display().to_string();
        let reason = reason.into();
        match self {
            Self::TypeFile => Error::TypeFileRefused { path, reason },
            Self::Luarc => Error::LuarcRefused { path, reason },
        }
    }
}

/// One file setup may write: where it is for the user, where it really is, and what it holds now.
struct Target {
    owner: Owner,
    /// The project path joined with the relative path, as errors show it.
    shown: PathBuf,
    /// The path to read and write, with any symlink resolved.
    actual: PathBuf,
    /// The bytes currently on disk, `None` when the target does not exist.
    existing: Option<Vec<u8>>,
    /// The permissions a rewrite keeps, `None` when the target does not exist.
    permissions: Option<fs::Permissions>,
}

impl Target {
    /// Finds `rel` under `project`, refuses it when it cannot be written safely, and reads it.
    fn resolve(project: &Path, rel: &str, owner: Owner) -> Result<Self> {
        let shown = project.join(rel);
        let read_error = |source| Error::Io {
            operation: "read",
            path: shown.display().to_string(),
            source,
        };

        let entry = match fs::symlink_metadata(&shown) {
            Ok(metadata) => metadata,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                return Ok(Self {
                    owner,
                    actual: shown.clone(),
                    shown,
                    existing: None,
                    permissions: None,
                });
            }
            Err(error) => return Err(read_error(error)),
        };

        let actual = if entry.file_type().is_symlink() {
            match fs::canonicalize(&shown) {
                Ok(resolved) => resolved,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Err(owner.refuse(&shown, "it is a dangling symlink"));
                }
                Err(error) => return Err(read_error(error)),
            }
        } else {
            shown.clone()
        };

        // The kind and permissions of what a write would replace: the link's target, not the link.
        let metadata = if entry.file_type().is_symlink() {
            fs::metadata(&actual).map_err(read_error)?
        } else {
            entry
        };
        if !metadata.is_file() {
            return Err(owner.refuse(&shown, "it is not a regular file"));
        }
        let permissions = metadata.permissions();
        if permissions.readonly() {
            return Err(owner.refuse(&shown, "it is read-only"));
        }

        let existing = fs::read(&actual).map_err(read_error)?;
        Ok(Self {
            owner,
            shown,
            actual,
            existing: Some(existing),
            permissions: Some(permissions),
        })
    }

    /// Writes `bytes` unless the target already holds exactly them, and says which it was.
    fn write_if_different(&self, bytes: &[u8]) -> Result<FileChange> {
        match &self.existing {
            Some(current) if current == bytes => Ok(FileChange::Unchanged),
            existing => {
                self.write(bytes)?;
                Ok(if existing.is_some() {
                    FileChange::Updated
                } else {
                    FileChange::Created
                })
            }
        }
    }

    /// Writes a temporary file beside the actual target and renames it over the target, so a reader
    /// never sees a half-written file and a symlink to the target survives.
    ///
    /// A failure is reported against the target and carries the operating-system error unchanged.
    /// The temporary file is created through [`tempfile::Builder::make_in`] with our own open call
    /// because the library's convenience constructor wraps its error with the temporary file's
    /// path, which would put an unrelated name in the message and hide the cause. The file is
    /// flushed to disk before the rename, so a crash cannot leave the target empty.
    fn write(&self, bytes: &[u8]) -> Result<()> {
        let fail = |source: io::Error| Error::Io {
            operation: "write",
            path: self.shown.display().to_string(),
            source,
        };
        let directory = self.actual.parent().ok_or_else(|| {
            fail(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the path has no parent directory",
            ))
        })?;
        let mut file = tempfile::Builder::new()
            .make_in(directory, create_private)
            .map_err(fail)?;
        file.as_file_mut().write_all(bytes).map_err(fail)?;
        self.permissions
            .as_ref()
            .map_or_else(
                || set_created_mode(file.as_file()),
                |permissions| file.as_file().set_permissions(permissions.clone()),
            )
            .map_err(fail)?;
        file.as_file().sync_all().map_err(fail)?;
        file.persist(&self.actual)
            .map_err(|error| fail(error.error))?;
        Ok(())
    }
}

/// Creates the temporary file, failing if the name is taken. On Unix it is readable only by its
/// owner until [`set_created_mode`] or the target's own permissions replace that.
fn create_private(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Gives a file setup created the mode a hand-made file would have. The temporary file's own mode
/// would hide it from every other user.
#[cfg(unix)]
fn set_created_mode(file: &fs::File) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(0o644))
}

/// Leaves a created file with the temporary file's own permissions.
#[cfg(not(unix))]
fn set_created_mode(_file: &fs::File) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use std::fs;
    use std::path::Path;
    use std::time::Duration;

    use super::{FileChange, FileReport, setup};
    use crate::description::{Function, ModuleDescription};
    use crate::error::{Error, Result};
    use crate::luals::TypeFile;
    use crate::modules::{HostModule, InstallContext, ModuleSet};
    use crate::types::{ModuleName, RootTable};

    /// What a stub installs: the function `f`, `f` plus a key its description does not mention, or
    /// nothing.
    #[derive(Clone, Copy)]
    enum Installs {
        F,
        FAndUndescribed,
        Nothing,
    }

    /// Whether a stub's `describe` succeeds.
    #[derive(Clone, Copy)]
    enum Describes {
        Fine,
        Fails,
    }

    /// A module that describes the function `f` and installs what it is told to.
    struct Stub {
        name: ModuleName,
        describes: Describes,
        installs: Installs,
    }

    impl HostModule for Stub {
        fn name(&self) -> &ModuleName {
            &self.name
        }

        fn install(
            &self,
            lua: &mlua::Lua,
            table: &mlua::Table,
            _context: &InstallContext<'_>,
        ) -> Result<()> {
            let keys: &[&str] = match self.installs {
                Installs::F => &["f"],
                Installs::FAndUndescribed => &["f", "g"],
                Installs::Nothing => &[],
            };
            for key in keys {
                let function = lua
                    .create_function(|_, ()| Ok(()))
                    .map_err(|e| Error::lua("stub", e))?;
                table
                    .set(*key, function)
                    .map_err(|e| Error::lua("stub", e))?;
            }
            Ok(())
        }

        fn describe(&self) -> Result<ModuleDescription> {
            match self.describes {
                Describes::Fine => ModuleDescription::builder("A stub.")
                    .function(Function::new("f", "A function."))
                    .build(),
                Describes::Fails => Err(Error::InvalidDescription { reason: "x".into() }),
            }
        }
    }

    fn set_of(modules: &[(&str, Describes, Installs)]) -> ModuleSet {
        let mut set = ModuleSet::new();
        for (name, describes, installs) in modules {
            set.insert(Box::new(Stub {
                name: ModuleName::new(*name).unwrap(),
                describes: *describes,
                installs: *installs,
            }))
            .unwrap();
        }
        set
    }

    fn one() -> ModuleSet {
        set_of(&[("one", Describes::Fine, Installs::F)])
    }

    /// A report entry as a comparable pair.
    fn entry(report: &FileReport) -> (&str, FileChange) {
        (report.path(), report.change())
    }

    fn run(project: &Path, modules: &ModuleSet) -> Result<super::SetupReport> {
        setup(project, modules, &RootTable::default())
    }

    const TYPE_FILE: &str = ".airsl/types/airsstack.lua";
    const L1: &str = "{\n  \"workspace.library\": [\n    \".airsl/types\"\n  ]\n}\n";

    fn rendered(modules: &ModuleSet) -> String {
        TypeFile::render(modules, &RootTable::default())
            .unwrap()
            .as_str()
            .to_owned()
    }

    #[test]
    fn first_run_creates_both_files() {
        let dir = tempfile::tempdir().unwrap();
        let modules = one();
        let report = run(dir.path(), &modules).unwrap();
        assert_eq!(
            report.to_string(),
            "created .airsl/types/airsstack.lua\ncreated .luarc.json"
        );
        assert_eq!(entry(report.type_file()), (TYPE_FILE, FileChange::Created));
        assert_eq!(entry(report.luarc()), (".luarc.json", FileChange::Created));
        assert_eq!(
            fs::read_to_string(dir.path().join(TYPE_FILE)).unwrap(),
            rendered(&modules)
        );
        assert_eq!(
            fs::read_to_string(dir.path().join(".luarc.json")).unwrap(),
            L1
        );
    }

    #[test]
    fn second_run_reports_unchanged_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let modules = one();
        run(dir.path(), &modules).unwrap();
        let mtime = |rel: &str| {
            fs::metadata(dir.path().join(rel))
                .unwrap()
                .modified()
                .unwrap()
        };
        let before = (mtime(TYPE_FILE), mtime(".luarc.json"));
        #[cfg(unix)]
        let inodes_before = inodes(dir.path());
        std::thread::sleep(Duration::from_millis(20));

        let report = run(dir.path(), &modules).unwrap();
        assert_eq!(
            report.to_string(),
            "unchanged .airsl/types/airsstack.lua\nunchanged .luarc.json"
        );
        assert_eq!(before, (mtime(TYPE_FILE), mtime(".luarc.json")));
        // A rewrite renames a new file into place, so an unchanged inode shows no rewrite even where
        // the modification time is too coarse to.
        #[cfg(unix)]
        assert_eq!(inodes_before, inodes(dir.path()));
    }

    /// The inode numbers of the type file and the luarc under `project`.
    #[cfg(unix)]
    fn inodes(project: &Path) -> (u64, u64) {
        use std::os::unix::fs::MetadataExt;
        let ino = |rel: &str| fs::metadata(project.join(rel)).unwrap().ino();
        (ino(TYPE_FILE), ino(".luarc.json"))
    }

    #[test]
    fn a_changed_module_updates_only_the_type_file() {
        let dir = tempfile::tempdir().unwrap();
        run(dir.path(), &one()).unwrap();
        let more = set_of(&[
            ("one", Describes::Fine, Installs::F),
            ("two", Describes::Fine, Installs::F),
        ]);
        let report = run(dir.path(), &more).unwrap();
        assert_eq!(entry(report.type_file()), (TYPE_FILE, FileChange::Updated));
        assert_eq!(
            entry(report.luarc()),
            (".luarc.json", FileChange::Unchanged)
        );
    }

    #[test]
    fn a_hand_edited_type_file_is_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let modules = one();
        run(dir.path(), &modules).unwrap();
        fs::write(dir.path().join(TYPE_FILE), "x").unwrap();
        let report = run(dir.path(), &modules).unwrap();
        assert_eq!(entry(report.type_file()), (TYPE_FILE, FileChange::Updated));
        assert_eq!(
            fs::read_to_string(dir.path().join(TYPE_FILE)).unwrap(),
            rendered(&modules)
        );
    }

    #[test]
    fn other_files_in_the_types_directory_are_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let types = dir.path().join(".airsl/types");
        fs::create_dir_all(&types).unwrap();
        fs::write(types.join("other.lua"), "keep").unwrap();
        run(dir.path(), &one()).unwrap();
        assert_eq!(fs::read_to_string(types.join("other.lua")).unwrap(), "keep");
    }

    #[test]
    fn a_luarc_refusal_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let luarc = dir.path().join(".luarc.json");
        fs::write(&luarc, "{\n  // keep me\n  \"workspace.library\": []\n}").unwrap();
        let error = run(dir.path(), &one()).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "will not edit `{}`: it is not plain JSON (key must be a string at line 2 column 3); add \".airsl/types\" to `workspace.library` by hand",
                luarc.display()
            )
        );
        assert!(!dir.path().join(".airsl").exists());
    }

    #[test]
    fn a_drifting_module_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let error = run(
            dir.path(),
            &set_of(&[("one", Describes::Fine, Installs::FAndUndescribed)]),
        )
        .unwrap_err();
        assert!(matches!(error, Error::DescriptionDrift { .. }));
        assert!(!dir.path().join(".airsl").exists());
        assert!(!dir.path().join(".luarc.json").exists());
    }

    #[test]
    fn a_missing_project_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("missing");
        let error = run(&project, &one()).unwrap_err();
        assert!(
            error.to_string().starts_with(&format!(
                "open project directory failed on `{}`: ",
                project.display()
            )),
            "{error}"
        );
        assert!(!project.exists());
    }

    #[test]
    fn a_file_as_project_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("file");
        fs::write(&project, "").unwrap();
        let error = run(&project, &one()).unwrap_err();
        assert!(
            error.to_string().starts_with(&format!(
                "open project directory failed on `{}`: ",
                project.display()
            )),
            "{error}"
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn prefers_luarc_json_over_jsonc() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".luarc.json"), "{}").unwrap();
        fs::write(dir.path().join(".luarc.jsonc"), "{}").unwrap();
        let report = run(dir.path(), &one()).unwrap();
        assert_eq!(entry(report.luarc()), (".luarc.json", FileChange::Updated));
        assert_eq!(
            fs::read_to_string(dir.path().join(".luarc.json")).unwrap(),
            L1
        );
        assert_eq!(
            fs::read_to_string(dir.path().join(".luarc.jsonc")).unwrap(),
            "{}"
        );
    }

    #[test]
    fn edits_luarc_jsonc_when_it_is_the_only_one() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".luarc.jsonc"), "{}").unwrap();
        let report = run(dir.path(), &one()).unwrap();
        assert_eq!(entry(report.luarc()), (".luarc.jsonc", FileChange::Updated));
        assert_eq!(
            fs::read_to_string(dir.path().join(".luarc.jsonc")).unwrap(),
            L1
        );
        assert!(!dir.path().join(".luarc.json").exists());
    }

    #[test]
    fn io_errors_name_the_joined_path() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".airsl"), "").unwrap();
        let error = run(dir.path(), &one()).unwrap_err();
        let expected = dir.path().join(".airsl/types");
        assert!(
            error.to_string().starts_with(&format!(
                "create directory failed on `{}`: ",
                expected.display()
            )),
            "{error}"
        );
    }

    #[test]
    fn an_empty_module_set_sets_up_a_project() {
        let dir = tempfile::tempdir().unwrap();
        let modules = ModuleSet::new();
        run(dir.path(), &modules).unwrap();
        let text = fs::read_to_string(dir.path().join(TYPE_FILE)).unwrap();
        assert_eq!(text, rendered(&modules));
        assert!(text.starts_with("---@meta\n"));
        assert!(text.contains("---@class airsstack\nairsstack = {}"));
        assert!(!text.contains("airsstack."));
    }

    #[test]
    fn a_describe_error_stops_setup_before_writing() {
        let dir = tempfile::tempdir().unwrap();
        let error = run(
            dir.path(),
            &set_of(&[("one", Describes::Fails, Installs::Nothing)]),
        )
        .unwrap_err();
        assert!(matches!(error, Error::DescribeFailed { .. }));
        assert!(!dir.path().join(".airsl").exists());
        assert!(!dir.path().join(".luarc.json").exists());
    }

    #[test]
    fn a_read_only_luarc_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let luarc = dir.path().join(".luarc.json");
        fs::write(&luarc, "{}").unwrap();
        let original = fs::metadata(&luarc).unwrap().permissions();
        let mut read_only = original.clone();
        read_only.set_readonly(true);
        fs::set_permissions(&luarc, read_only).unwrap();

        let result = run(dir.path(), &one());
        fs::set_permissions(&luarc, original).unwrap();

        assert_eq!(
            result.unwrap_err().to_string(),
            format!(
                "will not edit `{}`: it is read-only; add \".airsl/types\" to `workspace.library` by hand",
                luarc.display()
            )
        );
        assert!(!dir.path().join(".airsl").exists());
    }

    #[test]
    fn a_read_only_type_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join(TYPE_FILE);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "old").unwrap();
        let original = fs::metadata(&target).unwrap().permissions();
        let mut read_only = original.clone();
        read_only.set_readonly(true);
        fs::set_permissions(&target, read_only).unwrap();

        let result = run(dir.path(), &one());
        fs::set_permissions(&target, original).unwrap();

        assert_eq!(
            result.unwrap_err().to_string(),
            format!("will not write `{}`: it is read-only", target.display())
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "old");
        assert!(!dir.path().join(".luarc.json").exists());
    }

    #[test]
    fn a_directory_at_the_type_file_path_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join(TYPE_FILE);
        fs::create_dir_all(&target).unwrap();
        let error = run(dir.path(), &one()).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "will not write `{}`: it is not a regular file",
                target.display()
            )
        );
        assert!(!dir.path().join(".luarc.json").exists());
    }

    #[cfg(unix)]
    mod unix {
        use std::fs;
        use std::os::unix::fs::{PermissionsExt, symlink};
        use std::os::unix::net::UnixListener;

        use super::{Error, L1, TYPE_FILE, one, rendered, run};

        fn mode(path: &std::path::Path) -> u32 {
            fs::metadata(path).unwrap().permissions().mode() & 0o777
        }

        #[test]
        fn an_unreadable_types_directory_is_an_io_read_error() {
            let dir = tempfile::tempdir().unwrap();
            let types = dir.path().join(".airsl/types");
            fs::create_dir_all(&types).unwrap();
            fs::set_permissions(&types, fs::Permissions::from_mode(0o000)).unwrap();
            // A superuser reads a directory whatever its mode, so the refusal cannot be provoked.
            if fs::read_dir(&types).is_ok() {
                fs::set_permissions(&types, fs::Permissions::from_mode(0o755)).unwrap();
                return;
            }

            let result = run(dir.path(), &one());
            fs::set_permissions(&types, fs::Permissions::from_mode(0o755)).unwrap();

            let error = result.unwrap_err();
            let expected = dir.path().join(TYPE_FILE);
            assert!(
                error
                    .to_string()
                    .starts_with(&format!("read failed on `{}`: ", expected.display())),
                "{error}"
            );
            assert!(!dir.path().join(".luarc.json").exists());
        }

        #[test]
        fn a_created_file_has_mode_644() {
            let dir = tempfile::tempdir().unwrap();
            run(dir.path(), &one()).unwrap();
            assert_eq!(mode(&dir.path().join(TYPE_FILE)), 0o644);
            assert_eq!(mode(&dir.path().join(".luarc.json")), 0o644);
        }

        #[test]
        fn an_updated_file_keeps_its_mode() {
            let dir = tempfile::tempdir().unwrap();
            let luarc = dir.path().join(".luarc.json");
            fs::write(&luarc, "{}").unwrap();
            fs::set_permissions(&luarc, fs::Permissions::from_mode(0o640)).unwrap();
            run(dir.path(), &one()).unwrap();
            assert_eq!(mode(&luarc), 0o640);
            assert_eq!(fs::read_to_string(&luarc).unwrap(), L1);
        }

        #[test]
        fn a_symlinked_luarc_is_written_through() {
            let dir = tempfile::tempdir().unwrap();
            let elsewhere = dir.path().join("elsewhere");
            fs::create_dir(&elsewhere).unwrap();
            fs::write(elsewhere.join("luarc.json"), "{}").unwrap();
            let link = dir.path().join(".luarc.json");
            symlink(elsewhere.join("luarc.json"), &link).unwrap();

            run(dir.path(), &one()).unwrap();

            assert!(
                fs::symlink_metadata(&link)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(
                fs::read_to_string(elsewhere.join("luarc.json")).unwrap(),
                L1
            );
        }

        #[test]
        fn a_dangling_symlink_luarc_is_refused() {
            let dir = tempfile::tempdir().unwrap();
            let link = dir.path().join(".luarc.json");
            symlink(dir.path().join("missing.json"), &link).unwrap();
            let error = run(dir.path(), &one()).unwrap_err();
            assert_eq!(
                error.to_string(),
                format!(
                    "will not edit `{}`: it is a dangling symlink; add \".airsl/types\" to `workspace.library` by hand",
                    link.display()
                )
            );
            assert!(!dir.path().join(".airsl").exists());
        }

        #[test]
        fn a_socket_luarc_is_refused_without_opening_it() {
            let dir = tempfile::tempdir().unwrap();
            let _listener = UnixListener::bind(dir.path().join(".luarc.json")).unwrap();
            let error = run(dir.path(), &one()).unwrap_err();
            assert!(
                matches!(&error, Error::LuarcRefused { reason, .. } if reason == "it is not a regular file"),
                "{error}"
            );
        }

        #[test]
        fn a_symlinked_type_file_is_written_through() {
            let dir = tempfile::tempdir().unwrap();
            let elsewhere = dir.path().join("elsewhere");
            fs::create_dir(&elsewhere).unwrap();
            fs::write(elsewhere.join("types.lua"), "old").unwrap();
            fs::create_dir_all(dir.path().join(".airsl/types")).unwrap();
            let link = dir.path().join(TYPE_FILE);
            symlink(elsewhere.join("types.lua"), &link).unwrap();
            let modules = one();

            run(dir.path(), &modules).unwrap();

            assert!(
                fs::symlink_metadata(&link)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(
                fs::read_to_string(elsewhere.join("types.lua")).unwrap(),
                rendered(&modules)
            );
        }

        #[test]
        fn a_symlink_cycle_luarc_is_an_io_read_error() {
            let dir = tempfile::tempdir().unwrap();
            let luarc = dir.path().join(".luarc.json");
            let other = dir.path().join("b.json");
            symlink(&other, &luarc).unwrap();
            symlink(&luarc, &other).unwrap();
            let error = run(dir.path(), &one()).unwrap_err();
            assert!(
                error
                    .to_string()
                    .starts_with(&format!("read failed on `{}`: ", luarc.display())),
                "{error}"
            );
            assert!(!dir.path().join(".airsl").exists());
        }

        #[test]
        fn a_symlink_to_a_read_only_luarc_is_refused() {
            let dir = tempfile::tempdir().unwrap();
            let elsewhere = dir.path().join("elsewhere");
            fs::create_dir(&elsewhere).unwrap();
            let target = elsewhere.join("luarc.json");
            fs::write(&target, "{}").unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o444)).unwrap();
            let link = dir.path().join(".luarc.json");
            symlink(&target, &link).unwrap();

            let result = run(dir.path(), &one());
            fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();

            assert_eq!(
                result.unwrap_err().to_string(),
                format!(
                    "will not edit `{}`: it is read-only; add \".airsl/types\" to `workspace.library` by hand",
                    link.display()
                )
            );
            assert_eq!(fs::read_to_string(&target).unwrap(), "{}");
            assert!(!dir.path().join(".airsl").exists());
        }

        #[test]
        fn a_failed_write_names_the_target_not_the_temporary_file() {
            let dir = tempfile::tempdir().unwrap();
            run(dir.path(), &one()).unwrap();
            let target = dir.path().join(TYPE_FILE);
            fs::write(&target, "x").unwrap();
            let types = dir.path().join(".airsl/types");
            fs::set_permissions(&types, fs::Permissions::from_mode(0o555)).unwrap();
            // A superuser creates files in a read-only directory, so the failure cannot be provoked.
            if fs::File::create(types.join("probe")).is_ok() {
                fs::set_permissions(&types, fs::Permissions::from_mode(0o755)).unwrap();
                return;
            }

            let result = run(dir.path(), &one());
            fs::set_permissions(&types, fs::Permissions::from_mode(0o755)).unwrap();

            // The project directory is itself a `.tmp` name, so only the part after the target path
            // is checked.
            let message = result.unwrap_err().to_string();
            let prefix = format!("write failed on `{}`: ", target.display());
            assert!(message.starts_with(&prefix), "{message}");
            let source = &message[prefix.len()..];
            assert_eq!(source, "Permission denied (os error 13)", "{message}");
            assert!(!source.contains(".tmp"), "{message}");
            assert!(!source.contains("at path"), "{message}");
            assert_eq!(fs::read_to_string(&target).unwrap(), "x");
            let leftovers: Vec<_> = fs::read_dir(&types)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .filter(|name| name.to_string_lossy().starts_with(".tmp"))
                .collect();
            assert!(leftovers.is_empty(), "{leftovers:?}");
        }

        #[test]
        fn a_dangling_symlink_type_file_is_refused() {
            let dir = tempfile::tempdir().unwrap();
            fs::create_dir_all(dir.path().join(".airsl/types")).unwrap();
            let link = dir.path().join(TYPE_FILE);
            symlink(dir.path().join("missing.lua"), &link).unwrap();
            let error = run(dir.path(), &one()).unwrap_err();
            assert_eq!(
                error.to_string(),
                format!(
                    "will not write `{}`: it is a dangling symlink",
                    link.display()
                )
            );
        }
    }
}
