//! The files the book's `{{#include}}`, `{{#rustdoc_include}}` and `{{#playground}}` directives read.
//!
//! Its own module because the book build cannot be trusted to notice a missing target: `mdbook`
//! 0.4.52 logs an `ERROR` for a file it cannot read and still exits `0`, so CI's `mdbook build`
//! step passes, with only that logged error to show the chapter lost its listing. This reads the
//! directives itself.
//!
//! Responsibilities: finding every directive of those three kinds in a chapter, and requiring its
//! target to be a file, resolved relative to the chapter's own directory. A directive is read as
//! `mdbook` reads it: written `{{#name ...}}`, or with whitespace after the opening braces, as
//! `{{ #name ...}}`. A backslash before `{{#` escapes it, so `\{{#include x}}` shows the directive
//! as text; a backslash before the spaced form does not, and `mdbook` still reads it. For an
//! include the target is what precedes any `:anchor` or `:start:end` line-range suffix; for a
//! playground it is the first word, since what follows is a flag such as `editable`.
//!
//! Non-responsibilities: whether an anchor or line range exists inside the target, and directives
//! nested inside an included file.

use std::path::Path;

use crate::workspace::normalize;

/// The directive names that read a file, as `mdbook` spells them.
const DIRECTIVES: [&str; 3] = ["include", "rustdoc_include", "playground"];

/// One file-reading directive: its name, its 1-based line and the target path, suffix removed.
#[derive(Debug, PartialEq, Eq)]
struct Include {
    /// The directive's name, as written after `{{#`.
    directive: String,
    /// 1-based line of the directive's opening braces.
    line: usize,
    /// The path before any `:anchor` or `:start:end` suffix.
    target: String,
}

/// Every file-reading directive in a chapter, as `mdbook` reads it. `{{#include ...}}` preceded by a
/// backslash is the escape that shows the directive as text, and is not read; whitespace between the
/// braces and the `#` makes it a directive again.
fn includes(chapter: &str) -> Vec<Include> {
    let text = normalize(chapter);
    let mut found = Vec::new();
    for (start, _) in text.match_indices("{{") {
        let after_braces = &text[start + 2..];
        let Some(body) = after_braces.trim_start().strip_prefix('#') else {
            continue;
        };
        let spaced = after_braces.starts_with(char::is_whitespace);
        if !spaced && text[..start].ends_with('\\') {
            continue;
        }
        let Some(end) = body.find("}}") else {
            continue;
        };
        let Some((name, argument)) = body[..end].split_once(char::is_whitespace) else {
            continue;
        };
        if !DIRECTIVES.contains(&name) {
            continue;
        }
        let argument = argument.trim();
        let target = if name == "playground" {
            argument.split_whitespace().next()
        } else {
            argument.split(':').next()
        }
        .unwrap_or_default();
        found.push(Include {
            directive: name.to_owned(),
            line: text[..start].matches('\n').count() + 1,
            target: target.to_owned(),
        });
    }
    found
}

