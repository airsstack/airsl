//! `airsl test` — discovering and running Lua test files.
//!
//! Exists because the migration it serves is otherwise unverifiable. The plugin suite has test
//! files that neither `cargo make dod` nor CI executes — they run under `sh` and `python3` by hand
//! — and porting several thousand lines of script onto a new runtime without a test story is how a
//! migration becomes a rewrite with unknown behaviour.
//!
//! The conventions are deliberately thin, because every convention is something a script author
//! has to learn:
//!
//! - A **test file** is named `*_test.lua` or `test_*.lua`.
//! - It **returns a table** whose named function values are the tests.
//! - A test **passes by returning**. It fails by raising, so Lua's own `assert` is the entire
//!   assertion surface and there is nothing new to learn.
//!
//! Responsibilities: [`run`], which discovers, executes and reports.
//!
//! Non-responsibilities: deciding what the tests may reach. They run under the same policy and the
//! same grant flags as `airsl run`, so a test has exactly the authority the thing it tests would.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::path::{Path, PathBuf};

use airsl::{Engine, Policy, Script};

/// What one test file reported.
struct FileOutcome {
    passed: usize,
    failures: Vec<String>,
}

/// Runs every test under `path` and returns the process exit code.
///
/// Zero when everything passed and there was something to run. A run that discovers no test files
/// at all exits non-zero: "no tests" and "all tests passed" print differently for a person and
/// must not read the same to CI, which is how a broken discovery glob goes unnoticed for months.
pub(crate) fn run(path: &Path, policy: &Policy) -> i32 {
    let files = match discover(path) {
        Ok(files) => files,
        Err(error) => {
            eprintln!("airsl: {error}");
            return 1;
        }
    };

    if files.is_empty() {
        eprintln!("airsl: no test files found under `{}`", path.display());
        eprintln!("       a test file is named `*_test.lua` or `test_*.lua`");
        return 1;
    }

    let mut passed = 0;
    let mut failures = Vec::new();

    for file in &files {
        let outcome = run_file(file, policy);
        passed += outcome.passed;
        for failure in outcome.failures {
            failures.push(failure);
        }
    }

    println!();
    if failures.is_empty() {
        println!("{passed} passed, 0 failed ({} files)", files.len());
        return 0;
    }

    println!(
        "{passed} passed, {} failed ({} files)",
        failures.len(),
        files.len()
    );
    println!();
    for failure in &failures {
        println!("{failure}");
    }
    1
}

/// Every test file under `path`, in sorted order.
///
/// Sorted so a failing run is reproducible: an unsorted walk means two machines report the same
/// failures in a different order, and a person comparing two logs cannot tell that from a
/// behavioural difference.
fn discover(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut found = Vec::new();
    for entry in walkdir::WalkDir::new(path).sort_by_file_name() {
        let entry = entry?;
        if entry.file_type().is_file() && is_test_file(entry.path()) {
            found.push(entry.path().to_path_buf());
        }
    }
    Ok(found)
}

