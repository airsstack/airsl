//! The links into the published book that the shipped sources and documents carry.
//!
//! Its own module because the book's address is a promise made from outside the book: the CLI
//! prints a chapter's URL, a README links to another, and renaming or unlisting that chapter breaks
//! each link without a build, a test or a reviewer noticing. `mdbook build` cannot see a link in a
//! `.rs` file, and the other guards read paths and modules, not addresses.
//!
//! Responsibilities:
//!
//! - Finding every `https://airsstack.github.io/airsl/…` address in the `.rs` files under
//!   `crates/` (this crate's own excepted), the repository `CLAUDE.md`, `README.md` and
//!   `CHANGELOG.md`, and every crate README.
//! - Requiring a `….html` address to name a chapter that exists in `book/src/` and that
//!   `book/src/SUMMARY.md` lists, since `mdbook` publishes only what the summary lists.
//! - Requiring a `#anchor` on such an address to be the id of a heading of that chapter, derived
//!   as `mdbook` derives it: tags and the link destinations dropped, repeated headings numbered
//!   `-1`, `-2`.
//! - Accepting the site root and `index.html` (an anchor on either is checked against the first
//!   chapter listed) and the generated `print.html` and `404.html`, and requiring any other
//!   address to name a file of `install/`, the wrappers the site serves beside the book.
//! - Reporting a scanned file or a chapter that exists and cannot be read.
//!
//! Non-responsibilities: whether the book builds, which is the `mdbook build` step of CI; a
//! heading given an explicit id (`{#id}`) or written with an HTML entity, whose id this does not
//! derive; and whether the install wrappers behave.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::workspace::{self, normalize};

/// The address every checked link starts with, up to and including the slash after the project
/// name.
const SITE: &str = "https://airsstack.github.io/airsl/";

/// The summary file, relative to the repository root. `mdbook` publishes the chapters it lists.
const SUMMARY: &str = "book/src/SUMMARY.md";

/// The chapter directory of the book, relative to the repository root.
const BOOK_SRC: &str = "book/src";

/// The pages `mdbook` generates besides the chapters. Their anchors are not the chapters' ids, so
/// an address to one is accepted whole.
const GENERATED_PAGES: [&str; 2] = ["print.html", "404.html"];

/// The directory whose files the site serves at its root, relative to the repository root.
const INSTALL_DIR: &str = "install";

/// What follows [`SITE`] in an address: the characters of a path and an anchor.
///
/// Anything else ends the address, which keeps a closing quote, bracket or backtick out of it.
fn is_address_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | '#')
}

/// One address found in a text: the 1-based line it is on and everything after [`SITE`].
#[derive(Debug, PartialEq, Eq)]
struct Link<'a> {
    /// 1-based line of the address.
    line: usize,
    /// The path and optional `#anchor` after the site address; empty for the site root.
    rest: &'a str,
}

/// Every site address in `text`, in order. A full stop that closes a sentence is not part of the
/// address.
fn links(text: &str) -> Vec<Link<'_>> {
    text.match_indices(SITE)
        .map(|(start, _)| {
            let after = &text[start + SITE.len()..];
            let end = after
                .find(|ch: char| !is_address_char(ch))
                .unwrap_or(after.len());
            Link {
                line: text[..start].matches('\n').count() + 1,
                rest: after[..end].trim_end_matches('.'),
            }
        })
        .collect()
}

/// The chapter files `SUMMARY.md` lists, as paths relative to `book/src`, in order. A link with no
/// target (a draft chapter) is not a listing.
fn listed_chapters(summary: &str) -> Vec<String> {
    let mut chapters = Vec::new();
    for (index, _) in summary.match_indices("](") {
        let after = &summary[index + 2..];
        let Some(close) = after.find(')') else {
            continue;
        };
        let target = after[..close].trim().trim_start_matches("./");
        if !target.is_empty() && !target.contains("://") {
            chapters.push(target.to_owned());
        }
    }
    chapters
}