/// Every directive of `chapter` whose target is not a file, one line each. Targets resolve relative
/// to the chapter's own directory, which is what `mdbook` does.
pub(super) fn problems(chapter_path: &Path, text: &str, root: &Path) -> Vec<String> {
    let shown = chapter_path
        .strip_prefix(root)
        .unwrap_or(chapter_path)
        .display()
        .to_string();
    let dir = chapter_path.parent().unwrap_or(root);
    includes(text)
        .into_iter()
        .filter(|include| !dir.join(&include.target).is_file())
        .map(|include| {
            format!(
                "{shown}:{}: `{{{{#{}}}}}` reads `{}`, which does not exist relative to the chapter (mdbook only logs this and still exits 0)",
                include.line, include.directive, include.target,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "fixtures are known to be creatable; a panic is the failure signal"
    )]

    use super::*;

    use std::fs;

    use walkdir::WalkDir;

    use crate::workspace::root;

    #[test]
    fn a_suffix_is_stripped_from_the_target() {
        let text = "{{#include plain.lua}}\n{{#rustdoc_include ../x/y.rs:anchor}}\n{{#include z.rs:2:10}}\n{{#include z.rs::10}}\n{{#include z.rs:2:}}\n";
        let all = includes(text);
        let targets: Vec<(usize, &str)> = all
            .iter()
            .map(|include| (include.line, include.target.as_str()))
            .collect();
        assert_eq!(
            targets,
            vec![
                (1, "plain.lua"),
                (2, "../x/y.rs"),
                (3, "z.rs"),
                (4, "z.rs"),
                (5, "z.rs")
            ]
        );
    }

    #[test]
    fn a_playground_target_is_its_first_word() {
        let text = "{{#playground p.rs}}\n{{#playground dir/q.rs editable}}\n";
        let all = includes(text);
        let targets: Vec<&str> = all.iter().map(|include| include.target.as_str()).collect();
        assert_eq!(targets, ["p.rs", "dir/q.rs"]);
    }

    #[test]
    fn a_missing_playground_target_fails_and_a_present_one_with_a_flag_passes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("book/src")).unwrap();
        fs::write(root.join("book/src/ok.rs"), "").unwrap();
        let chapter = root.join("book/src/ch.md");
        let text = "{{#playground ok.rs editable}}\n{{#playground gone.rs editable}}\n";
        let found = problems(&chapter, text, root);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("book/src/ch.md:2: `{{#playground}}` reads `gone.rs`"),
            "{found:?}"
        );
    }

    #[test]
    fn a_directive_with_whitespace_after_its_braces_is_read() {
        let text = "{{ #include a.rs }}\n{{\t#rustdoc_include b.rs:x}}\n{{ #playground c.rs editable}}\n{{\n#include d.rs}}\n";
        let all = includes(text);
        let found: Vec<(&str, usize, &str)> = all
            .iter()
            .map(|include| {
                (
                    include.directive.as_str(),
                    include.line,
                    include.target.as_str(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                ("include", 1, "a.rs"),
                ("rustdoc_include", 2, "b.rs"),
                ("playground", 3, "c.rs"),
                ("include", 4, "d.rs"),
            ]
        );
    }

    #[test]
    fn a_backslash_escapes_only_the_unspaced_form() {
        assert_eq!(includes("\\{{ #include still_read.rs}}\n").len(), 1);
        assert_eq!(includes("\\{{#include shown.rs}}\n"), Vec::new());
    }

    #[test]
    fn an_escaped_directive_and_other_directives_are_not_read() {
        let text = "\\{{#include shown_as_text.rs}}\n{{#title A Title}}\n\\{{#playground p.rs}}\n";
        assert_eq!(includes(text), Vec::new());
    }

    #[test]
    fn a_directive_inside_a_fence_is_still_read() {
        let text = "```\n{{#include in_fence.rs}}\n```\n";
        assert_eq!(includes(text).len(), 1);
    }

    #[test]
    fn a_target_resolves_relative_to_the_chapter_and_a_missing_one_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("book/src")).unwrap();
        fs::create_dir_all(root.join("crates/x")).unwrap();
        fs::write(root.join("book/src/here.lua"), "").unwrap();
        fs::write(root.join("crates/x/there.rs"), "").unwrap();
        let chapter = root.join("book/src/ch.md");
        let text = "{{#include here.lua}}\n{{#rustdoc_include ../../crates/x/there.rs:setup}}\n{{#rustdoc_include ../../crates/x/gone.rs:setup}}\n{{#include missing.lua}}\n";
        let found = problems(&chapter, text, root);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
        found[0].starts_with(
            "book/src/ch.md:3: `{{#rustdoc_include}}` reads `../../crates/x/gone.rs`, which does not exist"
        ),
        "{found:?}"
    );
        assert!(
            found[1].starts_with("book/src/ch.md:4: `{{#include}}` reads `missing.lua`"),
            "{found:?}"
        );
    }

    #[test]
    fn every_file_directive_target_in_the_book_exists() {
        let root = root();
        let chapters: Vec<_> = WalkDir::new(root.join("book/src"))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry.path().extension().is_some_and(|ext| ext == "md")
            })
            .map(walkdir::DirEntry::into_path)
            .collect();
        assert!(!chapters.is_empty(), "no chapter found under book/src");
        let found: Vec<String> = chapters
            .iter()
            .flat_map(|chapter| {
                let text = fs::read_to_string(chapter).unwrap();
                problems(chapter, &text, &root)
            })
            .collect();
        assert!(
            found.is_empty(),
            "book directives that point at nothing:\n{}",
            found.join("\n")
        );
    }
}
