//! The `airsstack.fs` host module.
//!
//! The first module that takes a grant, and the reason the grant machinery exists. Every function
//! here resolves its argument through the crate's internal path guard before touching anything, so
//! a script holds a string and never a file handle — there is nothing for it to reach around.
//!
//! Split from `path` deliberately: building a path needs no authority, opening one does. A script
//! that only assembles filenames is granted nothing and can still do its job.
//!
//! Responsibilities: [`Fs`], installing the read, write, metadata and directory operations.
//!
//! Non-responsibilities: deciding what a grant covers ([`crate::GrantSet`]) and resolving a path
//! to something checkable (the internal `guard` module). This module only knows which of the two
//! directions each operation needs, which is the one thing those cannot know.

use std::io::Write as _;
use std::path::Path as StdPath;
use std::sync::Arc;

use crate::error::{Error, Result};
use crate::modules::guard::PathGuard;
use crate::modules::{HostModule, InstallContext};
use crate::types::ModuleName;

/// Installs `airsstack.fs`.
#[derive(Debug)]
pub struct Fs {
    name: ModuleName,
}

impl Fs {
    /// Builds the module.
    ///
    /// # Panics
    ///
    /// Never in practice: the name is a literal that satisfies [`ModuleName`]'s rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: ModuleName::new("fs")
                .unwrap_or_else(|_| unreachable!("`fs` is a valid module name")),
        }
    }
}

impl Default for Fs {
    fn default() -> Self {
        Self::new()
    }
}

/// Wraps an I/O failure with what was being attempted.
fn io(operation: &'static str, path: &StdPath) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.display().to_string();
    move |source| Error::Io {
        operation,
        path,
        source,
    }
}

