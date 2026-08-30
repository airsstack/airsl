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
use crate::paths::rules::native::to_script_string;
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
                Ok(to_script_string(&out))
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

        // `is_absolute` and `absolute` are deliberately left out of the `/`-vocabulary conversion
        // that the rest of this module applies. Absoluteness is a property of the platform's path
        // grammar, not of which separator a rendering uses: `Path::is_absolute` already answers
        // "does this path's grammar make it absolute" correctly per platform (a bare `/a` is
        // absolute on unix and merely drive-relative on Windows), and re-deriving that from a
        // `/`-normalised string would have to reimplement the same platform grammar by hand. This
        // is the one place in the module where the outward vocabulary a script sees stays uniform
        // (`/`-separated) while the semantics underneath it are not.
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
        Some(parent) if !parent.as_os_str().is_empty() => to_script_string(parent),
        Some(_) => String::from("."),
        None => path.to_owned(),
    }
}

/// The final component of `path`, or the path itself when it has no components to strip.
fn basename(path: &str) -> String {
    // `file_name` yields a single path component, which by definition holds no separator on
    // either platform — this is the "cannot matter" case, not a missed conversion, so
    // `to_string_lossy` stays unconverted here.
    std::path::Path::new(path).file_name().map_or_else(
        || path.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The final component with its extension removed.
fn stem(path: &str) -> String {
    // Same reasoning as `basename`: `file_stem` is a fragment of a single component, so it cannot
    // carry a separator on either platform.
    std::path::Path::new(path)
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
}

/// The extension of the final component, without the dot, or an empty string.
///
/// Empty rather than `nil` so that a script can concatenate the result without checking it, and so
/// that "no extension" and "the call failed" are not the same value.
fn extension(path: &str) -> String {
    // Same reasoning as `basename`: `extension` is a fragment of a single component, so it cannot
    // carry a separator on either platform.
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
    let mut rooted = false;
    let mut leading: usize = 0;

    for component in std::path::Path::new(path).components() {
        match component {
            Component::RootDir | Component::Prefix(_) => {
                rooted = true;
                out.push(component.as_os_str());
            }
            // Popping a normal component cancels it; otherwise the `..` walks above what `out`
            // holds, which is either the filesystem root (every operating system already
            // absorbs that rather than rejecting it — `/..` is `/`) or, on a relative path, the
            // start of the path, with nothing above it to cancel against, so the `..` has to
            // survive into the result.
            Component::ParentDir if !out.pop() && !rooted => leading += 1,
            Component::CurDir | Component::ParentDir => {}
            Component::Normal(_) => out.push(component.as_os_str()),
        }
    }

    // `leading > 0` only happens when `rooted` is false — a rooted `..` above the root is
    // absorbed above and never reaches this count — so `out` is itself relative here and
    // pushing it onto a `PathBuf` that already holds the leading `..`s cannot have it replace
    // that prefix the way pushing a rooted path would. What `push` does not guard against is
    // `out` being empty: pushing an empty component still appends a trailing separator, so a
    // result that is purely leading `..`s (`".."`, `"../.."`) needs `out`'s components appended
    // one at a time rather than pushed as a (possibly empty) whole.
    let assembled = if leading == 0 {
        out
    } else {
        let mut prefix = PathBuf::new();
        for _ in 0..leading {
            prefix.push("..");
        }
        for component in out.components() {
            prefix.push(component);
        }
        prefix
    };

    let text = to_script_string(&assembled);
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

    let text = to_script_string(stripped);
    Ok(if text.is_empty() {
        String::from(".")
    } else {
        text
    })
}

/// Makes `path` absolute against the process working directory, without resolving symlinks.
///
/// [`std::path::absolute`] rather than `canonicalize`: the latter requires every component to
/// exist, which a script building an output path has no reason to satisfy. Kept as
/// [`std::path::absolute`] rather than reimplemented over the `/`-normalised vocabulary — see the
/// comment above this module's `is_absolute`/`absolute` registration for why absoluteness is
/// judged by the platform's own path grammar rather than by separator spelling.
///
/// # Errors
///
/// Returns [`Error::PathResolution`] when the working directory cannot be read.
fn absolute(path: &str) -> Result<String> {
    // Renders through `normalize` rather than converting the resolved path directly: `normalize`
    // already renders once through the `/`-vocabulary rule as its last step, so this inherits
    // that conversion (and the `/..`-absorption fix that comes with it) for free and needs no
    // conversion of its own.
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
    fn join_renders_in_the_script_vocabulary_regardless_of_platform() {
        // `PathBuf::push` joins with the platform separator (`\` on Windows), so the assembled
        // path is converted before it reaches Lua. Identity on unix, where this run happens — the
        // conversion itself is proved by `paths::rules::to_script_string`'s own flavour-taking
        // test, not by a difference this assertion can observe on this host.
        assert_eq!(eval("return airsstack.path.join('a', 'b')"), "a/b");
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
    fn dirname_renders_in_the_script_vocabulary_regardless_of_platform() {
        // `Path::parent` on a multi-component result carries the platform separator, same
        // reasoning as `join`'s sibling test above: identity on this unix host, proved for real by
        // `paths::rules::to_script_string`'s own flavour-taking test.
        assert_eq!(eval("return airsstack.path.dirname('a/b/c')"), "a/b");
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
    fn normalize_of_a_purely_leading_dotdot_result_has_no_trailing_separator() {
        // `PathBuf::push` appends a separator even when the pushed component is empty, so a result
        // that is entirely leading `..`s must not be assembled by pushing an empty remainder onto
        // the `..` prefix.
        assert_eq!(eval("return airsstack.path.normalize('..')"), "..");
        assert_eq!(eval("return airsstack.path.normalize('../..')"), "../..");
        assert_eq!(eval("return airsstack.path.normalize('a/../..')"), "..");
        assert_eq!(eval("return airsstack.path.normalize('./..')"), "..");
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
    fn normalize_absorbs_a_dotdot_at_the_filesystem_root() {
        // Unix-observable: every operating system already applies this rule to `/..` itself, so
        // `normalize` absorbs it the same way rather than reporting an escape above the root.
        assert_eq!(eval("return airsstack.path.normalize('/..')"), "/");
    }

    #[test]
    fn normalize_absorbs_a_dotdot_that_reaches_the_root_through_deeper_components() {
        assert_eq!(eval("return airsstack.path.normalize('/a/../..')"), "/");
    }

    #[cfg(windows)]
    #[test]
    fn normalize_absorbs_a_dotdot_above_a_drive_relative_root() {
        // `#[cfg(windows)]`: this does not even compile, let alone run, on the unix host this
        // change was authored and verified on. It is exercised by CI's Windows runner only.
        assert_eq!(eval("return airsstack.path.normalize('C:a/../..')"), "C:");
    }

    #[cfg(windows)]
    #[test]
    fn normalize_absorbs_a_dotdot_at_a_drive_absolute_root() {
        // `#[cfg(windows)]`: unverified on this unix host; see the sibling test above.
        assert_eq!(eval("return airsstack.path.normalize('C:/a/../..')"), "C:/");
    }

    #[cfg(windows)]
    #[test]
    fn normalize_converts_backslashes_while_absorbing_a_dotdot() {
        // `#[cfg(windows)]`: unverified on this unix host; see the sibling tests above.
        // Lua's long-bracket string (`[[...]]`) rather than a quoted literal, so the backslashes
        // reach `normalize` unescaped instead of being read as Lua escape sequences.
        assert_eq!(
            eval("return airsstack.path.normalize([[C:\\a\\..\\b]])"),
            "C:/b"
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
    fn relative_to_renders_in_the_script_vocabulary_regardless_of_platform() {
        // `strip_prefix`'s multi-component remainder carries the platform separator, same
        // reasoning as `join` and `dirname` above: identity on this unix host, proved for real by
        // `paths::rules::to_script_string`'s own flavour-taking test.
        assert_eq!(
            eval("return airsstack.path.relative_to('a/b/c', 'a')"),
            "b/c"
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
        // Absoluteness is a property of the platform's own path grammar (`is_absolute` is one of
        // this module's two deliberate exceptions to the uniform `/`-vocabulary rule), so a bare
        // `/a` is absolute on unix but only drive-relative on Windows, where a drive prefix is
        // required. `#[cfg(windows)]` here does not even compile-check on this unix host; CI's
        // Windows runner is what exercises it.
        #[cfg(unix)]
        assert_eq!(
            eval("return tostring(airsstack.path.is_absolute('/a'))"),
            "true"
        );
        #[cfg(windows)]
        {
            assert_eq!(
                eval("return tostring(airsstack.path.is_absolute('/a'))"),
                "false"
            );
            assert_eq!(
                eval("return tostring(airsstack.path.is_absolute('C:/a'))"),
                "true"
            );
        }
        // Holds on both platforms: a bare relative name is never absolute either way.
        assert_eq!(
            eval("return tostring(airsstack.path.is_absolute('a'))"),
            "false"
        );
    }

    #[test]
    fn absolute_leaves_an_absolute_path_alone() {
        // `#[cfg(windows)]` arm below is unverified on this unix host; see the comment on
        // `is_absolute_distinguishes_the_two_kinds_of_path`.
        #[cfg(unix)]
        assert_eq!(eval("return airsstack.path.absolute('/a/b')"), "/a/b");
        #[cfg(windows)]
        assert_eq!(eval("return airsstack.path.absolute('C:/a/b')"), "C:/a/b");
    }

    #[test]
    fn absolute_makes_a_relative_path_absolute() {
        let resolved = eval("return airsstack.path.absolute('a')");
        // `starts_with('/')` only proves absoluteness on unix; on Windows a rooted path can start
        // with a drive letter instead, so the platform's own `is_absolute` judgement is asserted
        // there rather than the unix-specific leading-slash shape. `#[cfg(windows)]` arm is
        // unverified on this unix host; see the comment on
        // `is_absolute_distinguishes_the_two_kinds_of_path`.
        #[cfg(unix)]
        assert!(resolved.starts_with('/'), "{resolved}");
        #[cfg(windows)]
        assert!(std::path::Path::new(&resolved).is_absolute(), "{resolved}");
        // Holds on both platforms: this is the vocabulary claim itself — the rendering is
        // `/`-separated regardless of which platform produced it.
        assert!(resolved.ends_with("/a"), "{resolved}");
    }

    #[test]
    fn absolute_normalises_what_it_produces() {
        // `#[cfg(windows)]` arm is unverified on this unix host; see the comment on
        // `is_absolute_distinguishes_the_two_kinds_of_path`.
        #[cfg(unix)]
        assert_eq!(eval("return airsstack.path.absolute('/a/b/../c')"), "/a/c");
        #[cfg(windows)]
        assert_eq!(
            eval("return airsstack.path.absolute('C:/a/b/../c')"),
            "C:/a/c"
        );
    }

    #[test]
    fn absolute_does_not_require_the_path_to_exist() {
        // `#[cfg(windows)]` arm is unverified on this unix host; see the comment on
        // `is_absolute_distinguishes_the_two_kinds_of_path`.
        #[cfg(unix)]
        assert_eq!(
            eval("return airsstack.path.absolute('/nonexistent/deep/file.lua')"),
            "/nonexistent/deep/file.lua"
        );
        #[cfg(windows)]
        assert_eq!(
            eval("return airsstack.path.absolute('C:/nonexistent/deep/file.lua')"),
            "C:/nonexistent/deep/file.lua"
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
