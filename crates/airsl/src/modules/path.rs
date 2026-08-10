//! The `airsstack.path` host module.
//!
//! Its own module, and separate from `fs`, because path manipulation needs no authority at all —
//! it is string arithmetic over separators. Splitting it from the module that opens files is what
//! lets an extension that only builds paths be granted nothing, and it is the clearest instance of
//! the rule that every module is a capability.
//!
//! Nothing here touches the filesystem. `normalize` resolves `.` and `..` lexically rather than by
//! asking the operating system, so it neither follows symlinks nor requires the path to exist —
//! which also means it is not a containment check. Confinement is `fs`'s job, decided against a
//! canonical path, and no amount of string normalisation substitutes for it.
//!
//! Responsibilities: [`Path`], installing `join`, `dirname`, `basename`, `stem`, `ext`,
//! `normalize`, `relative_to`, `is_absolute` and `absolute`.
//!
//! Non-responsibilities: reading anything. The single exception is `absolute`, which reads the
//! process working directory to resolve a relative path, and reads nothing else.

use std::path::{Component, PathBuf};

use crate::error::{Error, Result};
use crate::modules::{HostModule, InstallContext};
use crate::types::ModuleName;

/// Installs `airsstack.path`.
#[derive(Debug)]
pub struct Path {
    name: ModuleName,
}

impl Path {
    /// Builds the module.
    ///
    /// # Panics
    ///
    /// Never in practice: the name is a literal that satisfies [`ModuleName`]'s rules, and the
    /// crate's own test suite covers it.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: ModuleName::new("path")
                .unwrap_or_else(|_| unreachable!("`path` is a valid module name")),
        }
    }
}

impl Default for Path {
    fn default() -> Self {
        Self::new()
    }
}

impl HostModule for Path {
    fn name(&self) -> &ModuleName {
        &self.name
    }

