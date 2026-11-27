//! Example READMEs against the examples themselves.
//!
//! Its own module because an example README's `## Output` block is a promise about what a program
//! prints, and the only way to keep it is to run the program: this runs each example of the `airsl`
//! crate and requires the block to equal what it printed, so rewording a message or changing a
//! number names the example that drifted.
//!
//! Every example directory that holds a `main.rs` must state its expectation, so that renaming a
//! heading cannot silently drop an example from the check. It has either a README with an
//! `## Output` block, or a marker file named `no-output` whose text is the reason there is none.
//! A marker file named `unix-only` excuses an example from running on Windows.
//!
//! An example whose output names something each platform spells its own way, such as the
//! filesystem root, keeps the block every other platform prints first and adds a `### Windows`
//! sub-heading inside `## Output` whose first block is what Windows prints instead. Windows is the
//! one platform that differs because a path there starts with a drive, while Linux and macOS share
//! the unix spelling, so one heading covers every case the examples have; a Windows block with no
//! shared block before it is refused, so no platform is left without an expectation.
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
//! `.lua` files, which stay with `cargo make examples`; and running an example with the invoking
//! shell's stdin, which `cargo make examples` does and this does not.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// The names of the example directories that carry each marker, sorted.
#[derive(Debug, PartialEq, Eq)]
struct Marked {
    /// Examples with a `no-output` marker.
    no_output: Vec<String>,
    /// Examples with a `unix-only` marker.
    unix_only: Vec<String>,
}

/// Reads which of `dirs` carry each marker.
fn marked(dirs: &[PathBuf]) -> Marked {
    let named = |marker: &str| -> Vec<String> {
        let mut names: Vec<String> = dirs
            .iter()
            .filter(|dir| dir.join(marker).is_file())
            .filter_map(|dir| dir.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    Marked {
        no_output: named("no-output"),
        unix_only: named("unix-only"),
    }
}

/// Where `found` differs from the expected lists, one line per marker kind; empty when it does
/// not.
fn unlisted_markers(found: &Marked, no_output: &[&str], unix_only: &[&str]) -> String {
    let mut report = String::new();
    for (kind, found, expected) in [
        ("no-output", &found.no_output, no_output),
        ("unix-only", &found.unix_only, unix_only),
    ] {
        if found
            .iter()
            .map(String::as_str)
            .ne(expected.iter().copied())
        {
            let _ = writeln!(
                report,
                "examples with a `{kind}` marker are {found:?}, but the expected list is {expected:?}; update the list in examples.rs together with the marker"
            );
        }
    }
    report
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

/// The platform an example runs on, as far as which output block it is held to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Platform {
    /// Windows, held to a `### Windows` block when the README has one.
    Windows,
    /// Every other platform, held to the shared block.
    Other,
}

impl Platform {
    /// The platform this test binary was built for.
    const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Other
        }
    }
}

/// The fenced blocks of a README's `## Output` section, each without its trailing newlines.
#[derive(Debug, Default, PartialEq, Eq)]
struct OutputBlocks {
    /// The first fence outside a `### Windows` sub-section: what every platform prints, Windows
    /// included when it has no block of its own.
    shared: Option<String>,
    /// The first fence inside a `### Windows` sub-section: what Windows prints instead.
    windows: Option<String>,
}

/// The blocks of the `## Output` section, which ends at the next first- or second-level heading.
///
/// Both are `None` when the document has no such heading, or no fence before the section ends. A
/// `###` heading other than `### Windows` ends a Windows sub-section, so a fence under it is read
/// as shared.
fn output_blocks(readme: &str) -> OutputBlocks {
    let text = normalize(readme);
    let mut blocks = OutputBlocks::default();
    let mut lines = text.lines();
    if !lines.any(|line| line.trim_end() == "## Output") {
        return blocks;
    }
    let mut in_windows = false;
    while let Some(line) = lines.next() {
        if line.starts_with("```") {
            // The body is consumed here, so a `#` line inside a fence is never read as a heading.
            let body: Vec<&str> = lines
                .by_ref()
                .take_while(|line| !line.starts_with("```"))
                .collect();
            let slot = if in_windows {
                &mut blocks.windows
            } else {
                &mut blocks.shared
            };
            if slot.is_none() {
                *slot = Some(body.join("\n").trim_end_matches('\n').to_owned());
            }
        } else if line.starts_with("# ") || line.starts_with("## ") {
            break;
        } else if let Some(heading) = line.strip_prefix("### ") {
            in_windows = heading.trim_end() == "Windows";
        }
    }
    blocks
}