/// Whether `path` is named the way a test file is named.
fn is_test_file(path: &Path) -> bool {
    if path.extension().is_none_or(|extension| extension != "lua") {
        return false;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    // Case-sensitive on purpose: these are conventions a person types, and `Index_Test.LUA`
    // silently counting as a test file would be a surprise rather than a convenience.
    name.ends_with("_test.lua") || name.starts_with("test_")
}

/// Runs one file's tests, printing a line per test.
fn run_file(file: &Path, policy: &Policy) -> FileOutcome {
    let mut outcome = FileOutcome {
        passed: 0,
        failures: Vec::new(),
    };

    println!("{}", file.display());

    // A fresh engine per file. Sharing one would let a test file leave globals behind for the next
    // — the isolation gap the crate documents — and a test suite is exactly where that would turn
    // into a failure nobody can reproduce alone.
    let engine = match Engine::builder().policy(policy.clone()).build() {
        Ok(engine) => engine,
        Err(error) => {
            outcome
                .failures
                .push(format!("{}: {error}", file.display()));
            return outcome;
        }
    };

    let script = match Script::from_file(file) {
        Ok(script) => script,
        Err(error) => {
            outcome
                .failures
                .push(format!("{}: {error}", file.display()));
            return outcome;
        }
    };

    let table = match engine.eval_to::<mlua::Table>(&script) {
        Ok(table) => table,
        Err(error) => {
            // Either the file failed to load, or it returned something that is not a table. Both
            // are the author's mistake and both need the same nudge.
            outcome.failures.push(format!(
                "{}: {error}\n    a test file must return a table of named functions",
                file.display()
            ));
            return outcome;
        }
    };

    for (name, case) in cases(&table) {
        match case.call::<()>(()) {
            Ok(()) => {
                outcome.passed += 1;
                println!("  ok    {name}");
            }
            Err(error) => {
                println!("  FAIL  {name}");
                outcome
                    .failures
                    .push(format!("{}: {name}\n    {error}", file.display()));
            }
        }
    }

    outcome
}

/// The named functions in `table`, in sorted order.
///
/// Sorted for the same reason discovery is: a test suite that runs in hash order reports its
/// failures in a different order on every run.
fn cases(table: &mlua::Table) -> Vec<(String, mlua::Function)> {
    let mut found: Vec<(String, mlua::Function)> = table
        .pairs::<mlua::Value, mlua::Value>()
        .filter_map(std::result::Result::ok)
        .filter_map(|(key, value)| match (key, value) {
            (mlua::Value::String(name), mlua::Value::Function(case)) => {
                name.to_str().ok().map(|name| (name.to_owned(), case))
            }
            _ => None,
        })
        .collect();
    found.sort_by(|left, right| left.0.cmp(&right.0));
    found
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{discover, is_test_file, run};
    use airsl::Policy;
    use std::path::Path;

    fn write(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn both_naming_conventions_are_recognised() {
        assert!(is_test_file(Path::new("a/index_test.lua")));
        assert!(is_test_file(Path::new("a/test_index.lua")));
    }

    #[test]
    fn an_ordinary_lua_file_is_not_a_test_file() {
        assert!(!is_test_file(Path::new("a/index.lua")));
        assert!(!is_test_file(Path::new("a/testing.lua")));
        assert!(!is_test_file(Path::new("a/test_index.txt")));
    }

    #[test]
    fn discovery_is_sorted_and_recursive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        write(dir.path(), "z_test.lua", "return {}");
        write(dir.path(), "a_test.lua", "return {}");
        write(&dir.path().join("sub"), "m_test.lua", "return {}");
        write(dir.path(), "ignored.lua", "return {}");

        let found: Vec<String> = discover(dir.path())
            .unwrap()
            .iter()
            .map(|path| {
                path.strip_prefix(dir.path())
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        // Sorted, and the nested file sorts with its directory rather than by its own name.
        assert_eq!(found, ["a_test.lua", "sub/m_test.lua", "z_test.lua"]);
    }

    #[test]
    fn a_passing_suite_exits_zero() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "a_test.lua",
            "return { works = function() assert(1 + 1 == 2) end }",
        );
        assert_eq!(run(dir.path(), &Policy::pure()), 0);
    }

    #[test]
    fn a_failing_assertion_exits_non_zero() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "a_test.lua",
            "return { broken = function() assert(false, 'nope') end }",
        );
        assert_ne!(run(dir.path(), &Policy::pure()), 0);
    }

    #[test]
    fn one_failure_among_several_still_fails_the_run() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "a_test.lua",
            "return {
               ok_one = function() end,
               ok_two = function() end,
               bad    = function() error('boom') end,
             }",
        );
        assert_ne!(run(dir.path(), &Policy::pure()), 0);
    }

    #[test]
    fn finding_no_tests_at_all_is_a_failure_not_a_pass() {
        // Otherwise a discovery glob that stops matching reads exactly like a green suite.
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "not-a-test.lua", "return {}");
        assert_ne!(run(dir.path(), &Policy::pure()), 0);
    }

    #[test]
    fn a_file_that_returns_no_table_is_reported_rather_than_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a_test.lua", "return 42");
        assert_ne!(run(dir.path(), &Policy::pure()), 0);
    }

    #[test]
    fn a_file_that_fails_to_compile_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a_test.lua", "this is not lua");
        assert_ne!(run(dir.path(), &Policy::pure()), 0);
    }

    #[test]
    fn a_single_file_can_be_named_directly() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "a_test.lua",
            "return { works = function() end }",
        );
        assert_eq!(run(&dir.path().join("a_test.lua"), &Policy::pure()), 0);
    }

    #[test]
    fn non_function_entries_are_ignored_rather_than_run() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "a_test.lua",
            "return { fixture = 'data', works = function() end }",
        );
        assert_eq!(run(dir.path(), &Policy::pure()), 0);
    }

    #[test]
    fn tests_run_under_the_policy_they_were_given() {
        // A suite testing `fs` needs grants; one testing pure logic should not silently get them.
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "a_test.lua",
            "return { denied = function()
               local ok = pcall(airsstack.fs.read, '/etc/hostname')
               assert(not ok, 'reading should have been refused')
             end }",
        );
        assert_eq!(run(dir.path(), &Policy::confined()), 0);
    }
}