impl HostModule for Fs {
    #[expect(
        clippy::too_many_lines,
        reason = "one statement per installed function; splitting it would hide the surface \
                  this module presents rather than clarify it"
    )]
    fn install(
        &self,
        lua: &mlua::Lua,
        table: &mlua::Table,
        context: &InstallContext<'_>,
    ) -> Result<()> {
        let fail = |e: mlua::Error| Error::ModuleInstall {
            module: String::from("fs"),
            reason: e.to_string(),
        };
        let guard = PathGuard::new(Arc::new(context.grants().clone()), "fs");

        // --- reading ---

        let g = guard.clone();
        let read = lua
            .create_function(move |_, path: mlua::LuaString| {
                let target = g.read("read", &path.to_str()?)?;
                std::fs::read_to_string(&target)
                    .map_err(io("read", &target))
                    .map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("read", read).map_err(fail)?;

        let g = guard.clone();
        let read_lines = lua
            .create_function(move |lua, path: mlua::LuaString| {
                let target = g.read("read_lines", &path.to_str()?)?;
                let text = std::fs::read_to_string(&target).map_err(io("read_lines", &target))?;
                // A trailing newline terminates the last line rather than starting an empty one,
                // which is what every line-oriented tool means by it.
                let body = text.strip_suffix('\n').unwrap_or(&text);
                let lines: Vec<&str> = if body.is_empty() {
                    Vec::new()
                } else {
                    body.split('\n')
                        .map(|l| l.strip_suffix('\r').unwrap_or(l))
                        .collect()
                };
                lua.create_sequence_from(lines)
            })
            .map_err(fail)?;
        table.set("read_lines", read_lines).map_err(fail)?;

        // --- writing ---

        let g = guard.clone();
        let write = lua
            .create_function(move |_, (path, body): (mlua::LuaString, mlua::LuaString)| {
                let target = g.write("write", &path.to_str()?)?;
                std::fs::write(&target, body.as_bytes())
                    .map_err(io("write", &target))
                    .map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("write", write).map_err(fail)?;

        let g = guard.clone();
        let append = lua
            .create_function(move |_, (path, body): (mlua::LuaString, mlua::LuaString)| {
                let target = g.write("append", &path.to_str()?)?;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&target)
                    .map_err(io("append", &target))?;
                file.write_all(&body.as_bytes())
                    .map_err(io("append", &target))
                    .map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("append", append).map_err(fail)?;

        let g = guard.clone();
        let atomic_write = lua
            .create_function(move |_, (path, body): (mlua::LuaString, mlua::LuaString)| {
                let target = g.write("atomic_write", &path.to_str()?)?;
                atomic_write(&target, &body.as_bytes()).map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("atomic_write", atomic_write).map_err(fail)?;

        let g = guard.clone();
        let create_exclusive = lua
            .create_function(
                move |_, (path, body): (mlua::LuaString, Option<mlua::LuaString>)| {
                    let target = g.write("create_exclusive", &path.to_str()?)?;
                    let contents = body.as_ref().map(mlua::LuaString::as_bytes);
                    match std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&target)
                    {
                        Ok(mut file) => {
                            if let Some(bytes) = contents {
                                file.write_all(&bytes)
                                    .map_err(io("create_exclusive", &target))?;
                            }
                            Ok(true)
                        }
                        // Losing the race is the expected other outcome, not a failure: this function
                        // exists so that exactly one of several concurrent callers proceeds.
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
                        Err(e) => Err(mlua::Error::from(io("create_exclusive", &target)(e))),
                    }
                },
            )
            .map_err(fail)?;
        table
            .set("create_exclusive", create_exclusive)
            .map_err(fail)?;

        // --- interrogation ---

        // These three refuse an ungranted path rather than answering `false` for it. Answering
        // would conflate "the policy does not let you ask" with "it is not there", which is the
        // same distinction `env.get` draws and has to be drawn the same way. It leaks nothing: a
        // denial says the path is outside the grant, which the caller's own manifest already told
        // it, and says nothing about whether anything is there.
        let g = guard.clone();
        let exists = lua
            .create_function(move |_, path: mlua::LuaString| {
                Ok(g.read("exists", &path.to_str()?)?.exists())
            })
            .map_err(fail)?;
        table.set("exists", exists).map_err(fail)?;

        let g = guard.clone();
        let is_file = lua
            .create_function(move |_, path: mlua::LuaString| {
                Ok(g.read("is_file", &path.to_str()?)?.is_file())
            })
            .map_err(fail)?;
        table.set("is_file", is_file).map_err(fail)?;

        let g = guard.clone();
        let is_dir = lua
            .create_function(move |_, path: mlua::LuaString| {
                Ok(g.read("is_dir", &path.to_str()?)?.is_dir())
            })
            .map_err(fail)?;
        table.set("is_dir", is_dir).map_err(fail)?;

        let g = guard.clone();
        let stat = lua
            .create_function(move |lua, path: mlua::LuaString| {
                let target = g.read("stat", &path.to_str()?)?;
                let meta = std::fs::symlink_metadata(&target).map_err(io("stat", &target))?;
                let out = lua.create_table()?;
                out.set("size", meta.len())?;
                out.set(
                    "kind",
                    if meta.is_dir() {
                        "dir"
                    } else if meta.is_symlink() {
                        "symlink"
                    } else {
                        "file"
                    },
                )?;
                out.set("readonly", meta.permissions().readonly())?;
                out.set("modified", modified_seconds(&meta))?;
                Ok(out)
            })
            .map_err(fail)?;
        table.set("stat", stat).map_err(fail)?;

        let g = guard.clone();
        let canonicalize = lua
            .create_function(move |_, path: mlua::LuaString| {
                let target = g.read("canonicalize", &path.to_str()?)?;
                Ok(target.to_string_lossy().into_owned())
            })
            .map_err(fail)?;
        table.set("canonicalize", canonicalize).map_err(fail)?;

        let g = guard.clone();
        let same_content = lua
            .create_function(
                move |_, (left, right): (mlua::LuaString, mlua::LuaString)| {
                    let a = g.read("same_content", &left.to_str()?)?;
                    let b = g.read("same_content", &right.to_str()?)?;
                    let left = std::fs::read(&a).map_err(io("same_content", &a))?;
                    let right = std::fs::read(&b).map_err(io("same_content", &b))?;
                    Ok(left == right)
                },
            )
            .map_err(fail)?;
        table.set("same_content", same_content).map_err(fail)?;

        // --- directories ---

        let g = guard.clone();
        let list = lua
            .create_function(move |lua, path: mlua::LuaString| {
                let target = g.read("list", &path.to_str()?)?;
                let mut names = Vec::new();
                for entry in std::fs::read_dir(&target).map_err(io("list", &target))? {
                    let entry = entry.map_err(io("list", &target))?;
                    names.push(entry.file_name().to_string_lossy().into_owned());
                }
                // Directory order is whatever the filesystem returns and differs between machines.
                names.sort();
                lua.create_sequence_from(names)
            })
            .map_err(fail)?;
        table.set("list", list).map_err(fail)?;

        let g = guard.clone();
        let walk = lua
            .create_function(move |lua, path: mlua::LuaString| {
                let target = g.read("walk", &path.to_str()?)?;
                lua.create_sequence_from(walk(&target)?)
            })
            .map_err(fail)?;
        table.set("walk", walk).map_err(fail)?;

        let g = guard.clone();
        let mkdir = lua
            .create_function(move |_, path: mlua::LuaString| {
                let target = g.write("mkdir", &path.to_str()?)?;
                std::fs::create_dir_all(&target)
                    .map_err(io("mkdir", &target))
                    .map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("mkdir", mkdir).map_err(fail)?;

        let g = guard.clone();
        let remove = lua
            .create_function(move |_, path: mlua::LuaString| {
                let target = g.write("remove", &path.to_str()?)?;
                std::fs::remove_file(&target)
                    .map_err(io("remove", &target))
                    .map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("remove", remove).map_err(fail)?;

        let g = guard.clone();
        let remove_dir = lua
            .create_function(move |_, path: mlua::LuaString| {
                let target = g.write("remove_dir", &path.to_str()?)?;
                std::fs::remove_dir_all(&target)
                    .map_err(io("remove_dir", &target))
                    .map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("remove_dir", remove_dir).map_err(fail)?;

        let g = guard.clone();
        let copy = lua
            .create_function(move |_, (from, to): (mlua::LuaString, mlua::LuaString)| {
                let source = g.read("copy", &from.to_str()?)?;
                let target = g.write("copy", &to.to_str()?)?;
                std::fs::copy(&source, &target).map_err(io("copy", &target))?;
                Ok(())
            })
            .map_err(fail)?;
        table.set("copy", copy).map_err(fail)?;

        let g = guard.clone();
        let rename = lua
            .create_function(move |_, (from, to): (mlua::LuaString, mlua::LuaString)| {
                // Both sides need write authority: a rename removes the source as surely as
                // `remove` does.
                let source = g.write("rename", &from.to_str()?)?;
                let target = g.write("rename", &to.to_str()?)?;
                std::fs::rename(&source, &target)
                    .map_err(io("rename", &target))
                    .map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("rename", rename).map_err(fail)?;

        // --- temporary files ---

        let g = guard.clone();
        let tempdir = lua
            .create_function(move |_, ()| {
                let base = std::env::temp_dir();
                let checked = g.write("tempdir", &base.to_string_lossy())?;
                let made = tempfile::Builder::new()
                    .prefix("airsl-")
                    .tempdir_in(&checked)
                    .map_err(io("tempdir", &checked))?;
                Ok(made.keep().to_string_lossy().into_owned())
            })
            .map_err(fail)?;
        table.set("tempdir", tempdir).map_err(fail)?;

        let g = guard;
        let temp_file = lua
            .create_function(move |_, ()| {
                let base = std::env::temp_dir();
                let checked = g.write("tempfile", &base.to_string_lossy())?;
                let made = tempfile::Builder::new()
                    .prefix("airsl-")
                    .tempfile_in(&checked)
                    .map_err(io("tempfile", &checked))?;
                let (_, path) = made.keep().map_err(|e| Error::Io {
                    operation: "tempfile",
                    path: checked.display().to_string(),
                    source: e.error,
                })?;
                Ok(path.to_string_lossy().into_owned())
            })
            .map_err(fail)?;
        table.set("tempfile", temp_file).map_err(fail)?;

        Ok(())
    }

    fn name(&self) -> &ModuleName {
        &self.name
    }
}

/// Seconds since the Unix epoch, or zero when the platform does not record it.
fn modified_seconds(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|since| i64::try_from(since.as_secs()).ok())
        .unwrap_or_default()
}

/// Every entry under `root`, as paths relative to it, in sorted order.
fn walk(root: &StdPath) -> Result<Vec<String>> {
    let mut found = Vec::new();
    for entry in walkdir::WalkDir::new(root).sort_by_file_name() {
        let entry = entry.map_err(|e| Error::Io {
            operation: "walk",
            path: root.display().to_string(),
            source: e.into(),
        })?;
        if entry.path() == root {
            continue;
        }
        if let Ok(relative) = entry.path().strip_prefix(root) {
            found.push(relative.to_string_lossy().into_owned());
        }
    }
    Ok(found)
}

/// Writes `body` to `target` so that a reader sees either the old contents or the new ones.
///
/// The temporary file is created in the same directory as the target, because a rename across
/// filesystems is not atomic and `/tmp` is routinely a different filesystem.
fn atomic_write(target: &StdPath, body: &[u8]) -> Result<()> {
    let directory = target.parent().unwrap_or_else(|| StdPath::new("."));
    let mut staged = tempfile::Builder::new()
        .prefix(".airsl-")
        .tempfile_in(directory)
        .map_err(io("atomic_write", directory))?;

    staged.write_all(body).map_err(io("atomic_write", target))?;
    staged.flush().map_err(io("atomic_write", target))?;
    staged.persist(target).map_err(|e| Error::Io {
        operation: "atomic_write",
        path: target.display().to_string(),
        source: e.error,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Fs;
    use crate::{Engine, GrantSet, HostModule as _, Policy, Script};
    use std::path::Path as StdPath;

    /// An engine granted read and write under `root`, plus the system temp directory.
    fn engine(root: &StdPath) -> Engine {
        let root = root.to_path_buf();
        Engine::builder()
            .policy(
                Policy::confined().with_grants(GrantSet::declared().with_fs(|fs| {
                    fs.read(root.clone())
                        .write(root)
                        .write(std::env::temp_dir())
                })),
            )
            .build()
            .unwrap()
    }

    /// Evaluates `source` with `root` available to the script as `arg[1]`.
    fn run<T: mlua::FromLuaMulti>(root: &StdPath, source: &str) -> crate::Result<T> {
        let engine = engine(root);
        let script = Script::from_source(source, "test")
            .unwrap()
            .with_args([root.to_string_lossy().into_owned()]);
        engine.eval_to::<T>(&script)
    }

    fn sandbox() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        (dir, root)
    }

    #[test]
    fn the_module_is_named_fs() {
        assert_eq!(Fs::new().name().as_str(), "fs");
    }

    #[test]
    fn write_then_read_round_trips() {
        let (_dir, root) = sandbox();
        let text: String = run(
            &root,
            "airsstack.fs.write(arg[1] .. '/a.txt', 'hello')
             return airsstack.fs.read(arg[1] .. '/a.txt')",
        )
        .unwrap();
        assert_eq!(text, "hello");
    }

    #[test]
    fn append_adds_to_what_is_already_there() {
        let (_dir, root) = sandbox();
        let text: String = run(
            &root,
            "airsstack.fs.write(arg[1] .. '/a.txt', 'one')
             airsstack.fs.append(arg[1] .. '/a.txt', 'two')
             return airsstack.fs.read(arg[1] .. '/a.txt')",
        )
        .unwrap();
        assert_eq!(text, "onetwo");
    }

    #[test]
    fn read_lines_splits_without_inventing_a_trailing_empty_line() {
        let (_dir, root) = sandbox();
        let count: i64 = run(
            &root,
            "airsstack.fs.write(arg[1] .. '/a.txt', 'one\\ntwo\\n')
             return #airsstack.fs.read_lines(arg[1] .. '/a.txt')",
        )
        .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn read_lines_of_an_empty_file_is_empty() {
        let (_dir, root) = sandbox();
        let count: i64 = run(
            &root,
            "airsstack.fs.write(arg[1] .. '/a.txt', '')
             return #airsstack.fs.read_lines(arg[1] .. '/a.txt')",
        )
        .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn read_lines_strips_a_carriage_return() {
        let (_dir, root) = sandbox();
        let first: String = run(
            &root,
            "airsstack.fs.write(arg[1] .. '/a.txt', 'one\\r\\ntwo\\r\\n')
             return airsstack.fs.read_lines(arg[1] .. '/a.txt')[1]",
        )
        .unwrap();
        assert_eq!(first, "one");
    }

    #[test]
    fn reading_outside_the_granted_root_is_refused() {
        let (_dir, root) = sandbox();
        let err = run::<String>(&root, "return airsstack.fs.read('/etc/hostname')").unwrap_err();
        assert!(err.to_string().contains("fs.read denied"), "{err}");
    }

    #[test]
    fn a_refusal_is_catchable_with_pcall() {
        let (_dir, root) = sandbox();
        let ok: bool = run(&root, "return (pcall(airsstack.fs.read, '/etc/hostname'))").unwrap();
        assert!(!ok);
    }

    #[test]
    fn writing_outside_the_granted_root_is_refused() {
        let (_dir, root) = sandbox();
        let err = run::<()>(&root, "airsstack.fs.write('/tmp/../etc/x', 'x')").unwrap_err();
        assert!(err.to_string().contains("denied"), "{err}");
    }

    #[test]
    fn exists_is_and_is_file_and_is_dir_agree_with_the_filesystem() {
        let (_dir, root) = sandbox();
        let out: String = run(
            &root,
            "airsstack.fs.write(arg[1] .. '/a.txt', 'x')
             airsstack.fs.mkdir(arg[1] .. '/sub')
             return tostring(airsstack.fs.exists(arg[1] .. '/a.txt'))
                 .. ',' .. tostring(airsstack.fs.is_file(arg[1] .. '/a.txt'))
                 .. ',' .. tostring(airsstack.fs.is_dir(arg[1] .. '/sub'))
                 .. ',' .. tostring(airsstack.fs.exists(arg[1] .. '/absent'))",
        )
        .unwrap();
        assert_eq!(out, "true,true,true,false");
    }

    #[test]
    fn exists_refuses_an_ungranted_path_rather_than_answering_false() {
        // Answering would make "you may not ask" indistinguishable from "it is not there", and a
        // script would report a missing file when it was actually denied. `env.get` draws the same
        // distinction; the two have to draw it the same way.
        let (_dir, root) = sandbox();
        let err = run::<bool>(&root, "return airsstack.fs.exists('/etc/hostname')").unwrap_err();
        assert!(err.to_string().contains("fs.exists denied"), "{err}");
    }

    #[test]
    fn exists_still_answers_false_for_an_absent_path_inside_the_grant() {
        let (_dir, root) = sandbox();
        let found: bool = run(&root, "return airsstack.fs.exists(arg[1] .. '/absent')").unwrap();
        assert!(!found);
    }

    #[test]
    fn is_file_and_is_dir_refuse_an_ungranted_path_too() {
        let (_dir, root) = sandbox();
        for call in ["is_file", "is_dir"] {
            let err = run::<bool>(
                &root,
                &format!("return airsstack.fs.{call}('/etc/hostname')"),
            )
            .unwrap_err();
            assert!(
                err.to_string().contains(&format!("fs.{call} denied")),
                "{err}"
            );
        }
    }

    #[test]
    fn stat_reports_size_and_kind() {
        let (_dir, root) = sandbox();
        let out: String = run(
            &root,
            "airsstack.fs.write(arg[1] .. '/a.txt', 'hello')
             local s = airsstack.fs.stat(arg[1] .. '/a.txt')
             return s.kind .. ':' .. tostring(s.size)",
        )
        .unwrap();
        assert_eq!(out, "file:5");
    }

    #[test]
    fn list_is_sorted_rather_than_in_filesystem_order() {
        let (_dir, root) = sandbox();
        let out: String = run(
            &root,
            "for _, n in ipairs({'zeta','alpha','mid'}) do
               airsstack.fs.write(arg[1] .. '/' .. n, '')
             end
             return table.concat(airsstack.fs.list(arg[1]), ',')",
        )
        .unwrap();
        assert_eq!(out, "alpha,mid,zeta");
    }

    #[test]
    fn walk_returns_sorted_paths_relative_to_the_root() {
        let (_dir, root) = sandbox();
        let out: String = run(
            &root,
            "airsstack.fs.mkdir(arg[1] .. '/sub')
             airsstack.fs.write(arg[1] .. '/sub/b.txt', '')
             airsstack.fs.write(arg[1] .. '/a.txt', '')
             return table.concat(airsstack.fs.walk(arg[1]), ',')",
        )
        .unwrap();
        assert_eq!(out, "a.txt,sub,sub/b.txt");
    }

    #[test]
    fn mkdir_creates_intermediate_directories() {
        let (_dir, root) = sandbox();
        let found: bool = run(
            &root,
            "airsstack.fs.mkdir(arg[1] .. '/a/b/c')
             return airsstack.fs.is_dir(arg[1] .. '/a/b/c')",
        )
        .unwrap();
        assert!(found);
    }

    #[test]
    fn remove_and_remove_dir_delete_what_they_name() {
        let (_dir, root) = sandbox();
        let out: String = run(
            &root,
            "airsstack.fs.write(arg[1] .. '/a.txt', '')
             airsstack.fs.mkdir(arg[1] .. '/sub')
             airsstack.fs.remove(arg[1] .. '/a.txt')
             airsstack.fs.remove_dir(arg[1] .. '/sub')
             return tostring(airsstack.fs.exists(arg[1] .. '/a.txt'))
                 .. ',' .. tostring(airsstack.fs.exists(arg[1] .. '/sub'))",
        )
        .unwrap();
        assert_eq!(out, "false,false");
    }

    #[test]
    fn copy_and_rename_move_contents() {
        let (_dir, root) = sandbox();
        let out: String = run(
            &root,
            "airsstack.fs.write(arg[1] .. '/a.txt', 'body')
             airsstack.fs.copy(arg[1] .. '/a.txt', arg[1] .. '/b.txt')
             airsstack.fs.rename(arg[1] .. '/b.txt', arg[1] .. '/c.txt')
             return airsstack.fs.read(arg[1] .. '/c.txt')
                 .. ',' .. tostring(airsstack.fs.exists(arg[1] .. '/b.txt'))",
        )
        .unwrap();
        assert_eq!(out, "body,false");
    }

    #[test]
    fn same_content_compares_bytes_rather_than_names() {
        let (_dir, root) = sandbox();
        let out: String = run(
            &root,
            "airsstack.fs.write(arg[1] .. '/a', 'x')
             airsstack.fs.write(arg[1] .. '/b', 'x')
             airsstack.fs.write(arg[1] .. '/c', 'y')
             return tostring(airsstack.fs.same_content(arg[1]..'/a', arg[1]..'/b'))
                 .. ',' .. tostring(airsstack.fs.same_content(arg[1]..'/a', arg[1]..'/c'))",
        )
        .unwrap();
        assert_eq!(out, "true,false");
    }

    #[test]
    fn atomic_write_replaces_the_contents() {
        let (_dir, root) = sandbox();
        let text: String = run(
            &root,
            "airsstack.fs.write(arg[1] .. '/a.txt', 'old')
             airsstack.fs.atomic_write(arg[1] .. '/a.txt', 'new')
             return airsstack.fs.read(arg[1] .. '/a.txt')",
        )
        .unwrap();
        assert_eq!(text, "new");
    }

    #[test]
    fn atomic_write_leaves_no_staging_file_behind() {
        let (_dir, root) = sandbox();
        let names: String = run(
            &root,
            "airsstack.fs.atomic_write(arg[1] .. '/a.txt', 'new')
             return table.concat(airsstack.fs.list(arg[1]), ',')",
        )
        .unwrap();
        assert_eq!(names, "a.txt");
    }

    #[test]
    fn create_exclusive_admits_exactly_one_claimant() {
        // The property `enforce.py` depends on: the second caller is told it lost, rather than
        // both proceeding as a read-then-write would allow.
        let (_dir, root) = sandbox();
        let out: String = run(
            &root,
            "local first  = airsstack.fs.create_exclusive(arg[1] .. '/claim', 'mine')
             local second = airsstack.fs.create_exclusive(arg[1] .. '/claim', 'yours')
             return tostring(first) .. ',' .. tostring(second)
                 .. ',' .. airsstack.fs.read(arg[1] .. '/claim')",
        )
        .unwrap();
        assert_eq!(out, "true,false,mine");
    }

    #[test]
    fn tempdir_and_tempfile_land_somewhere_writable() {
        let (_dir, root) = sandbox();
        let out: String = run(
            &root,
            "local d = airsstack.fs.tempdir()
             local f = airsstack.fs.tempfile()
             return tostring(#d > 0) .. ',' .. tostring(#f > 0)",
        )
        .unwrap();
        assert_eq!(out, "true,true");
    }

    #[test]
    fn a_policy_granting_nothing_refuses_every_operation() {
        let (_dir, root) = sandbox();
        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();
        let script = Script::from_source(
            "return (pcall(airsstack.fs.read, arg[1] .. '/a.txt'))",
            "probe",
        )
        .unwrap()
        .with_args([root.to_string_lossy().into_owned()]);
        assert!(!engine.eval_to::<bool>(&script).unwrap());
    }

    #[test]
    fn a_trusted_policy_waives_the_checks() {
        let (_dir, root) = sandbox();
        std::fs::write(root.join("a.txt"), "body").unwrap();
        let engine = Engine::builder().policy(Policy::trusted()).build().unwrap();
        let script = Script::from_source("return airsstack.fs.read(arg[1] .. '/a.txt')", "probe")
            .unwrap()
            .with_args([root.to_string_lossy().into_owned()]);
        assert_eq!(engine.eval_to::<String>(&script).unwrap(), "body");
    }
}