/// What an example directory promises about its output.
#[derive(Debug, PartialEq, Eq)]
enum Expectation {
    /// The README's `## Output` block for this platform, which the example must print exactly.
    Output(String),
    /// A `no-output` marker excuses the example from having a block.
    Exempt,
    /// The directory states nothing checkable; the string says why that fails.
    Unstated(String),
}

/// Reads an example directory's expectation on `platform`.
fn expectation(dir: &Path, platform: Platform) -> Expectation {
    let readme = fs::read_to_string(dir.join("README.md")).ok();
    let blocks = readme.as_deref().map(output_blocks).unwrap_or_default();
    let marked = dir.join("no-output").is_file();
    match (blocks, marked, readme.is_some()) {
        (
            OutputBlocks {
                shared: None,
                windows: None,
            },
            true,
            _,
        ) => Expectation::Exempt,
        (_, true, _) => Expectation::Unstated(
            "it has a `no-output` marker and an `## Output` block; keep one".to_owned(),
        ),
        (
            OutputBlocks {
                shared: Some(shared),
                windows,
            },
            false,
            _,
        ) => Expectation::Output(match platform {
            Platform::Windows => windows.unwrap_or(shared),
            Platform::Other => shared,
        }),
        // Refused on every platform, not only where the shared block would be read, so a README
        // that cannot be checked off Windows fails on the platform its author ran it on.
        (
            OutputBlocks {
                shared: None,
                windows: Some(_),
            },
            false,
            _,
        ) => Expectation::Unstated(
            "its `## Output` section has a `### Windows` block but no block before it for the other platforms"
                .to_owned(),
        ),
        (_, false, true) => Expectation::Unstated(
            "its README has no `## Output` block (add one, or a `no-output` marker file holding the reason)"
                .to_owned(),
        ),
        (_, false, false) => Expectation::Unstated(
            "it has no README.md (add one with an `## Output` block, or a `no-output` marker file holding the reason)"
                .to_owned(),
        ),
    }
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
    fn output_block_is_the_first_fence_under_the_heading() {
        let readme = "# t\n\n## Run\n\n```bash\ncargo run\n```\n\n## Output\n\n```\nline one\n\nline three\n```\n\n## Next\n\n```\nnot this\n```\n";
        assert_eq!(
            output_blocks(readme).shared.as_deref(),
            Some("line one\n\nline three")
        );
    }

    #[test]
    fn a_windows_block_is_read_apart_from_the_shared_one_and_a_fenced_heading_is_not_a_heading() {
        let readme = "## Output\n\n```\nunix\n## not a heading\n```\n\nprose\n\n```\nsecond\n```\n\n\
            ### Windows\n\nOn Windows:\n\n```\nwindows\n```\n\n### Notes\n\n```\nnotes\n```\n\n## Next\n";
        assert_eq!(
            output_blocks(readme),
            OutputBlocks {
                shared: Some("unix\n## not a heading".to_owned()),
                windows: Some("windows".to_owned()),
            }
        );
    }

    #[test]
    fn a_fence_under_another_sub_heading_after_windows_is_shared() {
        let readme =
            "## Output\n\n### Windows\n\n```\nwindows\n```\n\n### Unix\n\n```\nunix\n```\n";
        assert_eq!(
            output_blocks(readme),
            OutputBlocks {
                shared: Some("unix".to_owned()),
                windows: Some("windows".to_owned()),
            }
        );
    }

    #[test]
    fn output_block_ignores_crlf_line_endings() {
        let lf = "## Output\n\n```\na\nb\n```\n";
        let crlf = lf.replace('\n', "\r\n");
        assert_eq!(output_blocks(&crlf), output_blocks(lf));
        assert_eq!(output_blocks(&crlf).shared.as_deref(), Some("a\nb"));
    }

    #[test]
    fn output_block_is_absent_without_a_heading_or_before_the_next_heading() {
        assert_eq!(
            output_blocks("# t\n\n```\nx\n```\n"),
            OutputBlocks::default()
        );
        assert_eq!(
            output_blocks("## Output\n\nprose only\n\n## Next\n\n```\nx\n```\n"),
            OutputBlocks::default()
        );
    }

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
    fn an_example_directory_must_state_its_output_or_say_it_has_none() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        assert!(
            matches!(expectation(dir, Platform::Other), Expectation::Unstated(why) if why.contains("no README.md"))
        );

        fs::write(dir.join("README.md"), "# t\n\n## Output\n\nprose only\n").unwrap();
        assert!(
            matches!(expectation(dir, Platform::Other), Expectation::Unstated(why) if why.contains("no `## Output` block"))
        );

        fs::write(dir.join("no-output"), "prints nothing").unwrap();
        assert_eq!(expectation(dir, Platform::Other), Expectation::Exempt);

        fs::write(dir.join("README.md"), "## Output\n\n```\nhi\n```\n").unwrap();
        assert!(
            matches!(expectation(dir, Platform::Other), Expectation::Unstated(why) if why.contains("keep one"))
        );

        fs::remove_file(dir.join("no-output")).unwrap();
        assert_eq!(
            expectation(dir, Platform::Other),
            Expectation::Output("hi".to_owned())
        );
    }

    #[test]
    fn windows_is_held_to_its_own_block_and_falls_back_to_the_shared_one() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("README.md"), "## Output\n\n```\n/\n```\n").unwrap();
        assert_eq!(
            expectation(dir, Platform::Windows),
            Expectation::Output("/".to_owned())
        );

        fs::write(
            dir.join("README.md"),
            "## Output\n\n```\n/\n```\n\n### Windows\n\n```\nC:/\n```\n",
        )
        .unwrap();
        assert_eq!(
            expectation(dir, Platform::Windows),
            Expectation::Output("C:/".to_owned())
        );
        assert_eq!(
            expectation(dir, Platform::Other),
            Expectation::Output("/".to_owned())
        );
    }

    #[test]
    fn a_windows_block_alone_is_refused_on_every_platform_and_counts_against_a_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(
            dir.join("README.md"),
            "## Output\n\n### Windows\n\n```\nC:/\n```\n",
        )
        .unwrap();
        for platform in [Platform::Windows, Platform::Other] {
            assert!(
                matches!(expectation(dir, platform), Expectation::Unstated(why) if why.contains("no block before it")),
                "{platform:?}"
            );
        }

        fs::write(dir.join("no-output"), "prints nothing").unwrap();
        assert!(
            matches!(expectation(dir, Platform::Windows), Expectation::Unstated(why) if why.contains("keep one"))
        );
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
    fn the_marked_examples_are_read_from_the_marker_files_and_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs: Vec<PathBuf> = ["b", "a", "c"]
            .iter()
            .map(|name| {
                let dir = tmp.path().join(name);
                fs::create_dir_all(&dir).unwrap();
                dir
            })
            .collect();
        fs::write(dirs[0].join("unix-only"), "no sh").unwrap();
        fs::write(dirs[1].join("unix-only"), "no sh").unwrap();
        fs::write(dirs[2].join("no-output"), "prints nothing").unwrap();
        assert_eq!(
            marked(&dirs),
            Marked {
                no_output: vec!["c".to_owned()],
                unix_only: vec!["a".to_owned(), "b".to_owned()],
            }
        );
    }

    #[test]
    fn a_marker_missing_from_the_expected_list_or_the_reverse_is_reported() {
        let found = Marked {
            no_output: vec!["quiet".to_owned()],
            unix_only: vec!["a".to_owned(), "b".to_owned()],
        };
        assert_eq!(unlisted_markers(&found, &["quiet"], &["a", "b"]), "");
        let added = unlisted_markers(&found, &[], &["a", "b"]);
        assert!(
            added.contains("`no-output` marker are [\"quiet\"]"),
            "{added}"
        );
        let dropped = unlisted_markers(&found, &["quiet"], &["a", "b", "c"]);
        assert!(dropped.contains("`unix-only` marker"), "{dropped}");
        assert!(!dropped.contains("`no-output`"), "{dropped}");
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
