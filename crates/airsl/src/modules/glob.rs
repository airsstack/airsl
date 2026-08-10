//! The `airsstack.glob` host module.
//!
//! `match` is pure pattern arithmetic and needs no authority. `walk` reads directories and so goes
//! through the same guard `fs` uses, inheriting the policy's read grants — which is what "inherits
//! `fs`" means in the roster, made concrete.
//!
//! Responsibilities: [`Glob`], installing `match` and `walk`.
//!
//! Non-responsibilities: the traversal itself, which is `walkdir`'s, and the containment rule,
//! which is the crate's internal path guard's.

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::modules::guard::PathGuard;
use crate::modules::{HostModule, InstallContext};
use crate::types::ModuleName;

/// Installs `airsstack.glob`.
#[derive(Debug)]
pub struct Glob {
    name: ModuleName,
}

impl Glob {
    /// Builds the module.
    ///
    /// # Panics
    ///
    /// Never in practice: the name is a literal that satisfies [`ModuleName`]'s rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: ModuleName::new("glob")
                .unwrap_or_else(|_| unreachable!("`glob` is a valid module name")),
        }
    }
}

impl Default for Glob {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds a matcher for `pattern`.
///
/// `literal_separator` is **on**, so `*` and `?` stop at a directory boundary: `*.rs` matches
/// `main.rs` and not `src/main.rs`. That is what every other path glob means — `.gitignore`,
/// `PurePath.match`, a shell with `globstar` — and `globset` recommends it for matching paths.
///
/// It was off, on the stated grounds that `*` crossing a separator was "the way the plugin scripts
/// expect". The opposite was true: the enforcement manifests declare things like `match:
/// ["**/*.rs"]`, and under the loose reading a manifest saying `*.rs` would also match
/// `deeply/nested/file.rs` — enforcing a rule over files its author never named. Widening
/// authority is the one direction this can be wrong in, so the module now compiles the strict
/// reading and the dispatcher's own compiler agrees with it.
///
/// What the switch must not cost is the `**/` case: `**/Cargo.toml` has to match a root-level
/// `Cargo.toml` as well as a nested one, because a root-level `Cargo.toml` is this repository's
/// most important Rust file. `globset` keeps `**` recursive under `literal_separator`, which is
/// exactly the split that makes the strict reading usable, and the tests below pin both halves.
fn matcher(pattern: &str) -> Result<globset::GlobMatcher> {
    globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|glob| glob.compile_matcher())
        .map_err(|source| Error::Denied {
            module: "glob",
            operation: "compile",
            detail: format!("`{pattern}` is not a valid glob: {source}"),
        })
}

impl HostModule for Glob {
    fn name(&self) -> &ModuleName {
        &self.name
    }

