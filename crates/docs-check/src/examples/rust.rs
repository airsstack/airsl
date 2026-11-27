//! The Rust examples of the `airsl` crate against their READMEs.
//!
//! Its own module because an example README's `## Output` block is a promise about what a program
//! prints, and the only way to keep it is to run the program: this runs each example of the `airsl`
//! crate and requires the block to equal what it printed, so rewording a message or changing a
//! number names the example that drifted.
//!
//! A passing test prints nothing a reader sees, so an excused example cannot be reported by name
//! as it is skipped. Instead the excusals are an explicit list in this file (`EXPECTED_NO_OUTPUT`
//! and `EXPECTED_UNIX_ONLY`) that `every_marker_file_is_named_in_its_expected_list` asserts the
//! markers on disk against, on every platform. Adding a marker without adding its name here fails
//! that test, so an excusal is always a visible edit.
//!
//! The inner `cargo run` takes its build from the test binary's own path: its profile, its target
//! triple when the outer run was cross-directed, and the target directory, which is the nearest
//! ancestor of the binary that holds a `CACHEDIR.TAG`. Under `build.build-dir` that ancestor is the
//! build directory rather than the target directory, and the inner run is given that one. The inner
//! build is therefore the outer build: the same profile, triple and target directory. Without that,
//! a `cargo test --release` or `--target-dir` run would quietly build a second tree, and an example
//! would be compared against the wrong build. Flags that cargo does not expose to a test, namely
//! the outer run's `--locked`, `--offline` and `--frozen`, are not forwarded; their environment
//! forms, such as `CARGO_NET_OFFLINE`, are inherited by the child process like any other variable,
//! and the outer run has already resolved the lockfile the inner run reads.
//!
//! Responsibilities:
//!
//! - Running each example and comparing its stdout with its README's `## Output` block for the
//!   current platform, line endings folded.
//! - Requiring every example to have a block or an explicit `no-output` marker.
//! - Asserting the marked examples against the expected lists.
//! - Keeping the `examples` task of `Makefile.toml` from comparing an example's output with README
//!   text, which this module alone does.
//! - Deriving the outer build's flags from the test binary's own path.
//!
//! Non-responsibilities: the Lua transcripts of the book appendices and compiling the shipped
//! `.lua` files, which stay with `cargo make examples`; running an example with the invoking
//! shell's stdin, which `cargo make examples` does and this does not; and the Lua examples of the
//! native module, which `lua` owns.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::readme::{Expectation, Platform, expectation, marked, unlisted_markers};
use crate::workspace::normalize;

/// The examples whose directory holds a `no-output` marker, sorted. Adding one is a decision to
/// stop checking that example's output, so it is written down here.
const EXPECTED_NO_OUTPUT: [&str; 0] = [];

/// The examples whose directory holds a `unix-only` marker, sorted: they do not run on Windows,
/// where `sh` is not present.
const EXPECTED_UNIX_ONLY: [&str; 2] = ["denials-are-data", "env-and-proc"];

/// The lines of a `Makefile.toml` that call `assert_documented`, the duckscript containment check
/// of the `examples` task, with a target other than the book appendix.
///
/// The gate compares each README's `## Output` block with what its example prints, exactly, here.
/// The `examples` task used to repeat that comparison by containment, which cannot hold an example
/// that prints nothing and so contradicted the `no-output` marker. Nothing runs that task on a
/// fixture, so this reads its text: the appendix is the only file it may still compare.
fn containment_outside_the_appendix(makefile: &str) -> Vec<&str> {
    makefile
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#') && !line.starts_with("function "))
        .filter(|line| line.contains("assert_documented") && !line.contains("${appendix}"))
        .collect()
}

/// How the outer `cargo test` was built, as far as an inner `cargo run` must match it.
#[derive(Debug, PartialEq, Eq)]
struct OuterBuild {
    /// The target directory, the one holding `CACHEDIR.TAG`.
    target_dir: PathBuf,
    /// The `--target` triple, when the outer build was cross-directed, else `None`.
    triple: Option<String>,
    /// The cargo profile name, as `--profile` spells it (`dev`, not `debug`).
    profile: String,
}