/// The id `mdbook` gives a heading with this text before it numbers repeats: letters, digits, `_`
/// and `-` kept (ASCII letters lowercased), whitespace as `-`, everything else dropped.
fn slug(heading: &str) -> String {
    heading
        .chars()
        .filter_map(|ch| {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                Some(ch.to_ascii_lowercase())
            } else if ch.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

/// The heading text without the destinations of its links, which `mdbook` does not render. Other
/// markup characters are dropped by [`slug`] anyway.
fn visible_text(heading: &str) -> String {
    let mut shown = String::new();
    let mut rest = heading;
    while let Some(ch) = rest.chars().next() {
        let closes_link = ch == ']' && rest[1..].starts_with('(');
        if let (true, Some(close)) = (closes_link, rest.find(')')) {
            rest = &rest[close + 1..];
            continue;
        }
        shown.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    shown
}

/// The heading text without a closing run of `#`, which `CommonMark` ends a heading with only when
/// a space precedes it (`## Closed ##`, but `## C#` keeps its `#`).
fn without_closing_hashes(text: &str) -> &str {
    let stripped = text.trim_end_matches('#');
    if stripped.len() < text.len() && (stripped.is_empty() || stripped.ends_with(' ')) {
        stripped.trim_end()
    } else {
        text
    }
}

/// The heading text without inline HTML tags, which `mdbook` slugs from the rendered heading and
/// so never sees. A tag starts with `<` and a letter, `/`, `!` or `?` and ends at the next `>`; any
/// other `<` is text, which [`slug`] drops, and so is a `<` with no `>` after it.
fn without_tags(text: &str) -> String {
    let mut kept = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        let opens_tag = rest[start + 1..]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || matches!(ch, '/' | '!' | '?'));
        let end = rest[start..].find('>').filter(|_| opens_tag);
        if let Some(end) = end {
            kept.push_str(&rest[..start]);
            rest = &rest[start + end + 1..];
        } else {
            kept.push_str(&rest[..=start]);
            rest = &rest[start + 1..];
        }
    }
    kept.push_str(rest);
    kept
}

/// The id `mdbook` gives a heading written as `text`, before it numbers repeats.
fn heading_slug(text: &str) -> String {
    slug(&without_tags(&visible_text(text)))
}

/// How many columns a line is indented, a tab counting as four.
fn indent_of(line: &str) -> usize {
    line.chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .map(|ch| if ch == '\t' { 4 } else { 1 })
        .sum()
}

/// The fence a line opens: its character and length, when it is three or more backticks or
/// tildes.
fn fence_opened_by(trimmed: &str) -> Option<(char, usize)> {
    let ch = trimmed
        .chars()
        .next()
        .filter(|ch| matches!(ch, '`' | '~'))?;
    let length = trimmed.chars().take_while(|&c| c == ch).count();
    (length >= 3).then_some((ch, length))
}

/// Whether a line closes the fence `(ch, length)`: a run of the same character at least as long,
/// and nothing else.
fn closes_fence(trimmed: &str, (ch, length): (char, usize)) -> bool {
    let run = trimmed.chars().take_while(|&c| c == ch).count();
    run >= length && trimmed[run..].trim().is_empty()
}

/// Whether a line underlines the paragraph above it into a heading: only `=` or only `-`.
fn is_underline(trimmed: &str) -> bool {
    let body = trimmed.trim_end();
    !body.is_empty() && (body.chars().all(|c| c == '=') || body.chars().all(|c| c == '-'))
}

/// Whether a line starts a block that is not a paragraph a following underline could turn into a
/// heading: a list item or a block quote.
fn starts_container(trimmed: &str) -> bool {
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    trimmed.starts_with(['-', '*', '+', '>'])
        || (digits > 0 && trimmed[digits..].starts_with(['.', ')']))
}

/// The paragraph above a line, as far as a setext underline cares.
enum Paragraph<'a> {
    /// None: a blank line, a heading or a fence came last.
    None,
    /// One line of plain text, which an underline makes a heading.
    Line(&'a str),
    /// Several lines or a container. `mdbook` gives such a heading no id, or none at all.
    Other,
}

/// The ids `mdbook` gives the headings of a chapter, in order, repeated ones numbered `-1`, `-2`.
///
/// A heading is an `ATX` line (`## Text`) or a single line of text underlined with `=` or `-`,
/// outside fenced code and not indented four columns or more. A heading over several lines gets
/// no id from `mdbook`, so none is listed.
fn heading_ids(chapter: &str) -> Vec<String> {
    let text = normalize(chapter);
    let mut ids = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut fence: Option<(char, usize)> = None;
    let mut paragraph = Paragraph::None;
    let mut number = |base: String| {
        let count = seen.entry(base.clone()).or_insert(0);
        let id = if *count == 0 {
            base
        } else {
            format!("{base}-{count}")
        };
        *count += 1;
        id
    };
    for line in text.lines() {
        let trimmed = line.trim_start();
        let indented = indent_of(line) >= 4;
        if let Some(open) = fence {
            if !indented && closes_fence(trimmed, open) {
                fence = None;
            }
            continue;
        }
        if trimmed.trim().is_empty() {
            paragraph = Paragraph::None;
            continue;
        }
        if !indented {
            if let Some(open) = fence_opened_by(trimmed) {
                fence = Some(open);
                paragraph = Paragraph::None;
                continue;
            }
            if is_underline(trimmed) {
                if let Paragraph::Line(title) = paragraph {
                    ids.push(number(heading_slug(title.trim())));
                }
                paragraph = Paragraph::None;
                continue;
            }
            let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
            if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
                let title = without_closing_hashes(trimmed[hashes..].trim());
                ids.push(number(heading_slug(title)));
                paragraph = Paragraph::None;
                continue;
            }
        }
        paragraph = match paragraph {
            Paragraph::None if indented => Paragraph::None,
            Paragraph::None if !starts_container(trimmed) => Paragraph::Line(line),
            _ => Paragraph::Other,
        };
    }
    ids
}

