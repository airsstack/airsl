//! What an example directory promises about its output: its README's `## Output` block and its
//! marker files.
//!
//! Its own module because the Rust examples of `airsl` and the Lua examples of the native module
//! make the same promise in the same words, and a rule written twice drifts: both read the block,
//! the `### Windows` block, and the `no-output` and `unix-only` markers through the functions here.
//!
//! An example whose output names something each platform spells its own way, such as the
//! filesystem root, keeps the block every other platform prints first and adds a `### Windows`
//! sub-heading inside `## Output` whose first block is what Windows prints instead. Windows is the
//! one platform that differs because a path there starts with a drive, while Linux and macOS share
//! the unix spelling, so one heading covers every case the examples have; a Windows block with no
//! shared block before it is refused, so no platform is left without an expectation.
//!
//! Every example directory must state its expectation, so that renaming a heading cannot silently
//! drop an example from the check. It has either a README with an `## Output` block, or a marker
//! file named `no-output` whose text is the reason there is none. A marker file named `unix-only`
//! excuses an example from running on Windows.
//!
//! Responsibilities:
//!
//! - Reading the `## Output` blocks of a README, line endings folded.
//! - Turning a directory into an expectation for one platform: a block, an excusal, or the reason
//!   it states nothing checkable.
//! - Reading which directories carry each marker, and reporting where that differs from an expected
//!   list.
//!
//! Non-responsibilities: running anything. Which examples exist and how each is run belongs to
//! `rust` and `lua`.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::workspace::normalize;

/// The platform an example runs on, as far as which output block it is held to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Platform {
    /// Windows, held to a `### Windows` block when the README has one.
    Windows,
    /// Every other platform, held to the shared block.
    Other,
}

impl Platform {
    /// The platform this test binary was built for.
    pub(super) const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Other
        }
    }
}

/// The fenced blocks of a README's `## Output` section, each without its trailing newlines.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct OutputBlocks {
    /// The first fence outside a `### Windows` sub-section: what every platform prints, Windows
    /// included when it has no block of its own.
    pub(super) shared: Option<String>,
    /// The first fence inside a `### Windows` sub-section: what Windows prints instead.
    pub(super) windows: Option<String>,
}

/// The blocks of the `## Output` section, which ends at the next first- or second-level heading.
///
/// Both are `None` when the document has no such heading, or no fence before the section ends. A
/// `###` heading other than `### Windows` ends a Windows sub-section, so a fence under it is read
/// as shared.
pub(super) fn output_blocks(readme: &str) -> OutputBlocks {
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
pub(super) enum Expectation {
    /// The README's `## Output` block for this platform, which the example must print exactly.
    Output(String),
    /// A `no-output` marker excuses the example from having a block.
    Exempt,
    /// The directory states nothing checkable; the string says why that fails.
    Unstated(String),
}

/// Reads an example directory's expectation on `platform`.
pub(super) fn expectation(dir: &Path, platform: Platform) -> Expectation {
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

/// The names of the example directories that carry each marker, sorted.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Marked {
    /// Examples with a `no-output` marker.
    pub(super) no_output: Vec<String>,
    /// Examples with a `unix-only` marker.
    pub(super) unix_only: Vec<String>,
}

/// Reads which of `dirs` carry each marker.
pub(super) fn marked(dirs: &[PathBuf]) -> Marked {
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
pub(super) fn unlisted_markers(found: &Marked, no_output: &[&str], unix_only: &[&str]) -> String {
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
                "examples with a `{kind}` marker are {found:?}, but the expected list is {expected:?}; update the expected list in the module that owns the examples together with the marker"
            );
        }
    }
    report
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "fixtures are known to be writable; a panic is the failure signal"
    )]

    use super::*;

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
}
