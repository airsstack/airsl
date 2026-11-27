//! The `cargo make` tasks that the shipped prose names, held to `Makefile.toml`.
//!
//! Its own module because its subject is a command rather than a path, an output or a version, and
//! no other guard parses `Makefile.toml` as TOML. `CLAUDE.md` describes what `cargo make dod` runs,
//! and a README or a book chapter tells a reader to run a task; a task that is renamed or dropped,
//! or a step added to the gate that `CLAUDE.md` never mentions, leaves that prose false and nothing
//! in the build notices.
//!
//! Responsibilities:
//!
//! - Reading the tasks declared under `[tasks.<name>]` in `Makefile.toml` as TOML.
//! - Finding every `cargo make <task>` in a fenced code block or an inline code span of the
//!   documents `workspace::documents` lists (`CLAUDE.md`, the root `README.md` and every
//!   `README.md` under `crates/`, the examples' included) and of every chapter directly in
//!   `book/src/`, and requiring the task to be declared. The flags of `cargo make --help` are read:
//!   `-t` and `--task` name the task, a flag that takes a value is skipped with it, any other flag
//!   is skipped alone, and a flag that asks for something but a run (`--list-all-steps`, `--help`)
//!   ends the command with no task. What follows the task is ignored. The repository sets
//!   `skip_core_tasks`, so cargo-make's built-in tasks do not exist and no task is excepted.
//! - Requiring every task that `dod` lists in its `dependencies`, and every task that one of those
//!   lists in its own, to be written as `cargo make <step>` in a code span or a code block of
//!   `CLAUDE.md`. A bare `test` or `clippy` would be satisfied by the CLI subcommand, `cargo test`
//!   or a lint path, so the command form is the one that names a step. No step is excepted as
//!   plumbing: each is part of what the gate runs.
//! - Reporting a `Makefile.toml`, a document or the chapter directory that cannot be read, and a
//!   `Makefile.toml` that is not TOML.
//!
//! Non-responsibilities: what a task does, which is the task and the gate; steps below the second
//! level, which a listed step's own prose covers; a task written some other way than
//! `cargo make <task>` (`cargo-make`, `makers`); and indented code blocks, HTML and text outside
//! backticks, which are not code here.

use std::fs;
use std::path::Path;

use toml::Value;

use crate::workspace::{documents, toml_error_line};

/// The task graph, relative to the repository root.
const MAKEFILE: &str = "Makefile.toml";

/// The document that must name the gate's steps, relative to the repository root.
const CLAUDE_MD: &str = "CLAUDE.md";

/// The chapter directory of the book, relative to the repository root; every `.md` file directly
/// in it is scanned.
const BOOK_SRC: &str = "book/src";

/// The task whose steps `CLAUDE.md` must name.
const GATE: &str = "dod";

/// The flags of `cargo make --help` (cargo-make 0.37) that take the next word as their value,
/// besides `-t` and `--task`, which name the task.
const VALUE_FLAGS: [&str; 13] = [
    "--makefile",
    "--profile",
    "-p",
    "--completion",
    "--cwd",
    "--skip-tasks",
    "--env-file",
    "--env",
    "-e",
    "--loglevel",
    "-l",
    "--output-format",
    "--output-file",
];

/// The flags that ask cargo-make for something other than running a task, so the command names
/// none.
const NO_TASK_FLAGS: [&str; 7] = [
    "--help",
    "-h",
    "--version",
    "-V",
    "--list-all-steps",
    "--list-category-steps",
    "--completion",
];

/// What every task is written with: ASCII letters, digits, `-` and `_`.
fn is_word(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')
}

/// A piece of code in a document: an inline span or one line of a fenced block.
#[derive(Debug, PartialEq, Eq)]
struct Region {
    /// 1-based line the piece starts on.
    line: usize,
    /// Its text. An inline span that runs over several lines has them joined with a space.
    text: String,
}

/// The paragraph being collected, and what decides where it ends.
#[derive(Default)]
struct Paragraph {
    /// Its lines, joined with `\n`.
    text: String,
    /// 1-based line of its first line.
    line: usize,
    /// Whether it is inside a blockquote, whose continuation lines carry the marker.
    quoted: bool,
}