/// Reads the outer build off the path of the test binary, which cargo places at
/// `<target_dir>/[<triple>/]<profile dir>/deps/<name>`.
///
/// `None` when the path is not under `target_dir` in one of those two shapes.
fn outer_build(exe: &Path, target_dir: &Path) -> Option<OuterBuild> {
    let relative = exe.strip_prefix(target_dir).ok()?;
    let parts: Vec<&str> = relative
        .components()
        .map(|part| part.as_os_str().to_str())
        .collect::<Option<_>>()?;
    let (triple, profile_dir) = match parts.as_slice() {
        [profile, "deps", _] => (None, *profile),
        [triple, profile, "deps", _] => (Some((*triple).to_owned()), *profile),
        _ => return None,
    };
    // Cargo names the `dev` profile's directory `debug`; every other profile uses its own name.
    let profile = if profile_dir == "debug" {
        "dev"
    } else {
        profile_dir
    };
    Some(OuterBuild {
        target_dir: target_dir.to_path_buf(),
        triple,
        profile: profile.to_owned(),
    })
}

/// The outer build of the running test binary.
fn current_outer_build() -> Result<OuterBuild, String> {
    let exe = std::env::current_exe()
        .map_err(|error| format!("cannot find the running test binary: {error}"))?;
    let target_dir = exe
        .ancestors()
        .find(|dir| dir.join("CACHEDIR.TAG").is_file())
        .ok_or_else(|| {
            format!(
                "no ancestor of {} holds a CACHEDIR.TAG, so the target directory is unknown",
                exe.display()
            )
        })?;
    outer_build(&exe, target_dir).ok_or_else(|| {
        format!(
            "{} is not at <target dir>/[<triple>/]<profile>/deps/<name>, so the outer profile is unknown",
            exe.display()
        )
    })
}

/// The `cargo run` that runs one example of `airsl` inside the outer build.
fn example_command(cargo: &Path, build: &OuterBuild, root: &Path, name: &str) -> Command {
    let mut command = Command::new(cargo);
    command
        .args(["run", "-q", "-p", "airsl", "--example", name])
        .arg("--profile")
        .arg(&build.profile)
        .arg("--target-dir")
        .arg(&build.target_dir);
    if let Some(triple) = &build.triple {
        command.arg("--target").arg(triple);
    }
    command.current_dir(root);
    command
}

