//! `airsl check` — compiling every Lua file under a path without running any of it.
//!
//! Exists because `airsl test` covers only what a test file loads, and a hook's entry point is
//! typically loaded by nothing: the tests exercise the modules underneath it. A syntax error in a
//! driver therefore survives a green test run, and `--fail-open` then swallows it at the moment
//! the hook fires — CI says nothing, the session says nothing, and the hook has quietly stopped
//! working. Measured on this repository's own plugin suite before this existed: a missing `end` in
//! the enforcement dispatcher left 244 tests passing and the hook exiting 0.
//!
//! Responsibilities: [`run`], which discovers, compiles and reports.
//!
//! Non-responsibilities: everything past the parser. A misspelled field, a `nil` arithmetic, a
//! module that raises the moment it is required — all compile, and all are a test's job.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::{Path, PathBuf};

use airsl::{Engine, Policy, Script};

/// Compiles every Lua file under `path` and returns the process exit code.
///
/// Zero when everything compiled and there was something to compile. A run that discovers no Lua
/// files at all exits non-zero, for the same reason `airsl test` does: "nothing to check" and
/// "everything checks out" must not read the same to CI, which is how a discovery glob that
/// stopped matching goes unnoticed for months.
pub(crate) fn run(path: &Path) -> i32 {
    let files = match discover(path) {
        Ok(files) => files,
        Err(error) => {
            eprintln!("airsl: {error}");
            return 1;
        }
    };

    if files.is_empty() {
        eprintln!("airsl: no Lua files found under `{}`", path.display());
        return 1;
    }

    // One engine for the whole run. Parsing consults nothing an earlier chunk could have changed —
    // it never reaches the globals table — so there is no isolation to buy by rebuilding it, and
    // `pure` is simply the cheapest state to construct.
    let engine = match Engine::builder().policy(Policy::pure()).build() {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("airsl: {error}");
            return 1;
        }
    };

    let mut failures = Vec::new();
    for file in &files {
        if let Err(error) = compile(&engine, file) {
            failures.push(format!("{}\n    {error}", file.display()));
        }
    }

    if failures.is_empty() {
        println!("{} file(s) compiled, 0 failed", files.len());
        return 0;
    }

    println!(
        "{} file(s) compiled, {} failed",
        files.len(),
        failures.len()
    );
    println!();
    for failure in &failures {
        println!("{failure}");
    }
    1
}

/// Loads and compiles one file.
fn compile(engine: &Engine, file: &Path) -> airsl::Result<()> {
    engine.check(&Script::from_file(file)?)
}

/// Every `.lua` file under `path`, in sorted order.
///
/// Sorted so a failing run is reproducible: an unsorted walk reports the same failures in a
/// different order on two machines, and a person comparing two logs cannot tell that from a
/// behavioural difference.
///
/// Unlike test discovery this takes *every* Lua file — drivers, libraries and test files alike.
/// The gap it closes is precisely the files no naming convention singles out.
fn discover(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut found = Vec::new();
    for entry in walkdir::WalkDir::new(path).sort_by_file_name() {
        let entry = entry?;
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "lua") {
            found.push(entry.path().to_path_buf());
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{discover, run};
    use std::path::Path;

    fn write(dir: &Path, name: &str, body: &str) {
        let target = dir.join(name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(target, body).unwrap();
    }

    #[test]
    fn a_tree_that_compiles_exits_zero() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "driver.lua", "local x = 1\nreturn x\n");
        write(
            dir.path(),
            "lib/helper.lua",
            "return { f = function() end }\n",
        );
        assert_eq!(run(dir.path()), 0);
    }

    #[test]
    fn a_syntax_error_anywhere_in_the_tree_exits_nonzero() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "good.lua", "return 1\n");
        write(dir.path(), "broken.lua", "local function f(\n");
        assert_ne!(run(dir.path()), 0);
    }

    #[test]
    fn a_driver_no_test_would_load_is_still_checked() {
        // The gap this command exists for: nothing requires a driver, so nothing compiles it.
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "enforce.lua", "if true then\n");
        assert_ne!(run(dir.path()), 0);
    }

    #[test]
    fn a_file_is_never_executed_while_being_checked() {
        // Compiling a driver must not perform the side effect the driver exists for.
        let dir = tempfile::tempdir().unwrap();
        let witness = dir.path().join("witness.txt");
        write(
            dir.path(),
            "side-effect.lua",
            &format!("error('ran: {}')\n", witness.display()),
        );
        assert_eq!(run(dir.path()), 0, "a chunk that raises still compiles");
        assert!(!witness.exists());
    }

    #[test]
    fn an_empty_tree_exits_nonzero_rather_than_claiming_success() {
        // "Nothing to check" and "everything checks out" must not read the same to CI.
        let dir = tempfile::tempdir().unwrap();
        assert_ne!(run(dir.path()), 0);
    }

    #[test]
    fn a_single_file_can_be_checked_directly() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "one.lua", "return 1\n");
        assert_eq!(run(&dir.path().join("one.lua")), 0);
    }

    #[test]
    fn discovery_is_sorted_recursive_and_takes_every_lua_file() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "z.lua", "return 1");
        write(dir.path(), "a.lua", "return 1");
        write(dir.path(), "lib/m.lua", "return 1");
        write(dir.path(), "a_test.lua", "return {}");
        write(dir.path(), "notes.md", "text");

        let found: Vec<String> = discover(dir.path())
            .unwrap()
            .iter()
            .map(|p| {
                p.strip_prefix(dir.path())
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(found, vec!["a.lua", "a_test.lua", "lib/m.lua", "z.lua"]);
    }
}