impl Paragraph {
    /// Pushes the spans of the paragraph and starts an empty one.
    fn end(&mut self, regions: &mut Vec<Region>) {
        inline_spans(&self.text, self.line, regions);
        self.text.clear();
        self.quoted = false;
    }

    /// Adds a line, which starts the paragraph when it is empty.
    fn push(&mut self, number: usize, line: &str) {
        if self.text.is_empty() {
            self.line = number;
        } else {
            self.text.push('\n');
        }
        self.text.push_str(line);
    }
}

/// Every inline span and every line of a fenced block in `text`, in order.
///
/// A fence is three or more backticks or tildes opening a line, closed by a line of the same
/// character at least as long and nothing else, or by the end of the text; the info string of a
/// backtick fence holds no backtick. A paragraph ends at a blank line, a fence, an ATX heading
/// (which is a paragraph of its own), a bullet or numbered item, and a blockquote line that does
/// not continue a quote. A span is a run of backticks closed by a run of the same length within
/// the paragraph; a run with no closer is ordinary text, and a backslash makes the backtick after
/// it ordinary text outside a span.
fn code_regions(text: &str) -> Vec<Region> {
    let mut regions = Vec::new();
    let mut paragraph = Paragraph::default();
    // The open fence's character and length.
    let mut fence: Option<(char, usize)> = None;
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        if let Some((mark, length)) = fence {
            let closing = line.trim();
            if closing.len() >= length && closing.chars().all(|ch| ch == mark) {
                fence = None;
            } else {
                regions.push(Region {
                    line: number,
                    text: line.to_owned(),
                });
            }
            continue;
        }
        if let Some(opened) = fence_opening(line) {
            paragraph.end(&mut regions);
            fence = Some(opened);
        } else if line.trim().is_empty() {
            paragraph.end(&mut regions);
        } else if is_heading(line) {
            paragraph.end(&mut regions);
            paragraph.push(number, line);
            paragraph.end(&mut regions);
        } else if let Some(quote) = quoted_content(line) {
            if !paragraph.quoted {
                paragraph.end(&mut regions);
                paragraph.quoted = true;
            }
            paragraph.push(number, quote);
        } else {
            if is_list_item(line) {
                paragraph.end(&mut regions);
            }
            paragraph.push(number, line);
        }
    }
    paragraph.end(&mut regions);
    regions
}

/// The line without its leading spaces, at most three of them: more is an indented code block.
fn unindented(line: &str) -> Option<&str> {
    let rest = line.trim_start_matches(' ');
    (line.len() - rest.len() <= 3).then_some(rest)
}

/// The character and length of the fence a line opens, or `None` when it opens none.
fn fence_opening(line: &str) -> Option<(char, usize)> {
    let trimmed = line.trim_start();
    let mark = trimmed
        .chars()
        .next()
        .filter(|ch| matches!(ch, '`' | '~'))?;
    let length = trimmed.chars().take_while(|ch| *ch == mark).count();
    let info = &trimmed[length..];
    (length >= 3 && !(mark == '`' && info.contains('`'))).then_some((mark, length))
}

/// Whether a line is an ATX heading: one to six `#` and then a space, a tab or the end.
fn is_heading(line: &str) -> bool {
    let Some(rest) = unindented(line) else {
        return false;
    };
    let hashes = rest.chars().take_while(|ch| *ch == '#').count();
    (1..=6).contains(&hashes)
        && rest[hashes..]
            .chars()
            .next()
            .is_none_or(char::is_whitespace)
}

/// Whether a line starts a bullet (`-`, `*`, `+`) or a numbered item (up to nine digits, then `.`
/// or `)`), each followed by a space or a tab.
fn is_list_item(line: &str) -> bool {
    let Some(rest) = unindented(line) else {
        return false;
    };
    let after_marker = if let Some(after) = rest.strip_prefix(['-', '*', '+']) {
        after
    } else {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if !(1..=9).contains(&digits) {
            return false;
        }
        match rest[digits..].strip_prefix(['.', ')']) {
            Some(after) => after,
            None => return false,
        }
    };
    after_marker.starts_with([' ', '\t'])
}

/// What a blockquote line says after its `>` and the one space that may follow, or `None` when the
/// line is not a blockquote line.
fn quoted_content(line: &str) -> Option<&str> {
    let after = unindented(line)?.strip_prefix('>')?;
    Some(after.strip_prefix(' ').unwrap_or(after))
}