/// The example directories that hold a `main.rs`, sorted. Cargo ignores a directory without one.
fn example_dirs(examples: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(examples) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("main.rs").is_file())
        .collect();
    dirs.sort();
    dirs
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "fixtures and the checkout under test are known to exist; a panic is the failure signal"
    )]

    use super::*;

    use crate::workspace::root;

    #[test]
    fn the_outer_build_is_read_off_the_test_binary_path() {
        let target = Path::new("/w/target");
        let build = |exe: &str| outer_build(Path::new(exe), target);
        assert_eq!(
            build("/w/target/debug/deps/docs_check-ab12"),
            Some(OuterBuild {
                target_dir: target.to_path_buf(),
                triple: None,
                profile: "dev".to_owned()
            })
        );
        assert_eq!(
            build("/w/target/release/deps/docs_check-ab12").map(|b| b.profile),
            Some("release".to_owned())
        );
        assert_eq!(
            build("/w/target/ci-fast/deps/docs_check-ab12").map(|b| b.profile),
            Some("ci-fast".to_owned())
        );
        let cross = build("/w/target/aarch64-apple-darwin/release/deps/docs_check-ab12").unwrap();
        assert_eq!(cross.triple.as_deref(), Some("aarch64-apple-darwin"));
        assert_eq!(cross.profile, "release");
        assert_eq!(build("/w/target/debug/docs_check"), None);
        assert_eq!(outer_build(Path::new("/elsewhere/deps/x"), target), None);
    }

    #[test]
    fn the_inner_run_carries_the_outer_target_dir_profile_and_triple() {
        let build = OuterBuild {
            target_dir: PathBuf::from("/w/target"),
            triple: Some("aarch64-apple-darwin".to_owned()),
            profile: "release".to_owned(),
        };
        let command = example_command(Path::new("/c/cargo"), &build, Path::new("/w"), "hello");
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            args,
            [
                "run",
                "-q",
                "-p",
                "airsl",
                "--example",
                "hello",
                "--profile",
                "release",
                "--target-dir",
                "/w/target",
                "--target",
                "aarch64-apple-darwin"
            ]
        );
        assert_eq!(command.get_program(), "/c/cargo");
    }

    #[test]
    fn only_directories_with_a_main_rs_are_examples() {
        let tmp = tempfile::tempdir().unwrap();
        let examples = tmp.path();
        for rel in ["b/main.rs", "a/main.rs", "notes/setup.rs"] {
            let path = examples.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        let names: Vec<_> = example_dirs(examples)
            .iter()
            .map(|dir| dir.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a", "b"]);
    }

    #[test]
    fn every_marker_file_is_named_in_its_expected_list() {
        let dirs = example_dirs(&root().join("crates/airsl/examples"));
        assert!(
            !dirs.is_empty(),
            "no example directory found under crates/airsl/examples"
        );
        let report = unlisted_markers(&marked(&dirs), &EXPECTED_NO_OUTPUT, &EXPECTED_UNIX_ONLY);
        assert!(report.is_empty(), "{report}");
    }

    #[test]
    fn every_example_readme_output_block_equals_what_the_example_prints() {
        let root = root();
        let cargo = PathBuf::from(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
        let build = current_outer_build().unwrap();
        let dirs = example_dirs(&root.join("crates/airsl/examples"));
        assert!(
            !dirs.is_empty(),
            "no example directory found under crates/airsl/examples"
        );

        let mut failures = String::new();
        let mut checked = 0_usize;
        for dir in dirs {
            let name = dir.file_name().unwrap().to_string_lossy().into_owned();
            let expected = match expectation(&dir, Platform::current()) {
                Expectation::Output(block) => block,
                Expectation::Exempt => continue,
                Expectation::Unstated(why) => {
                    let _ = writeln!(failures, "example `{name}`: {why}");
                    continue;
                }
            };
            // A marker file named `unix-only` holds the reason an example cannot run on Windows.
            if cfg!(windows) && dir.join("unix-only").is_file() {
                continue;
            }
            let mut command = example_command(&cargo, &build, &root, &name);
            let ran = match command.output() {
                Ok(ran) => ran,
                Err(error) => {
                    let _ = writeln!(
                        failures,
                        "example `{name}`: could not run `{command:?}`: {error}"
                    );
                    continue;
                }
            };
            checked += 1;
            if !ran.status.success() {
                let _ = write!(
                    failures,
                    "example `{name}` exited with {} (`{command:?}`)\n--- stderr\n{}\n",
                    ran.status,
                    String::from_utf8_lossy(&ran.stderr),
                );
                continue;
            }
            let actual = normalize(&String::from_utf8_lossy(&ran.stdout));
            let actual = actual.trim_end_matches('\n');
            if actual != expected {
                let _ = write!(
                    failures,
                    "example `{name}`: its output differs from the `## Output` block for {:?} in {}\n--- README shows\n{expected}\n--- example printed\n{actual}\n",
                    Platform::current(),
                    dir.join("README.md").display(),
                );
            }
        }
        assert!(checked > 0, "no example README has an `## Output` block");
        assert!(failures.is_empty(), "README drift:\n{failures}");
    }

    #[test]
    fn a_containment_check_of_anything_but_the_appendix_is_found_in_a_makefile() {
        let makefile = "# assert_documented in a comment\nfunction assert_documented\n    return 1\nend\n\
            documented = assert_documented \"a\" ${ran.stdout} ${appendix}\n\
            documented = assert_documented ${name} ${ran.stdout} ${dir}/README.md\n";
        assert_eq!(
            containment_outside_the_appendix(makefile),
            ["documented = assert_documented ${name} ${ran.stdout} ${dir}/README.md"]
        );
    }

    #[test]
    fn the_examples_task_compares_only_the_appendix_with_containment() {
        let makefile = fs::read_to_string(root().join("Makefile.toml")).unwrap();
        assert!(
            makefile.contains("= assert_documented "),
            "Makefile.toml no longer calls assert_documented, so this guard reads nothing"
        );
        assert_eq!(
            containment_outside_the_appendix(&makefile),
            Vec::<&str>::new(),
            "the examples task compares output with something other than the appendix; the gate's docs-check test owns README output blocks"
        );
    }
}