/// Why `link` does not resolve, or `None` when it does.
fn link_problem(root: &Path, summary: &[String], link: &Link<'_>) -> Option<String> {
    let url = format!("{SITE}{}", link.rest);
    let (path, anchor) = link
        .rest
        .split_once('#')
        .map_or((link.rest, None), |(path, anchor)| (path, Some(anchor)));
    let chapter = if path.is_empty() || path == "index.html" {
        // The front page is the first chapter listed, whatever its name.
        summary.first().cloned()
    } else if GENERATED_PAGES.contains(&path) {
        return None;
    } else if let Some(stem) = path.strip_suffix(".html") {
        let file = format!("{stem}.md");
        if !root.join(BOOK_SRC).join(&file).is_file() {
            return Some(format!(
                "`{url}` names no chapter: {BOOK_SRC}/{file} does not exist"
            ));
        }
        if !summary.contains(&file) {
            return Some(format!(
                "`{url}` names {BOOK_SRC}/{file}, which {SUMMARY} does not list, so the book does \
                 not publish it"
            ));
        }
        Some(file)
    } else {
        let wrapper = !path.contains('/') && root.join(INSTALL_DIR).join(path).is_file();
        return (!wrapper).then(|| {
            format!(
                "`{url}` is neither a chapter (`….html`) nor a file of {INSTALL_DIR}/, which the \
                 site serves at its root"
            )
        });
    };
    let anchor = anchor?;
    let Some(file) = chapter else {
        return Some(format!(
            "`{url}` has an anchor, but {SUMMARY} lists no chapter"
        ));
    };
    match fs::read_to_string(root.join(BOOK_SRC).join(&file)) {
        Ok(text) => (!heading_ids(&text).iter().any(|id| id == anchor)).then(|| {
            format!("`{url}` has no heading with the id `{anchor}` in the chapter it names")
        }),
        Err(err) => Some(format!(
            "`{url}` names {BOOK_SRC}/{file}, which could not be read: {err}"
        )),
    }
}