/// Pushes every inline code span of one paragraph, which starts on `first_line`.
fn inline_spans(paragraph: &str, first_line: usize, regions: &mut Vec<Region>) {
    let bytes = paragraph.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'\\' && bytes.get(at + 1).is_some_and(u8::is_ascii_punctuation) {
            at += 2;
            continue;
        }
        if bytes[at] != b'`' {
            at += 1;
            continue;
        }
        let open = at;
        while bytes.get(at) == Some(&b'`') {
            at += 1;
        }
        let width = at - open;
        let mut scan = at;
        while scan < bytes.len() {
            if bytes[scan] != b'`' {
                scan += 1;
                continue;
            }
            let run = scan;
            while bytes.get(scan) == Some(&b'`') {
                scan += 1;
            }
            if scan - run == width {
                regions.push(Region {
                    line: first_line + paragraph[..open].matches('\n').count(),
                    text: paragraph[at..run].replace('\n', " "),
                });
                at = scan;
                break;
            }
        }
    }
}

/// The leading task-name characters of `word`, or `None` when it starts with something else.
fn task_word(word: &str) -> Option<&str> {
    let end = word.find(|ch| !is_word(ch)).unwrap_or(word.len());
    (end > 0).then(|| &word[..end])
}

/// The task a `cargo make` command runs, given what follows `cargo make`, or `None` when it runs
/// none. Flags come first: `-t`/`--task` take the task as their value, the others are skipped, and
/// those that take a value are skipped with it. The first word that is not a flag is the task.
fn task_after(args: &str) -> Option<&str> {
    let mut words = args.split_whitespace();
    while let Some(word) = words.next() {
        if !word.starts_with('-') {
            return task_word(word);
        }
        let (flag, inline) = match word.split_once('=') {
            Some((flag, value)) => (flag, Some(value)),
            None => (word, None),
        };
        if matches!(flag, "-t" | "--task") {
            return task_word(inline.or_else(|| words.next())?);
        }
        if NO_TASK_FLAGS.contains(&flag) {
            return None;
        }
        if inline.is_none() && VALUE_FLAGS.contains(&flag) {
            words.next();
        }
    }
    None
}

/// Every task a piece of code runs with `cargo make`, in order.
fn tasks_run(code: &str) -> Vec<&str> {
    const COMMAND: &str = "cargo make";
    let mut tasks = Vec::new();
    for (start, _) in code.match_indices(COMMAND) {
        if code[..start].chars().next_back().is_some_and(is_word) {
            continue;
        }
        let Some(after) = code[start + COMMAND.len()..].strip_prefix(char::is_whitespace) else {
            continue;
        };
        tasks.extend(task_after(after));
    }
    tasks
}

