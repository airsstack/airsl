//! `.rs` paths cited in code spans, and whether each names a real file.
//!
//! Its own module because a path citation has a grammar (what a span must look like to count) and
//! a resolution rule (what the path is relative to) that no other citation shares, and because the
//! resolution rule is the part that decides whether the guard can fail at all: a path tried
//! against every directory in the workspace passes on any file of that name.
//!
//! Responsibilities:
//!
//! - Recognising a span that is wholly a `.rs` path, and a span that adds a line number to one.
//! - Refusing line-number citations, which go stale on the next edit above the cited line.
//! - Resolving a path against the citing document only: beside the document, in the document's
//!   own crate and that crate's `src`, or from the repository root when the path starts with
//!   `crates/`. A document outside every crate resolves against the repository root alone.
//!
//! Non-responsibilities: module paths (`a::b`) and book includes, which have their own files; and
//! whether a cited symbol exists in the file, which is prose the author keeps true.

use std::path::Path;

use walkdir::WalkDir;

use super::span::code_spans;
use crate::workspace::citing_crate;

/// What a code span cites, when it cites a source file.
#[derive(Debug, PartialEq, Eq)]
enum Citation<'a> {
    /// The whole span is a `.rs` path.
    Path(&'a str),
    /// The whole span is a `.rs` path followed by `:<line>` or `:<line>-<line>`.
    Line {
        /// The path, without the line suffix.
        path: &'a str,
    },
}

/// Whether `path` is a bare `.rs` path: path characters only, a non-empty stem, not absolute.
fn is_rs_path(path: &str) -> bool {
    let plain = path
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/'));
    let has_stem = path
        .strip_suffix(".rs")
        .is_some_and(|stem| !stem.is_empty());
    plain && has_stem && !path.starts_with('/')
}

/// Classifies a span. A glob, a URL, a command or any prose in backticks is not a citation.
fn parse(span: &str) -> Option<Citation<'_>> {
    if let Some((path, suffix)) = span.rsplit_once(':') {
        let is_line = suffix.starts_with(|c: char| c.is_ascii_digit())
            && suffix.bytes().all(|b| b.is_ascii_digit() || b == b'-');
        if is_line && is_rs_path(path) {
            return Some(Citation::Line { path });
        }
    }
    is_rs_path(span).then_some(Citation::Path(span))
}

/// Whether the crate directory holds a file called `name` anywhere beneath it.
fn crate_holds_file_named(krate: &Path, name: &str) -> bool {
    WalkDir::new(krate)
        .into_iter()
        .filter_map(Result::ok)
        .any(|entry| entry.file_type().is_file() && entry.file_name() == name)
}

/// Whether a cited path names a real file, and where it looked when it does not.
///
/// A path starting `crates/` is read from the repository root. Any other path is read against the
/// citing document's own crate: beside the document, at the crate root, or under the crate's
/// `src`; a bare file name may also be any file of that name inside the crate. A document outside
/// every crate reads every other path from the repository root, so a path into a crate must carry
/// its `crates/<name>/` prefix there.
fn resolve(cited: &str, doc: &Path, root: &Path) -> Result<(), String> {
    if cited.starts_with("crates/") {
        return if root.join(cited).is_file() {
            Ok(())
        } else {
            Err("looked at the repository root".to_owned())
        };
    }
    let Some(krate) = citing_crate(doc, root) else {
        return if root.join(cited).is_file() {
            Ok(())
        } else {
            Err(
                "looked at the repository root; a path into a crate needs its `crates/<name>/` prefix"
                    .to_owned(),
            )
        };
    };
    let beside = doc.parent().unwrap_or(&krate);
    let found = [beside, &krate, &krate.join("src")]
        .iter()
        .any(|base| base.join(cited).is_file())
        || (!cited.contains('/') && crate_holds_file_named(&krate, cited));
    if found {
        return Ok(());
    }
    let krate_name = krate.file_name().unwrap_or_default().to_string_lossy();
    Err(format!(
        "looked beside the document and in the `{krate_name}` crate, at its root and its `src`"
    ))
}