/// The files whose links are checked: the documents the guards read and `CHANGELOG.md`, which
/// links chapters too, then every `.rs` file under `crates/` except this crate's own, whose tests
/// spell broken links on purpose.
fn scanned(root: &Path) -> Vec<PathBuf> {
    let own = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = workspace::documents(root);
    files.push(root.join("CHANGELOG.md"));
    let mut sources: Vec<PathBuf> = WalkDir::new(root.join("crates"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_file()
                && entry.path().extension().is_some_and(|ext| ext == "rs")
                && !entry.path().starts_with(own)
        })
        .map(walkdir::DirEntry::into_path)
        .collect();
    sources.sort();
    files.extend(sources);
    files
}

/// Every broken link under `root`, as `path:line: message`, empty when none is.
fn problems(root: &Path) -> Vec<String> {
    let summary = match fs::read_to_string(root.join(SUMMARY)) {
        Ok(text) => listed_chapters(&normalize(&text)),
        Err(err) => return vec![format!("{SUMMARY}: could not be read: {err}")],
    };
    let mut found = Vec::new();
    for file in scanned(root) {
        let shown = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let text = match fs::read_to_string(&file) {
            Ok(text) => text,
            // A document that does not exist has no links; one that exists and cannot be read
            // might.
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => {
                found.push(format!("{shown}: could not be read: {err}"));
                continue;
            }
        };
        for link in links(&text) {
            if let Some(reason) = link_problem(root, &summary, &link) {
                found.push(format!("{shown}:{}: {reason}", link.line));
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

    use tempfile::TempDir;

    use std::fs;
    use std::path::Path;

    use super::{Link, SUMMARY, heading_ids, links, listed_chapters, problems, scanned};
    use crate::workspace;

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// A tree whose links all resolve: three listed chapters, one chapter on disk that the summary
    /// leaves out, the two install wrappers, and a README and a source file each carrying links.
    fn fixture() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        write(
            root,
            SUMMARY,
            "# Summary\n\n[Introduction](introduction.md)\n\n- [One](ch01.md)\n    - [Two](sub/ch02.md)\n- [Draft]()\n",
        );
        write(root, "book/src/introduction.md", "# Intro\n\n## Welcome\n");
        write(root, "book/src/ch01.md", "# One\n\n## First step\n");
        write(root, "book/src/sub/ch02.md", "# Two\n");
        write(root, "book/src/unlisted.md", "# Unlisted\n");
        write(root, "install/install.sh", "");
        write(root, "install/install.ps1", "");
        write(
            root,
            "crates/a/README.md",
            "See https://airsstack.github.io/airsl/ch01.html for more.\n",
        );
        write(
            root,
            "crates/a/src/lib.rs",
            "//! <https://airsstack.github.io/airsl/sub/ch02.html>\n",
        );
        dir
    }

    /// The fixture with `crates/a/README.md` replaced by `text`.
    fn fixture_with_readme(text: &str) -> TempDir {
        let dir = fixture();
        write(dir.path(), "crates/a/README.md", text);
        dir
    }

    #[test]
    fn the_fixture_tree_has_no_problem() {
        let dir = fixture();
        assert_eq!(problems(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn an_address_ends_at_the_first_character_a_path_cannot_hold() {
        let text = "a \"https://airsstack.github.io/airsl/ch01.html\" b\n\
                    [x](https://airsstack.github.io/airsl/ch01.html#top) `https://airsstack.github.io/airsl/` \
                    https://airsstack.github.io/airsl/ch02.html.\n\
                    <https://airsstack.github.io/airsl/…>";
        assert_eq!(
            links(text),
            vec![
                Link {
                    line: 1,
                    rest: "ch01.html"
                },
                Link {
                    line: 2,
                    rest: "ch01.html#top"
                },
                Link { line: 2, rest: "" },
                Link {
                    line: 2,
                    rest: "ch02.html"
                },
                Link { line: 3, rest: "" },
            ]
        );
    }

    #[test]
    fn a_chapter_that_does_not_exist_is_reported_at_its_line() {
        let dir =
            fixture_with_readme("ok\nhttps://airsstack.github.io/airsl/ch01-05-renamed.html\n");
        assert_eq!(
            problems(dir.path()),
            vec![
                "crates/a/README.md:2: `https://airsstack.github.io/airsl/ch01-05-renamed.html` \
                 names no chapter: book/src/ch01-05-renamed.md does not exist"
            ]
        );
    }

    #[test]
    fn a_chapter_in_a_source_file_is_checked_like_one_in_a_readme() {
        let dir = fixture();
        write(
            dir.path(),
            "crates/a/src/lib.rs",
            "const PAGE: &str = \"https://airsstack.github.io/airsl/gone.html\";\n",
        );
        assert_eq!(
            problems(dir.path()),
            vec![
                "crates/a/src/lib.rs:1: `https://airsstack.github.io/airsl/gone.html` names no \
                 chapter: book/src/gone.md does not exist"
            ]
        );
    }

    #[test]
    fn a_link_in_claude_md_and_the_root_readme_is_checked() {
        let dir = fixture();
        for doc in ["CLAUDE.md", "README.md"] {
            write(
                dir.path(),
                doc,
                "https://airsstack.github.io/airsl/gone.html\n",
            );
        }
        assert_eq!(
            problems(dir.path()),
            vec![
                "CLAUDE.md:1: `https://airsstack.github.io/airsl/gone.html` names no chapter: \
                 book/src/gone.md does not exist"
                    .to_owned(),
                "README.md:1: `https://airsstack.github.io/airsl/gone.html` names no chapter: \
                 book/src/gone.md does not exist"
                    .to_owned(),
            ]
        );
    }

    #[test]
    fn a_chapter_the_summary_does_not_list_is_reported() {
        let dir = fixture_with_readme("https://airsstack.github.io/airsl/unlisted.html\n");
        assert_eq!(
            problems(dir.path()),
            vec![
                "crates/a/README.md:1: `https://airsstack.github.io/airsl/unlisted.html` names \
                 book/src/unlisted.md, which book/src/SUMMARY.md does not list, so the book does \
                 not publish it"
            ]
        );
    }

    #[test]
    fn the_site_root_and_the_index_page_resolve() {
        let dir = fixture_with_readme(
            "https://airsstack.github.io/airsl/ and https://airsstack.github.io/airsl/index.html\n",
        );
        assert_eq!(problems(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn an_anchor_on_the_front_page_is_checked_against_the_first_listed_chapter() {
        let dir = fixture_with_readme(
            "https://airsstack.github.io/airsl/index.html#welcome \
             https://airsstack.github.io/airsl/#nowhere\n",
        );
        assert_eq!(
            problems(dir.path()),
            vec![
                "crates/a/README.md:1: `https://airsstack.github.io/airsl/#nowhere` has no \
                 heading with the id `nowhere` in the chapter it names"
            ]
        );
    }

    #[test]
    fn the_generated_print_and_not_found_pages_resolve() {
        let dir = fixture_with_readme(
            "https://airsstack.github.io/airsl/print.html \
             https://airsstack.github.io/airsl/404.html#top\n",
        );
        assert_eq!(problems(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn a_link_in_the_changelog_is_checked() {
        let dir = fixture();
        write(
            dir.path(),
            "CHANGELOG.md",
            "- see https://airsstack.github.io/airsl/gone.html\n",
        );
        assert_eq!(
            problems(dir.path()),
            vec![
                "CHANGELOG.md:1: `https://airsstack.github.io/airsl/gone.html` names no chapter: \
                 book/src/gone.md does not exist"
            ]
        );
    }

    #[test]
    fn a_scanned_file_that_cannot_be_read_is_reported_not_skipped() {
        let dir = fixture();
        fs::write(dir.path().join("crates/a/README.md"), [0xff_u8]).unwrap();
        let found = problems(dir.path());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("crates/a/README.md: could not be read: "),
            "{found:?}"
        );
    }

    #[test]
    fn a_chapter_that_cannot_be_read_is_reported_as_such() {
        let dir = fixture();
        fs::write(dir.path().join("book/src/ch01.md"), [0xff_u8]).unwrap();
        write(
            dir.path(),
            "crates/a/README.md",
            "https://airsstack.github.io/airsl/ch01.html#first-step\n",
        );
        let found = problems(dir.path());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with(
                "crates/a/README.md:1: `https://airsstack.github.io/airsl/ch01.html#first-step` \
                 names book/src/ch01.md, which could not be read: "
            ),
            "{found:?}"
        );
    }

    #[test]
    fn an_install_wrapper_resolves_and_any_other_file_is_reported() {
        let dir = fixture_with_readme(
            "https://airsstack.github.io/airsl/install.sh \
             https://airsstack.github.io/airsl/install.ps1.\n\
             https://airsstack.github.io/airsl/install.zsh \
             https://airsstack.github.io/airsl/sub/install.sh \
             https://airsstack.github.io/airsl/ch01\n",
        );
        let found = problems(dir.path());
        assert_eq!(found.len(), 3, "{found:?}");
        for (reported, name) in found.iter().zip(["install.zsh", "sub/install.sh", "ch01"]) {
            assert_eq!(
                reported,
                &format!(
                    "crates/a/README.md:2: `https://airsstack.github.io/airsl/{name}` is neither a \
                     chapter (`….html`) nor a file of install/, which the site serves at its root"
                )
            );
        }
    }

    #[test]
    fn an_anchor_must_be_the_id_of_a_heading_of_the_chapter() {
        let dir = fixture_with_readme(
            "https://airsstack.github.io/airsl/ch01.html#first-step\n\
             https://airsstack.github.io/airsl/ch01.html#second-step\n",
        );
        assert_eq!(
            problems(dir.path()),
            vec![
                "crates/a/README.md:2: `https://airsstack.github.io/airsl/ch01.html#second-step` \
                 has no heading with the id `second-step` in the chapter it names"
            ]
        );
    }

    /// Every heading below with the id `mdbook` 0.4.52 gave it when a scratch book was built from
    /// this very chapter. A heading with no id here is one `mdbook` gave none.
    const MDBOOK_CHAPTER: &str = "\
# Options for `ext doctor` and `ext fire`\n\
## `--fail-open`\n\
## `confined`: the default\n\
## A First Project: Counting Headings\n\
## Host access does not reach Lua's own functions\n\
## `path.absolute`\n\
## Should `.airsl/` be committed?\n\
## Pointing LuaRocks at Lua 5.4 or LuaJIT\n\
## \u{2460} The ceiling\n\
## Same\n\
## Same\n\
## Same\n\
## Same-1\n\
## Same\n\
## C#\n\
## Closed ##\n\
## \u{c9}tude \u{dc}n\u{ef}\n\
## \u{65e5}\u{672c}\u{8a9e} text\n\
## Tom & Jerry's \"quote\"\n\
## See [the link](x.md) here\n\
## *Emphasis* and **strong**\n\
## 2.5 > 2\n\
## a_b-c  d\n\
## \u{bd} and \u{2460} and \u{df}\n\
## \u{1c4} \u{1c5}\n\
## Trailing dot.\n\
```text\n\
## inside a fence\n\
```\n\
## `code` and <b>html</b>\n\
## a < b > c\n\
## x <span class=\"k\">y</span> z\n\
## unclosed <b here\n\
## <br/>Break\n";

    #[test]
    fn heading_ids_equal_the_ones_mdbook_gave_a_scratch_chapter() {
        assert_eq!(
            heading_ids(MDBOOK_CHAPTER),
            [
                "options-for-ext-doctor-and-ext-fire",
                "--fail-open",
                "confined-the-default",
                "a-first-project-counting-headings",
                "host-access-does-not-reach-luas-own-functions",
                "pathabsolute",
                "should-airsl-be-committed",
                "pointing-luarocks-at-lua-54-or-luajit",
                "\u{2460}-the-ceiling",
                "same",
                "same-1",
                "same-2",
                "same-1",
                "same-3",
                "c",
                "closed",
                "\u{c9}tude-\u{dc}n\u{ef}",
                "\u{65e5}\u{672c}\u{8a9e}-text",
                "tom--jerrys-quote",
                "see-the-link-here",
                "emphasis-and-strong",
                "25--2",
                "a_b-c--d",
                "\u{bd}-and-\u{2460}-and-\u{df}",
                "\u{1c4}-\u{1c5}",
                "trailing-dot",
                "code-and-html",
                "a--b--c",
                "x-y-z",
                "unclosed-b-here",
                "break",
            ]
        );
    }

    /// The structures `mdbook` 0.4.52 gave an id and the ones it did not, from a scratch chapter.
    /// A string continuation drops the indent of the next line, so the indented lines spell it as
    /// escapes.
    const MDBOOK_STRUCTURES: &str = "\
# Start\n\
\n\
Setext One\n\
==========\n\
\n\
Setext Two\n\
----------\n\
\n\
Multi\n\
line words\n\
---\n\
\n\
\x20\x20\x20\x20# indented four\n\
\n\
\x20\x20\x20## indented three\n\
\n\
\t# tab indented\n\
\n\
~~~\n\
```\n\
# in tilde fence\n\
~~~\n\
\n\
````\n\
```\n\
# in long fence\n\
````\n\
\n\
- item\n\
---\n\
\n\
Para\n\
\n\
---\n\
\n\
> quote\n\
---\n\
\n\
Trailing setext ##\n\
======\n\
\n\
| a | b |\n\
|---|---|\n\
\n\
Mixed *emph* Setext\n\
===\n\
\n\
# After\n";

    #[test]
    fn heading_ids_equal_the_ones_mdbook_gave_setext_indented_and_fenced_text() {
        assert_eq!(
            heading_ids(MDBOOK_STRUCTURES),
            [
                "start",
                "setext-one",
                "setext-two",
                "indented-three",
                "trailing-setext-",
                "mixed-emph-setext",
                "after",
            ]
        );
    }

    #[test]
    fn a_repeated_heading_is_found_by_the_numbered_id_it_took() {
        let dir = fixture();
        write(
            dir.path(),
            "book/src/ch01.md",
            "# One\n\n## Same\n\n## Same\n",
        );
        write(
            dir.path(),
            "crates/a/README.md",
            "https://airsstack.github.io/airsl/ch01.html#same-1 \
             https://airsstack.github.io/airsl/ch01.html#same-2\n",
        );
        assert_eq!(
            problems(dir.path()),
            vec![
                "crates/a/README.md:1: `https://airsstack.github.io/airsl/ch01.html#same-2` has \
                 no heading with the id `same-2` in the chapter it names"
            ]
        );
    }

    #[test]
    fn the_summary_lists_chapters_by_their_link_targets() {
        assert_eq!(
            listed_chapters(
                "[Intro](introduction.md)\n- [A](./a.md)\n  - [B](dir/b.md)\n- [Draft]()\n\
                 [Site](https://example.org/x.md)\n"
            ),
            ["introduction.md", "a.md", "dir/b.md"]
        );
    }

    #[test]
    fn a_missing_summary_is_reported_once_instead_of_every_link() {
        let dir = fixture();
        fs::remove_file(dir.path().join(SUMMARY)).unwrap();
        let found = problems(dir.path());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("book/src/SUMMARY.md: could not be read: "),
            "{found:?}"
        );
    }

    #[test]
    fn this_crates_own_sources_are_not_scanned() {
        let root = workspace::root();
        let own = Path::new(env!("CARGO_MANIFEST_DIR"));
        let scanned = scanned(&root);
        assert!(!scanned.is_empty());
        assert!(scanned.iter().all(|file| !file.starts_with(own)));
        assert!(scanned.iter().any(|file| file.ends_with("README.md")));
        assert!(scanned.iter().any(|file| file.ends_with("native.rs")));
    }

    #[test]
    fn every_link_into_the_book_resolves() {
        assert_eq!(problems(&workspace::root()), Vec::<String>::new());
    }
}