/// One line for every `cargo make <task>` in `text` whose task is not in `declared`, in order.
/// `shown` is the document's path as the line prints it.
fn unknown_task_problems(shown: &str, text: &str, declared: &[&str]) -> Vec<String> {
    code_regions(text)
        .iter()
        .flat_map(|region| {
            tasks_run(&region.text)
                .into_iter()
                .filter(|task| !declared.contains(task))
                .map(|task| {
                    format!(
                        "{shown}:{}: runs `cargo make {task}`, but {MAKEFILE} declares no task \
                         `{task}`",
                        region.line
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The names of the tasks `makefile` declares under `[tasks]`.
fn declared_tasks(makefile: &Value) -> Vec<&str> {
    makefile
        .get("tasks")
        .and_then(Value::as_table)
        .map_or_else(Vec::new, |tasks| tasks.keys().map(String::as_str).collect())
}

/// The tasks `task` lists in its `dependencies`, each as a name or as a table with a `name`.
fn dependencies<'a>(makefile: &'a Value, task: &str) -> Option<Vec<&'a str>> {
    let listed = makefile
        .get("tasks")?
        .get(task)?
        .get("dependencies")?
        .as_array()?;
    Some(
        listed
            .iter()
            .filter_map(|entry| match entry {
                Value::String(name) => Some(name.as_str()),
                other => other.get("name").and_then(Value::as_str),
            })
            .collect(),
    )
}

/// The steps of the gate and the step each is listed under, the gate's own first and then each
/// one's dependencies in the order they are listed. A task that is listed twice is kept at its
/// first place.
fn gate_steps(makefile: &Value) -> Option<Vec<(&str, &str)>> {
    let first = dependencies(makefile, GATE)?;
    let mut steps: Vec<(&str, &str)> = first.iter().map(|step| (*step, GATE)).collect();
    for parent in first {
        for step in dependencies(makefile, parent).unwrap_or_default() {
            if !steps.iter().any(|(known, _)| *known == step) {
                steps.push((step, parent));
            }
        }
    }
    Some(steps)
}

/// One line for every step of the gate that no code of `claude` runs as `cargo make <step>`, in
/// the order of [`gate_steps`], or one line saying the gate has no steps to name.
fn unnamed_step_problems(claude: &str, makefile: &Value) -> Vec<String> {
    let Some(steps) = gate_steps(makefile) else {
        return vec![format!(
            "{MAKEFILE}: task `{GATE}` lists no `dependencies`, so there are no steps of \
             `cargo make {GATE}` for {CLAUDE_MD} to name"
        )];
    };
    let regions = code_regions(claude);
    steps
        .into_iter()
        .filter(|(step, _)| {
            !regions
                .iter()
                .any(|region| tasks_run(&region.text).contains(step))
        })
        .map(|(step, parent)| {
            format!(
                "{CLAUDE_MD}: never names `{step}`, which `cargo make {GATE}` runs through \
                 `{parent}`"
            )
        })
        .collect()
}

/// The text of a document, or the problem line saying why it cannot be read.
fn read_document(root: &Path, rel: &str) -> Result<String, String> {
    fs::read_to_string(root.join(rel)).map_err(|err| format!("{rel}: could not be read: {err}"))
}

/// The chapters of the book, as paths relative to the root, in name order.
fn book_chapters(root: &Path) -> Result<Vec<String>, String> {
    let listed = fs::read_dir(root.join(BOOK_SRC)).and_then(Iterator::collect::<Result<Vec<_>, _>>);
    let entries = listed.map_err(|err| format!("{BOOK_SRC}: could not be listed: {err}"))?;
    let mut chapters: Vec<String> = entries
        .iter()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| {
            Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        })
        .map(|name| format!("{BOOK_SRC}/{name}"))
        .collect();
    chapters.sort();
    Ok(chapters)
}

/// A document's path under `root`, written with `/` on every platform.
fn shown(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// The text of `Makefile.toml` parsed, or the problem line saying why it cannot be.
fn read_makefile(root: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(root.join(MAKEFILE))
        .map_err(|err| format!("{MAKEFILE}: could not be read: {err}"))?;
    toml::from_str(&text).map_err(|err| {
        format!(
            "{MAKEFILE}: is not valid TOML: {}",
            toml_error_line(&err.to_string())
        )
    })
}

/// Every way the prose under `root` drifts from `Makefile.toml`, one line each, empty when it
/// does not.
///
/// A `Makefile.toml` that cannot be read is the only line then: with no tasks to compare with, any
/// other line would only be noise. A document or the chapter directory that cannot be read is a
/// line of its own and the others are still read.
fn problems(root: &Path) -> Vec<String> {
    let makefile = match read_makefile(root) {
        Ok(makefile) => makefile,
        Err(line) => return vec![line],
    };
    let declared = declared_tasks(&makefile);
    let mut found = Vec::new();
    let mut documents: Vec<String> = documents(root)
        .iter()
        .map(|path| shown(root, path))
        .collect();
    match book_chapters(root) {
        Ok(chapters) => documents.extend(chapters),
        Err(line) => found.push(line),
    }
    for rel in &documents {
        match read_document(root, rel) {
            Ok(text) => {
                found.extend(unknown_task_problems(rel, &text, &declared));
                if rel == CLAUDE_MD {
                    found.extend(unnamed_step_problems(&text, &makefile));
                }
            }
            Err(line) => found.push(line),
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

    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::{code_regions, problems, unknown_task_problems};
    use crate::workspace;

    /// A task graph shaped like the real one: `dod` runs five steps and `test` runs three more.
    const MAKEFILE: &str = r#"
[tasks.default]
alias = "dod"

[tasks.dod]
dependencies = ["fmt-check", "clippy", "doc", "test", "test-doc"]

[tasks.fmt-check]
[tasks.clippy]
[tasks.doc]
[tasks.test]
dependencies = ["test-cargo", "native-load", { name = "native-refusals" }]

[tasks.test-cargo]
[tasks.native-load]
[tasks.native-refusals]
[tasks.test-doc]
[tasks.fmt]
[tasks.clippy-fix]
[tasks.rocks]
[tasks.dod-crate]
"#;

    /// A `CLAUDE.md` that names every step as `cargo make <step>` in a code block or a code span,
    /// and also uses `test` and `clippy` for other things: a CLI subcommand, `cargo test`, a lint.
    const CLAUDE: &str = "\
## Commands

```bash
cargo make dod          # the gate: fmt-check, clippy, doc, test, test-doc
cargo make fmt-check    # formatting, without rewriting
cargo make clippy-fix   # apply clippy suggestions
cargo make dod-crate airsl
cargo make --list-all-steps
```

The gate runs `cargo make clippy` and `cargo make doc`, then `cargo make test-doc`. The
`cargo make test` step runs `cargo make test-cargo`, then `cargo make native-load`, then
`cargo make native-refusals`. The CLI has `run`, `test` and `check`; `cargo test -p airsl` runs one
crate, and `clippy::pedantic` is a lint.
";

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// A whole valid tree: the makefile above, the `CLAUDE.md` above, and the READMEs (root, two
    /// crates, the examples index and one example) and a book chapter that run only declared tasks.
    /// Every test changes one input of it.
    fn fixture() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        write(root, "Makefile.toml", MAKEFILE);
        write(root, "CLAUDE.md", CLAUDE);
        write(root, "README.md", "Run `cargo make rocks` to pack.\n");
        write(root, "crates/airsl/README.md", "No tasks here.\n");
        write(root, "crates/airsl/examples/README.md", "No tasks here.\n");
        write(
            root,
            "crates/airsl/examples/demo/README.md",
            "No tasks here.\n",
        );
        write(root, "crates/airsl-cli/README.md", "No tasks here.\n");
        write(root, "book/src/ch01.md", "Then `cargo make fmt`.\n");
        dir
    }

    /// Leaves the documents that run tasks with text that runs none, for a test that swaps the
    /// makefile and wants only its own problem.
    fn quiet_documents(root: &Path) {
        write(root, "README.md", "Nothing to run.\n");
        write(root, "book/src/ch01.md", "Nothing to run.\n");
    }

    const DECLARED: [&str; 3] = ["dod", "dod-crate", "rocks"];

    /// The refusal line for a `cargo make <task>` of `text` at `line`, for a task that is not in
    /// `DECLARED`.
    fn unknown(shown: &str, line: usize, task: &str) -> String {
        format!(
            "{shown}:{line}: runs `cargo make {task}`, but Makefile.toml declares no task `{task}`"
        )
    }

    /// The refusal line for a step that `CLAUDE.md` never names.
    fn unnamed(step: &str, parent: &str) -> String {
        format!("CLAUDE.md: never names `{step}`, which `cargo make dod` runs through `{parent}`")
    }

    #[test]
    fn the_fixture_tree_has_no_problem() {
        let dir = fixture();
        assert_eq!(problems(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn a_task_the_makefile_does_not_declare_is_reported_in_a_code_block() {
        let dir = fixture();
        write(
            dir.path(),
            "CLAUDE.md",
            &format!("{CLAUDE}\n```bash\ncargo make nosuch\n```\n"),
        );
        assert_eq!(
            problems(dir.path()),
            vec![unknown("CLAUDE.md", 17, "nosuch")]
        );
    }

    #[test]
    fn a_task_the_makefile_does_not_declare_is_reported_in_every_document_that_names_it() {
        let dir = fixture();
        for rel in [
            "README.md",
            "crates/airsl/README.md",
            "crates/airsl/examples/README.md",
            "crates/airsl/examples/demo/README.md",
            "crates/airsl-cli/README.md",
            "book/src/ch01.md",
        ] {
            write(dir.path(), rel, "text\nthen `cargo make nosuch` runs\n");
        }
        assert_eq!(
            problems(dir.path()),
            [
                "README.md",
                "crates/airsl/README.md",
                "crates/airsl/examples/README.md",
                "crates/airsl/examples/demo/README.md",
                "crates/airsl-cli/README.md",
                "book/src/ch01.md",
            ]
            .map(|rel| unknown(rel, 2, "nosuch"))
        );
    }

    #[test]
    fn every_book_chapter_is_read_in_name_order() {
        let dir = fixture();
        write(dir.path(), "book/src/b.md", "`cargo make two`\n");
        write(dir.path(), "book/src/a.md", "`cargo make one`\n");
        write(dir.path(), "book/src/notes.txt", "`cargo make ignored`\n");
        let found = problems(dir.path());
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].starts_with("book/src/a.md:1: "), "{found:?}");
        assert!(found[1].starts_with("book/src/b.md:1: "), "{found:?}");
    }

    #[test]
    fn arguments_after_the_task_are_ignored() {
        let text = "`cargo make dod-crate airsl --env X=1`\n`cargo make rocks nosuch`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            Vec::<String>::new()
        );
    }

    #[test]
    fn the_task_flag_names_the_task_in_each_spelling() {
        let text = "`cargo make -t nosuch1`\n`cargo make --task nosuch2`\n\
                    `cargo make --task=nosuch3`\n`cargo make -t rocks`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![
                unknown("x.md", 1, "nosuch1"),
                unknown("x.md", 2, "nosuch2"),
                unknown("x.md", 3, "nosuch3"),
            ]
        );
    }

    #[test]
    fn a_flag_that_takes_a_value_is_skipped_with_its_value() {
        let text = "`cargo make --profile ci nosuch1`\n`cargo make -p ci --env X=1 nosuch2`\n\
                    `cargo make --cwd . --env=A=1 nosuch3`\n`cargo make --profile ci rocks`\n\
                    `cargo make --profile ci`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![
                unknown("x.md", 1, "nosuch1"),
                unknown("x.md", 2, "nosuch2"),
                unknown("x.md", 3, "nosuch3"),
            ]
        );
    }

    #[test]
    fn a_flag_without_a_value_is_skipped_and_the_next_word_is_the_task() {
        let text = "`cargo make --no-color nosuch1`\n`cargo make --verbose --print-steps nosuch2`\n\
                    `cargo make -- nosuch3`\n`cargo make --no-color rocks`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![
                unknown("x.md", 1, "nosuch1"),
                unknown("x.md", 2, "nosuch2"),
                unknown("x.md", 3, "nosuch3"),
            ]
        );
    }

    #[test]
    fn a_flag_that_runs_no_task_names_no_task() {
        let text = "`cargo make --list-all-steps`\n`cargo make --help nosuch`\n\
                    `cargo make --list-category-steps nosuch`\n`cargo make -V`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_span_broken_across_lines_still_names_its_task() {
        let text = "Run `cargo make\nnosuch` now\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![unknown("x.md", 1, "nosuch")]
        );
    }

    #[test]
    fn prose_outside_a_code_span_or_block_is_not_a_command() {
        let text = "You can cargo make nosuch in prose.\nOr `cargo install --locked cargo-make`.\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            Vec::<String>::new()
        );
    }

    #[test]
    fn an_indented_code_block_is_not_code() {
        let text = "Run:\n\n    cargo make nosuch\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_double_backtick_span_and_a_longer_fence_are_code() {
        let text = "``cargo make one``\n\n````\n```\ncargo make two\n```\n````\n~~~\n\
                    cargo make three\n~~~\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![
                unknown("x.md", 1, "one"),
                unknown("x.md", 5, "two"),
                unknown("x.md", 9, "three"),
            ]
        );
    }

    #[test]
    fn a_fence_that_is_never_closed_runs_to_the_end_of_the_text() {
        let text = "```\ncargo make nosuch\n";
        assert_eq!(unknown_task_problems("x.md", text, &DECLARED).len(), 1);
    }

    #[test]
    fn a_backtick_fence_whose_info_string_has_a_backtick_is_not_a_fence() {
        let text = "``` rust `x`\ncargo make nosuch\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_tilde_fence_may_carry_a_backtick_in_its_info_string() {
        let text = "~~~ rust `x`\ncargo make nosuch\n~~~\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![unknown("x.md", 2, "nosuch")]
        );
    }

    #[test]
    fn an_escaped_backtick_does_not_open_a_span() {
        let text = "an escaped \\` then `cargo make nosuch`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![unknown("x.md", 1, "nosuch")]
        );
    }

    #[test]
    fn an_escaped_backslash_does_not_escape_the_backtick_after_it() {
        let text = "a path \\\\`cargo make nosuch`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![unknown("x.md", 1, "nosuch")]
        );
    }

    #[test]
    fn a_backslash_inside_a_span_is_literal() {
        let text = "`cargo make nosuch \\` then `cargo make rocks`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![unknown("x.md", 1, "nosuch")]
        );
    }

    #[test]
    fn a_heading_ends_the_paragraph_before_it_and_is_its_own() {
        let text = "# a ` stray\n`cargo make nosuch1`\n# `cargo make nosuch2`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![unknown("x.md", 2, "nosuch1"), unknown("x.md", 3, "nosuch2")]
        );
    }

    #[test]
    fn a_bullet_item_ends_the_paragraph_before_it() {
        let text = "- one ` stray\n- two `cargo make nosuch`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![unknown("x.md", 2, "nosuch")]
        );
    }

    #[test]
    fn a_numbered_item_ends_the_paragraph_before_it() {
        let text = "1. one ` stray\n2) two `cargo make nosuch`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![unknown("x.md", 2, "nosuch")]
        );
    }

    #[test]
    fn a_blockquote_ends_the_paragraph_before_it() {
        let text = "text ` stray\n> quoted `cargo make nosuch`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![unknown("x.md", 2, "nosuch")]
        );
    }

    #[test]
    fn a_span_may_continue_over_a_blockquote_line() {
        let text = "> run `cargo make\n> nosuch` now\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![unknown("x.md", 1, "nosuch")]
        );
    }

    #[test]
    fn a_fence_ends_the_paragraph_before_it() {
        let text = "a stray ` here\n~~~\ncargo make nosuch1\n~~~\nthen `cargo make nosuch2`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            vec![unknown("x.md", 3, "nosuch1"), unknown("x.md", 5, "nosuch2")]
        );
    }

    #[test]
    fn a_flag_looking_line_or_a_hash_without_a_space_does_not_end_a_paragraph() {
        let text = "a ` stray\n-D warnings and #tag then `cargo make nosuch`\n";
        assert_eq!(
            unknown_task_problems("x.md", text, &DECLARED),
            Vec::<String>::new()
        );
    }

    #[test]
    fn dropping_native_refusals_from_claude_md_is_reported_through_its_parent() {
        let dir = fixture();
        write(
            dir.path(),
            "CLAUDE.md",
            &CLAUDE.replace(", then\n`cargo make native-refusals`", ""),
        );
        assert_eq!(
            problems(dir.path()),
            vec![unnamed("native-refusals", "test")]
        );
    }

    #[test]
    fn a_first_level_step_that_claude_md_never_names_is_reported_through_dod() {
        let dir = fixture();
        write(
            dir.path(),
            "CLAUDE.md",
            &CLAUDE
                .replace(
                    "cargo make fmt-check    # formatting, without rewriting\n",
                    "",
                )
                .replace("`cargo make test-doc`", "doctests"),
        );
        assert_eq!(
            problems(dir.path()),
            vec![unnamed("fmt-check", "dod"), unnamed("test-doc", "dod")]
        );
    }

    #[test]
    fn a_step_named_only_in_the_gates_comment_is_not_named() {
        let dir = fixture();
        write(
            dir.path(),
            "CLAUDE.md",
            &CLAUDE.replace(
                "cargo make fmt-check    # formatting, without rewriting\n",
                "",
            ),
        );
        assert_eq!(problems(dir.path()), vec![unnamed("fmt-check", "dod")]);
    }

    #[test]
    fn a_step_that_claude_md_calls_by_a_bare_name_in_a_span_is_not_named() {
        let dir = fixture();
        write(
            dir.path(),
            "CLAUDE.md",
            &CLAUDE.replace("`cargo make native-refusals`", "`native-refusals`"),
        );
        assert_eq!(
            problems(dir.path()),
            vec![unnamed("native-refusals", "test")]
        );
    }

    #[test]
    fn a_step_named_only_in_prose_is_not_named() {
        let dir = fixture();
        write(
            dir.path(),
            "CLAUDE.md",
            &CLAUDE.replace("`cargo make native-refusals`", "native-refusals"),
        );
        assert_eq!(
            problems(dir.path()),
            vec![unnamed("native-refusals", "test")]
        );
    }

    #[test]
    fn test_is_not_named_by_the_cli_subcommand_or_cargo_test() {
        let dir = fixture();
        write(
            dir.path(),
            "CLAUDE.md",
            &CLAUDE.replace("The\n`cargo make test` step runs", "The\ntests step runs"),
        );
        assert_eq!(problems(dir.path()), vec![unnamed("test", "dod")]);
    }

    #[test]
    fn clippy_is_not_named_by_a_lint_path_or_by_the_clippy_fix_task() {
        let dir = fixture();
        write(
            dir.path(),
            "CLAUDE.md",
            &CLAUDE.replace("`cargo make clippy` and ", "the lint step and "),
        );
        assert_eq!(problems(dir.path()), vec![unnamed("clippy", "dod")]);
    }

    #[test]
    fn a_name_inside_a_longer_task_name_does_not_name_the_shorter_task() {
        let dir = fixture();
        write(
            dir.path(),
            "CLAUDE.md",
            "`cargo make fmt-check` `cargo make clippy` `cargo make doc` `cargo make test-doc` \
             `cargo make test-cargo` `cargo make native-load` `cargo make native-refusals`\n",
        );
        assert_eq!(problems(dir.path()), vec![unnamed("test", "dod")]);
    }

    #[test]
    fn a_step_listed_at_both_levels_is_reported_once() {
        let dir = fixture();
        write(
            dir.path(),
            "Makefile.toml",
            "[tasks.dod]\ndependencies = [\"a\", \"b\"]\n[tasks.a]\ndependencies = [\"b\"]\n",
        );
        write(dir.path(), "CLAUDE.md", "`cargo make a`\n");
        quiet_documents(dir.path());
        assert_eq!(problems(dir.path()), vec![unnamed("b", "dod")]);
    }

    #[test]
    fn a_makefile_that_cannot_be_read_is_reported_alone() {
        let dir = fixture();
        fs::remove_file(dir.path().join("Makefile.toml")).unwrap();
        let found = problems(dir.path());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("Makefile.toml: could not be read: "),
            "{found:?}"
        );
    }

    #[test]
    fn a_makefile_that_is_not_valid_toml_shows_the_position_and_the_reason() {
        let dir = fixture();
        write(dir.path(), "Makefile.toml", "[package");
        assert_eq!(
            problems(dir.path()),
            vec![
                "Makefile.toml: is not valid TOML: TOML parse error at line 1, column 9: unclosed \
                 table, expected `]`"
            ]
        );
    }

    #[test]
    fn a_makefile_whose_dod_lists_no_steps_is_reported() {
        let dir = fixture();
        write(dir.path(), "Makefile.toml", "[tasks.rocks]\n");
        write(dir.path(), "CLAUDE.md", "Nothing to run.\n");
        quiet_documents(dir.path());
        assert_eq!(
            problems(dir.path()),
            vec![
                "Makefile.toml: task `dod` lists no `dependencies`, so there are no steps of \
                 `cargo make dod` for CLAUDE.md to name"
            ]
        );
    }

    #[test]
    fn a_document_that_cannot_be_read_is_reported() {
        let dir = fixture();
        fs::remove_file(dir.path().join("README.md")).unwrap();
        let found = problems(dir.path());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("README.md: could not be read: "),
            "{found:?}"
        );
    }

    #[test]
    fn a_book_directory_that_cannot_be_listed_is_reported() {
        let dir = fixture();
        fs::remove_dir_all(dir.path().join("book/src")).unwrap();
        let found = problems(dir.path());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("book/src: could not be listed: "),
            "{found:?}"
        );
    }

    #[test]
    fn crlf_line_endings_give_the_regions_lf_gives() {
        let lf = "# Title\n\nRun `cargo make\nnosuch` now.\n\n- item `a`\n\n```bash\n\
                  cargo make two\n```\n";
        let crlf = lf.replace('\n', "\r\n");
        assert_eq!(code_regions(&crlf), code_regions(lf));
        assert_eq!(code_regions(lf).len(), 3);
    }

    #[test]
    fn the_prose_names_only_tasks_the_makefile_declares_and_claude_md_names_every_step() {
        assert_eq!(problems(&workspace::root()), Vec::<String>::new());
    }
}