    fn install(
        &self,
        lua: &mlua::Lua,
        table: &mlua::Table,
        context: &InstallContext<'_>,
    ) -> Result<()> {
        let fail = |e: mlua::Error| Error::ModuleInstall {
            module: String::from("glob"),
            reason: e.to_string(),
        };
        let guard = PathGuard::new(Arc::new(context.grants().clone()), "glob");

        let matches = lua
            .create_function(|_, (pattern, path): (mlua::LuaString, mlua::LuaString)| {
                Ok(matcher(&pattern.to_str()?)?.is_match(path.to_str()?.as_ref()))
            })
            .map_err(fail)?;
        table.set("match", matches).map_err(fail)?;

        let g = guard;
        let walk = lua
            .create_function(
                move |lua, (root, pattern): (mlua::LuaString, mlua::LuaString)| {
                    let base = g.read("walk", &root.to_str()?)?;
                    let selected = matcher(&pattern.to_str()?)?;

                    let mut found = Vec::new();
                    for entry in walkdir::WalkDir::new(&base).sort_by_file_name() {
                        let entry = entry.map_err(|source| Error::Io {
                            operation: "walk",
                            path: base.display().to_string(),
                            source: source.into(),
                        })?;
                        let Ok(relative) = entry.path().strip_prefix(&base) else {
                            continue;
                        };
                        // Matched against the path relative to the root, so a pattern does not have to
                        // know where the root happens to live on this machine.
                        if !relative.as_os_str().is_empty() && selected.is_match(relative) {
                            found.push(relative.to_string_lossy().into_owned());
                        }
                    }
                    lua.create_sequence_from(found)
                },
            )
            .map_err(fail)?;
        table.set("walk", walk).map_err(fail)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{Glob, matcher};
    use crate::{Engine, GrantSet, HostModule as _, Policy, Script};
    use std::path::Path;

    fn eval(source: &str) -> String {
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        engine
            .eval_to::<String>(&Script::from_source(source, "test").unwrap())
            .unwrap()
    }

    #[test]
    fn the_module_is_named_glob() {
        assert_eq!(Glob::new().name().as_str(), "glob");
    }

    #[test]
    fn a_double_star_matches_zero_segments_as_well_as_many() {
        // The case that has to be checked rather than assumed: the enforcement dispatcher relies on
        // `**/Cargo.toml` matching a root-level `Cargo.toml`, which is this repository's most
        // important Rust file. A glob implementation that required at least one segment would
        // silently stop matching it.
        let m = matcher("**/Cargo.toml").unwrap();
        assert!(m.is_match(Path::new("Cargo.toml")), "zero segments");
        assert!(
            m.is_match(Path::new("crates/airsl/Cargo.toml")),
            "many segments"
        );
    }

    #[test]
    fn a_star_stops_at_a_directory_boundary() {
        // The defect this pins. With `literal_separator` off, `*.rs` also matched `src/main.rs`,
        // so an enforcement manifest declaring `match: ["*.rs"]` covered every Rust file in the
        // tree rather than the ones at its root — silently widening the rule past what its author
        // wrote. Widening authority is the one direction a matcher must not be wrong in.
        let m = matcher("*.rs").unwrap();
        assert!(
            m.is_match(Path::new("main.rs")),
            "a root-level file matches"
        );
        assert!(
            !m.is_match(Path::new("src/main.rs")),
            "`*` must not cross a separator"
        );

        let scoped = matcher("src/*.rs").unwrap();
        assert!(scoped.is_match(Path::new("src/main.rs")));
        assert!(
            !scoped.is_match(Path::new("src/a/b.rs")),
            "one segment, not a subtree"
        );
    }

    #[test]
    fn a_question_mark_stops_at_a_directory_boundary_too() {
        // Probed with a pattern the separator can actually land in: `?.rs` against `a/b.rs` can
        // never match either way, because `?` is one character and `a/b` is three, so it would
        // pass whatever the setting and prove nothing.
        let m = matcher("a?c").unwrap();
        assert!(m.is_match(Path::new("abc")));
        assert!(
            !m.is_match(Path::new("a/c")),
            "`?` must not match a separator"
        );
    }

    #[test]
    fn a_double_star_stays_recursive_under_the_strict_reading() {
        // The half the strict reading must not cost: `**` is still the recursive wildcard, which
        // is what makes `literal_separator` usable rather than merely stricter.
        let m = matcher("**/*.rs").unwrap();
        assert!(m.is_match(Path::new("main.rs")), "zero segments");
        assert!(m.is_match(Path::new("a/b/c/main.rs")), "many segments");

        let bare = matcher("**").unwrap();
        assert!(bare.is_match(Path::new("a/b/c")));
    }

    #[test]
    fn match_answers_from_lua_too() {
        assert_eq!(
            eval("return tostring(airsstack.glob.match('**/Cargo.toml', 'Cargo.toml'))"),
            "true"
        );
        assert_eq!(
            eval("return tostring(airsstack.glob.match('*.lua', 'a.rs'))"),
            "false"
        );
    }

    #[test]
    fn a_character_class_works() {
        assert_eq!(
            eval("return tostring(airsstack.glob.match('*.{lua,rs}', 'a.rs'))"),
            "true"
        );
    }

    #[test]
    fn an_invalid_pattern_raises_a_catchable_error() {
        assert_eq!(
            eval("return tostring(pcall(airsstack.glob.match, '[unclosed', 'x'))"),
            "false"
        );
    }

    #[test]
    fn walk_returns_sorted_matches_relative_to_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("Cargo.toml"), "").unwrap();
        std::fs::write(root.join("sub/Cargo.toml"), "").unwrap();
        std::fs::write(root.join("sub/other.txt"), "").unwrap();

        let engine = Engine::builder()
            .policy(
                Policy::confined()
                    .with_grants(GrantSet::declared().with_fs(|fs| fs.read(root.clone()))),
            )
            .build()
            .unwrap();
        let script = Script::from_source(
            "return table.concat(airsstack.glob.walk(arg[1], '**/Cargo.toml'), ',')",
            "t",
        )
        .unwrap()
        .with_args([root.to_string_lossy().into_owned()]);

        assert_eq!(
            engine.eval_to::<String>(&script).unwrap(),
            "Cargo.toml,sub/Cargo.toml"
        );
    }

    #[test]
    fn walk_needs_the_read_grant_that_listing_the_directory_would() {
        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();
        let script = Script::from_source("return airsstack.glob.walk('/etc', '*')", "t").unwrap();
        let err = engine.eval_to::<mlua::Value>(&script).unwrap_err();
        assert!(err.to_string().contains("glob.walk denied"), "{err}");
    }

    #[test]
    fn match_needs_no_grant_at_all() {
        // Pure pattern arithmetic: it reaches nothing, so the tightest preset still has it.
        assert_eq!(eval("return type(airsstack.glob.match)"), "function");
    }
}