/// Every problem with the source paths cited in `text`, the content of `doc`, one line each.
///
/// A problem is a line-number citation, or a path that resolves to no file.
pub(super) fn problems(doc: &Path, text: &str, root: &Path) -> Vec<String> {
    let shown = doc.strip_prefix(root).unwrap_or(doc).display().to_string();
    let mut found = Vec::new();
    for span in code_spans(text) {
        match parse(&span.text) {
            None => {}
            Some(Citation::Line { path }) => found.push(format!(
                "{shown}:{}: cites `{}` with a line number, which goes stale on the next edit; \
                 name the symbol in prose beside the bare path `{path}`",
                span.line, span.text,
            )),
            Some(Citation::Path(cited)) => {
                if let Err(looked) = resolve(cited, doc, root) {
                    found.push(format!(
                        "{shown}:{}: cites `{cited}`, which does not exist ({looked})",
                        span.line,
                    ));
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "fixtures are known to be creatable; a panic is the failure signal"
    )]

    use super::*;

    use std::fs;

    use crate::workspace::{documents, root};

    #[test]
    fn a_span_is_a_citation_only_when_it_is_wholly_a_rs_path() {
        assert_eq!(
            parse("src/engine.rs"),
            Some(Citation::Path("src/engine.rs"))
        );
        assert_eq!(parse("main.rs"), Some(Citation::Path("main.rs")));
        assert_eq!(parse("**/*.rs"), None);
        assert_eq!(parse("*.rs"), None);
        assert_eq!(parse(".rs"), None);
        assert_eq!(parse("cargo test src/engine.rs"), None);
        assert_eq!(parse("/etc/hosts.rs"), None);
        assert_eq!(parse("Engine::eval"), None);
        assert_eq!(parse("src/engine.rs:"), None);
        assert_eq!(parse("src/engine.rs:abc"), None);
    }

    #[test]
    fn a_line_suffix_makes_the_span_a_line_citation() {
        assert_eq!(
            parse("src/error.rs:217"),
            Some(Citation::Line {
                path: "src/error.rs"
            })
        );
        assert_eq!(
            parse("src/error.rs:12-30"),
            Some(Citation::Line {
                path: "src/error.rs"
            })
        );
    }

    /// A two-crate workspace: `a` holds `src/engine.rs`, `src/modules/fs.rs` and an example, `b`
    /// holds `src/only_in_b.rs`.
    fn workspace() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        for rel in [
            "crates/a/src/engine.rs",
            "crates/a/src/modules/fs.rs",
            "crates/a/examples/demo/main.rs",
            "crates/a/examples/demo/README.md",
            "crates/b/src/only_in_b.rs",
            "crates/b/README.md",
        ] {
            let path = root.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        tmp
    }

    fn check(rel_doc: &str, cited: &str) -> Vec<String> {
        let tmp = workspace();
        let root = tmp.path();
        problems(&root.join(rel_doc), &format!("See `{cited}`.\n"), root)
    }

    #[test]
    fn a_crate_document_resolves_beside_itself_and_in_its_own_crate() {
        let doc = "crates/a/examples/demo/README.md";
        for cited in [
            "main.rs",
            "src/engine.rs",
            "modules/fs.rs",
            "engine.rs",
            "fs.rs",
            "crates/a/src/engine.rs",
        ] {
            assert_eq!(check(doc, cited), Vec::<String>::new(), "{cited}");
        }
    }

    #[test]
    fn a_crate_document_does_not_resolve_into_another_crate() {
        let doc = "crates/b/README.md";
        for cited in ["src/engine.rs", "engine.rs", "modules/fs.rs", "main.rs"] {
            let found = check(doc, cited);
            assert_eq!(found.len(), 1, "{cited}: {found:?}");
            assert!(found[0].contains("does not exist"), "{found:?}");
        }
        assert!(check(doc, "src/only_in_b.rs").is_empty());
    }

    #[test]
    fn a_root_document_needs_the_crate_prefix() {
        assert!(check("CLAUDE.md", "crates/a/src/engine.rs").is_empty());
        for cited in ["src/engine.rs", "engine.rs", "crates/a/src/gone.rs"] {
            let found = check("CLAUDE.md", cited);
            assert_eq!(found.len(), 1, "{cited}: {found:?}");
            assert!(found[0].starts_with("CLAUDE.md:1: cites"), "{found:?}");
        }
        assert!(check("CLAUDE.md", "src/engine.rs")[0].contains("`crates/<name>/` prefix"));
    }

    #[test]
    fn a_line_citation_fails_even_when_the_file_exists() {
        let found = check("crates/a/README.md", "src/engine.rs:12");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("with a line number"), "{found:?}");
        assert!(found[0].contains("name the symbol"), "{found:?}");
        assert!(found[0].contains("`src/engine.rs`"), "{found:?}");
    }

    #[test]
    fn fenced_blocks_are_not_read_for_citations() {
        let tmp = workspace();
        let root = tmp.path();
        let text = "```\n`src/gone.rs`\n```\n";
        assert!(problems(&root.join("CLAUDE.md"), text, root).is_empty());
    }

    #[test]
    fn every_source_path_cited_in_the_docs_exists_and_none_names_a_line() {
        let root = root();
        let docs = documents(&root);
        assert!(
            docs.first().is_some_and(|claude| claude.is_file()),
            "{} is missing, so the repository's own citations went unchecked",
            root.join("CLAUDE.md").display(),
        );
        let found: Vec<String> = docs
            .iter()
            .filter(|doc| doc.is_file())
            .flat_map(|doc| {
                let text = fs::read_to_string(doc).unwrap();
                problems(doc, &text, &root)
            })
            .collect();
        assert!(
            found.is_empty(),
            "stale source citations:\n{}",
            found.join("\n")
        );
    }
}