    fn install(
        &self,
        lua: &mlua::Lua,
        table: &mlua::Table,
        _context: &InstallContext<'_>,
    ) -> Result<()> {
        let fail = |e: mlua::Error| Error::ModuleInstall {
            module: String::from("path"),
            reason: e.to_string(),
        };

        let join = lua
            .create_function(|_, parts: mlua::Variadic<mlua::LuaString>| {
                let mut out = PathBuf::new();
                for part in parts.iter() {
                    out.push(part.to_str()?.as_ref());
                }
                Ok(out.to_string_lossy().into_owned())
            })
            .map_err(fail)?;
        table.set("join", join).map_err(fail)?;

        let dirname = lua
            .create_function(|_, path: mlua::LuaString| Ok(dirname(&path.to_str()?)))
            .map_err(fail)?;
        table.set("dirname", dirname).map_err(fail)?;

        let basename = lua
            .create_function(|_, path: mlua::LuaString| Ok(basename(&path.to_str()?)))
            .map_err(fail)?;
        table.set("basename", basename).map_err(fail)?;

        let stem = lua
            .create_function(|_, path: mlua::LuaString| Ok(stem(&path.to_str()?)))
            .map_err(fail)?;
        table.set("stem", stem).map_err(fail)?;

        let ext = lua
            .create_function(|_, path: mlua::LuaString| Ok(extension(&path.to_str()?)))
            .map_err(fail)?;
        table.set("ext", ext).map_err(fail)?;

        let normalize = lua
            .create_function(|_, path: mlua::LuaString| Ok(normalize(&path.to_str()?)))
            .map_err(fail)?;
        table.set("normalize", normalize).map_err(fail)?;

        let relative_to = lua
            .create_function(|_, (path, base): (mlua::LuaString, mlua::LuaString)| {
                relative_to(&path.to_str()?, &base.to_str()?).map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("relative_to", relative_to).map_err(fail)?;

        let is_absolute = lua
            .create_function(|_, path: mlua::LuaString| {
                Ok(std::path::Path::new(path.to_str()?.as_ref()).is_absolute())
            })
            .map_err(fail)?;
        table.set("is_absolute", is_absolute).map_err(fail)?;

        let absolute = lua
            .create_function(|_, path: mlua::LuaString| {
                absolute(&path.to_str()?).map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("absolute", absolute).map_err(fail)?;

        Ok(())
    }
}

/// The directory part of `path`.
///
/// Follows POSIX `dirname` rather than [`std::path::Path::parent`], which yields an empty path for
/// a bare filename. `.` is what a script can pass straight back into `join`, and an empty string is
/// not.
fn dirname(path: &str) -> String {
    match std::path::Path::new(path).parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_string_lossy().into_owned(),
        Some(_) => String::from("."),
        None => path.to_owned(),
    }
}

/// The final component of `path`, or the path itself when it has no components to strip.
fn basename(path: &str) -> String {
    std::path::Path::new(path).file_name().map_or_else(
        || path.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The final component with its extension removed.
fn stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
}

/// The extension of the final component, without the dot, or an empty string.
///
/// Empty rather than `nil` so that a script can concatenate the result without checking it, and so
/// that "no extension" and "the call failed" are not the same value.
fn extension(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .map_or_else(String::new, |ext| ext.to_string_lossy().into_owned())
}

/// Resolves `.` and `..` textually, without consulting the filesystem.
///
/// Lexical because the alternative needs the path to exist, and most of what a script normalises is
/// a path it is about to create. A leading `..` on a relative path is kept — there is nothing above
/// the start of a relative path to cancel it against.
fn normalize(path: &str) -> String {
    let mut out = PathBuf::new();
    let mut leading: Vec<&str> = Vec::new();

    for component in std::path::Path::new(path).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // Popping a normal component cancels it; popping nothing means the `..` escapes
                // the start of a relative path and has to survive into the result.
                if !out.pop() {
                    leading.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }

    let rest = out.to_string_lossy();
    let text = if leading.is_empty() {
        rest.into_owned()
    } else if rest.is_empty() {
        leading.join("/")
    } else {
        format!("{}/{rest}", leading.join("/"))
    };

    if text.is_empty() {
        String::from(".")
    } else {
        text
    }
}

/// Expresses `path` relative to `base`.
///
/// Both sides are normalised first, so `a/./b` and `a/b` behave the same. Refuses rather than
/// walking upwards with `..`: the use this exists for is turning an absolute path into a
/// repository-relative one, and silently returning `../../elsewhere` for a path that is not under
/// `base` would answer a question the caller did not ask.
///
/// # Errors
///
/// Returns [`Error::PathNotRelative`] when `path` does not lie under `base`.
fn relative_to(path: &str, base: &str) -> Result<String> {
    let normalised = normalize(path);
    let root = normalize(base);

    let stripped = std::path::Path::new(&normalised)
        .strip_prefix(&root)
        .map_err(|_| Error::PathNotRelative {
            path: normalised.clone(),
            base: root.clone(),
        })?;

    let text = stripped.to_string_lossy();
    Ok(if text.is_empty() {
        String::from(".")
    } else {
        text.into_owned()
    })
}

/// Makes `path` absolute against the process working directory, without resolving symlinks.
///
/// [`std::path::absolute`] rather than `canonicalize`: the latter requires every component to
/// exist, which a script building an output path has no reason to satisfy.
///
/// # Errors
///
/// Returns [`Error::PathResolution`] when the working directory cannot be read.
fn absolute(path: &str) -> Result<String> {
    std::path::absolute(path)
        .map(|resolved| normalize(&resolved.to_string_lossy()))
        .map_err(|source| Error::PathResolution {
            path: path.to_owned(),
            source,
        })
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Path;
    use crate::{Engine, HostModule as _, Policy, Script};

    fn eval(source: &str) -> String {
        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();
        let script = Script::from_source(source, "test").unwrap();
        engine.eval_to::<String>(&script).unwrap()
    }

    #[test]
    fn the_module_is_named_path() {
        assert_eq!(Path::new().name().as_str(), "path");
    }

    #[test]
    fn join_builds_a_path_from_its_parts() {
        assert_eq!(eval("return airsstack.path.join('a', 'b', 'c')"), "a/b/c");
    }

    #[test]
    fn join_of_nothing_is_empty_rather_than_an_error() {
        assert_eq!(eval("return airsstack.path.join()"), "");
    }

    #[test]
    fn join_lets_an_absolute_part_replace_what_came_before_it() {
        // `std::path::PathBuf::push` semantics, and the behaviour a caller assembling a path from
        // a configured root plus a possibly-absolute override depends on.
        assert_eq!(eval("return airsstack.path.join('a', '/b')"), "/b");
    }

    #[test]
    fn dirname_gives_the_directory_part() {
        assert_eq!(eval("return airsstack.path.dirname('/a/b/c.lua')"), "/a/b");
    }

    #[test]
    fn dirname_of_a_bare_filename_is_the_current_directory() {
        // POSIX `dirname`, not `Path::parent`, which would give "" — a value that cannot be
        // passed back into `join`.
        assert_eq!(eval("return airsstack.path.dirname('c.lua')"), ".");
    }

    #[test]
    fn dirname_of_the_root_is_the_root() {
        assert_eq!(eval("return airsstack.path.dirname('/')"), "/");
    }

    #[test]
    fn basename_gives_the_final_component() {
        assert_eq!(
            eval("return airsstack.path.basename('/a/b/c.lua')"),
            "c.lua"
        );
        assert_eq!(eval("return airsstack.path.basename('c.lua')"), "c.lua");
    }

    #[test]
    fn stem_and_ext_split_the_final_component() {
        assert_eq!(eval("return airsstack.path.stem('/a/b/c.lua')"), "c");
        assert_eq!(eval("return airsstack.path.ext('/a/b/c.lua')"), "lua");
    }

    #[test]
    fn a_name_without_an_extension_has_an_empty_one() {
        assert_eq!(eval("return airsstack.path.ext('/a/b/README')"), "");
        assert_eq!(eval("return airsstack.path.stem('/a/b/README')"), "README");
    }

    #[test]
    fn a_dotfile_is_a_name_rather_than_an_extension() {
        assert_eq!(
            eval("return airsstack.path.stem('/a/.gitignore')"),
            ".gitignore"
        );
        assert_eq!(eval("return airsstack.path.ext('/a/.gitignore')"), "");
    }

    #[test]
    fn only_the_last_extension_is_taken() {
        assert_eq!(eval("return airsstack.path.ext('archive.tar.gz')"), "gz");
        assert_eq!(
            eval("return airsstack.path.stem('archive.tar.gz')"),
            "archive.tar"
        );
    }

    #[test]
    fn normalize_resolves_dot_and_dotdot() {
        assert_eq!(
            eval("return airsstack.path.normalize('/a/./b/../c')"),
            "/a/c"
        );
    }

    #[test]
    fn normalize_keeps_a_dotdot_that_escapes_a_relative_path() {
        // There is nothing above the start of a relative path to cancel against, so dropping it
        // would silently change which file the path names.
        assert_eq!(eval("return airsstack.path.normalize('../a')"), "../a");
        assert_eq!(eval("return airsstack.path.normalize('a/../../b')"), "../b");
    }

    #[test]
    fn normalize_of_an_empty_or_dot_path_is_the_current_directory() {
        assert_eq!(eval("return airsstack.path.normalize('')"), ".");
        assert_eq!(eval("return airsstack.path.normalize('./.')"), ".");
    }

    #[test]
    fn normalize_does_not_consult_the_filesystem() {
        assert_eq!(
            eval("return airsstack.path.normalize('/nonexistent/a/../b')"),
            "/nonexistent/b"
        );
    }

    #[test]
    fn relative_to_strips_the_base() {
        assert_eq!(
            eval("return airsstack.path.relative_to('/repo/crates/airsl/src', '/repo')"),
            "crates/airsl/src"
        );
    }

    #[test]
    fn relative_to_normalises_both_sides_first() {
        assert_eq!(
            eval("return airsstack.path.relative_to('/repo/./a/b', '/repo/c/..')"),
            "a/b"
        );
    }

    #[test]
    fn a_path_equal_to_its_base_is_the_current_directory() {
        assert_eq!(
            eval("return airsstack.path.relative_to('/repo', '/repo')"),
            "."
        );
    }

    #[test]
    fn relative_to_refuses_a_path_outside_the_base() {
        assert_eq!(
            eval(
                "local ok, err = pcall(airsstack.path.relative_to, '/elsewhere', '/repo')
                 return tostring(ok) .. ':' .. tostring(err):match('outside') "
            ),
            "false:outside"
        );
    }

    #[test]
    fn a_sibling_sharing_a_name_prefix_is_not_under_the_base() {
        // String prefix matching would accept this; component matching does not.
        assert_eq!(
            eval("return tostring(pcall(airsstack.path.relative_to, '/repo-extra/a', '/repo'))"),
            "false"
        );
    }

    #[test]
    fn is_absolute_distinguishes_the_two_kinds_of_path() {
        assert_eq!(
            eval("return tostring(airsstack.path.is_absolute('/a'))"),
            "true"
        );
        assert_eq!(
            eval("return tostring(airsstack.path.is_absolute('a'))"),
            "false"
        );
    }

    #[test]
    fn absolute_leaves_an_absolute_path_alone() {
        assert_eq!(eval("return airsstack.path.absolute('/a/b')"), "/a/b");
    }

    #[test]
    fn absolute_makes_a_relative_path_absolute() {
        let resolved = eval("return airsstack.path.absolute('a')");
        assert!(resolved.starts_with('/'), "{resolved}");
        assert!(resolved.ends_with("/a"), "{resolved}");
    }

    #[test]
    fn absolute_normalises_what_it_produces() {
        assert_eq!(eval("return airsstack.path.absolute('/a/b/../c')"), "/a/c");
    }

    #[test]
    fn absolute_does_not_require_the_path_to_exist() {
        assert_eq!(
            eval("return airsstack.path.absolute('/nonexistent/deep/file.lua')"),
            "/nonexistent/deep/file.lua"
        );
    }

    #[test]
    fn every_function_is_reachable_under_a_pure_policy_too() {
        // `path` needs no authority, so the tightest preset must still get all of it.
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        let script = Script::from_source(
            "local names = {'join','dirname','basename','stem','ext','normalize',
                            'relative_to','is_absolute','absolute'}
             for _, name in ipairs(names) do
               if type(airsstack.path[name]) ~= 'function' then return name end
             end
             return 'all'",
            "probe",
        )
        .unwrap();
        assert_eq!(engine.eval_to::<String>(&script).unwrap(), "all");
    }
}
