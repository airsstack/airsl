//! Module paths and `snake_case` names cited in the repository `CLAUDE.md`, checked against the code.
//!
//! Its own module because a module path is a different promise from a file path: `require::loader`
//! names a Rust module, and a module path stays the same spelling when its file moves or folds into
//! a folder, so the path in the prose keeps reading true while nothing checks it still exists.
//!
//! The rule is deliberately narrow, so that what it passes is exactly what it checked:
//!
//! - A code span is a module path when it is `::`-joined identifiers (an optional trailing `()`
//!   allowed), at least two of them, the first starting lowercase and not naming `std`, `core`,
//!   `alloc`, `crate`, `self`, `super`, a lint tool (`clippy`, `rustc`, `rustdoc`), a
//!   workspace crate, or a dependency in the root manifest's `[workspace.dependencies]`. So
//!   `airsl::mlua`, `mlua::Lua`, `std::env`, `Policy::trusted()` and `Engine::builder()` are not
//!   module paths and are not read.
//! - Its leading lowercase segments are its module path. The longest prefix that is a module of
//!   some workspace crate (`src/<a>/<b>.rs` or `src/<a>/<b>/mod.rs`) is the module; the segment
//!   after it, if any, must be declared in that module's source. Segments after that (a method,
//!   say) are not read.
//! - A segment is declared by a `mod`, `fn`, `struct`, `enum`, `union`, `trait`, `type`, `const` or
//!   `static` item, a `macro_rules!` definition, or a re-export (`pub use`, `pub(crate) use`, ...)
//!   that brings the name in: the last segment of its path, the name after `as`, or `a` in
//!   `a::{self}`. The source is read with comments, doc comments, and string and character
//!   literals (`"..."`, `r#"..."#`, and their `b` and `c` forms) blanked out, so prose that merely
//!   mentions the word does not declare it. Three things are not read as a declaration: a private
//!   `use`, an item inside an `impl` block (an associated item is not a name of the module), and an
//!   item inside a `#[cfg(test)]` module. Items inside any other block, a trait or a function body
//!   for one, are still read.
//! - A span that is a single `snake_case` name containing an underscore is a name, and must appear as
//!   a whole word in some `.rs` or `Cargo.toml` file of the workspace outside this crate, whose
//!   tests spell stale names on purpose. The names in `ANTI_EXAMPLES` are exempt, each with its
//!   reason.
//!
//! Responsibilities: classifying spans by that rule and reporting each module path or name that
//! does not resolve.
//!
//! Non-responsibilities: file paths, which `source` owns; and everything else in backticks (a type,
//! a method, a command, a Lua name), which this module does not read at all.

use std::cell::OnceCell;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use super::span::code_spans;
use crate::workspace::crate_dirs;

/// Leading segments that are never a workspace module: the standard library, path keywords and
/// the lint tools that scope lint names (`clippy::pedantic`).
const NOT_A_WORKSPACE_MODULE: [&str; 9] = [
    "std", "core", "alloc", "crate", "self", "super", "clippy", "rustc", "rustdoc",
];

/// Lone names that `CLAUDE.md` cites on purpose as what not to write, each with the reason it is
/// not looked up. Every entry is a deliberate exception to the rule that a cited name exists, so
/// it carries the reason and nothing else is exempt.
const ANTI_EXAMPLES: [(&str, &str); 1] = [(
    "test_confined",
    "the house rule's example of a terse test name, which is the thing the rule forbids",
)];

/// What a span cites, when it cites a module or a name.
#[derive(Debug, PartialEq, Eq)]
enum Candidate<'a> {
    /// `a::b::item`: the leading lowercase segments, then everything from the first segment that
    /// does not start lowercase.
    Path {
        /// The leading segments that start lowercase, in order.
        modules: Vec<&'a str>,
        /// The segments after them.
        rest: Vec<&'a str>,
    },
    /// A single `snake_case` name containing an underscore.
    Name(&'a str),
}

/// Whether `text` is a Rust identifier (ASCII only, which is all this workspace uses).
fn is_ident(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Whether `text` starts like a module name: lowercase or an underscore.
fn starts_lowercase(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
}

/// The keys of the `[workspace.dependencies]` table of a manifest, with `-` read as `_`, which is
/// how a crate is spelled in a path.
fn dependency_names(manifest: &str) -> Vec<String> {
    let mut in_table = false;
    let mut names = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_table = line == "[workspace.dependencies]";
        } else if in_table
            && !line.starts_with('#')
            && let Some((key, _)) = line.split_once('=')
        {
            let key = key.trim();
            let valid = !key.is_empty()
                && key
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'));
            if valid {
                names.push(key.replace('-', "_"));
            }
        }
    }
    names
}

/// The leading segments that name no module of this workspace: the fixed list, the workspace's own
/// crates and the dependencies its root manifest declares.
fn foreign_names(root: &Path) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = NOT_A_WORKSPACE_MODULE
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    for krate in crate_dirs(root) {
        if let Some(name) = krate.file_name() {
            names.insert(name.to_string_lossy().replace('-', "_"));
        }
    }
    if let Ok(manifest) = fs::read_to_string(root.join("Cargo.toml")) {
        names.extend(dependency_names(&manifest));
    }
    names
}

/// Classifies a span; `None` when it is neither a module path nor a name.
fn parse<'a>(span: &'a str, foreign: &BTreeSet<String>) -> Option<Candidate<'a>> {
    let body = span.strip_suffix("()").unwrap_or(span);
    if !body.contains("::") {
        let is_name = is_ident(body) && starts_lowercase(body) && body.contains('_');
        return is_name.then_some(Candidate::Name(body));
    }
    let segments: Vec<&str> = body.split("::").collect();
    let first = *segments.first()?;
    if segments.len() < 2
        || !segments.iter().all(|segment| is_ident(segment))
        || !starts_lowercase(first)
        || foreign.contains(first)
    {
        return None;
    }
    let lowercase = segments
        .iter()
        .take_while(|segment| starts_lowercase(segment))
        .count();
    let (modules, rest) = segments.split_at(lowercase);
    Some(Candidate::Path {
        modules: modules.to_vec(),
        rest: rest.to_vec(),
    })
}

/// The source files that define the module `modules` in any workspace crate:
/// `src/<a>/<b>.rs` and `src/<a>/<b>/mod.rs` for the module `a::b`.
fn module_files(root: &Path, modules: &[&str]) -> Vec<PathBuf> {
    let Some((last, parents)) = modules.split_last() else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for krate in crate_dirs(root) {
        let mut dir = krate.join("src");
        for parent in parents {
            dir.push(parent);
        }
        for candidate in [
            dir.join(format!("{last}.rs")),
            dir.join(last).join("mod.rs"),
        ] {
            if candidate.is_file() {
                files.push(candidate);
            }
        }
    }
    files
}

/// Whether `word` appears in `text` bounded by non-identifier characters on both sides.
fn mentions(text: &str, word: &str) -> bool {
    let is_ident_byte = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let bytes = text.as_bytes();
    text.match_indices(word).any(|(start, _)| {
        let bounded_before = start == 0 || !is_ident_byte(bytes[start - 1]);
        let bounded_after = bytes
            .get(start + word.len())
            .is_none_or(|b| !is_ident_byte(*b));
        bounded_before && bounded_after
    })
}

/// `text` with comments, doc comments, and string and character literals replaced by a space, so
/// that what is left is code and a word in prose cannot pass for a declaration.
fn strip_non_code(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let at = |offset: usize| chars.get(i + offset).copied();
        if c == '/' && at(1) == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && at(1) == Some('*') {
            let mut depth = 1_usize;
            i += 2;
            while i < chars.len() && depth > 0 {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            out.push(' ');
        } else if let Some((prefix, hashes)) =
            raw_string_opening(&chars[i..]).filter(|_| i == 0 || !is_ident(chars[i - 1]))
        {
            i += prefix + hashes + 1;
            let closing: Vec<char> = std::iter::once('"')
                .chain(std::iter::repeat_n('#', hashes))
                .collect();
            while i < chars.len() && !chars[i..].starts_with(&closing) {
                i += 1;
            }
            i += closing.len();
            out.push(' ');
        } else if c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += if chars[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            out.push(' ');
        } else if c == '\'' {
            if at(1) == Some('\\') {
                i += 2;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else if at(2) == Some('\'') {
                i += 3;
            } else {
                // A lifetime: the tick and its name are not a declaration.
                i += 1;
                while i < chars.len() && is_ident(chars[i]) {
                    i += 1;
                }
            }
            out.push(' ');
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// When `chars` starts a raw string (`r"`, `r#"`, `br"`, `cr##"`, ...), the length of its prefix
/// (`r`, `br` or `cr`) and the number of `#` in its fence.
fn raw_string_opening(chars: &[char]) -> Option<(usize, usize)> {
    let prefix = match chars {
        ['b' | 'c', 'r', ..] => 2,
        ['r', ..] => 1,
        _ => return None,
    };
    let rest = &chars[prefix..];
    let hashes = rest.iter().take_while(|c| **c == '#').count();
    (rest.get(hashes) == Some(&'"')).then_some((prefix, hashes))
}

/// A token of stripped code: a word, or one punctuation character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token<'a> {
    Word(&'a str),
    Punct(char),
}

/// Splits stripped code into words (identifier characters) and single punctuation characters.
fn tokenize(code: &str) -> Vec<Token<'_>> {
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut tokens = Vec::new();
    let mut word_start = None;
    for (index, c) in code
        .char_indices()
        .chain(std::iter::once((code.len(), ' ')))
    {
        match (is_word(c), word_start) {
            (true, None) => word_start = Some(index),
            (false, Some(start)) => {
                tokens.push(Token::Word(&code[start..index]));
                word_start = None;
            }
            _ => {}
        }
        if !is_word(c) && !c.is_whitespace() {
            tokens.push(Token::Punct(c));
        }
    }
    tokens
}

/// The keywords whose next word is the name of the item being declared.
const DECLARING: [&str; 9] = [
    "mod", "fn", "struct", "enum", "trait", "type", "const", "static", "union",
];

/// The index just past the group that opens at `tokens[open]` with `opening` and closes with the
/// matching `closing`, or the end of `tokens` when it never closes.
fn past_group(tokens: &[Token<'_>], open: usize, opening: char, closing: char) -> usize {
    let mut depth = 0_usize;
    for (offset, token) in tokens[open..].iter().enumerate() {
        if *token == Token::Punct(opening) {
            depth += 1;
        } else if *token == Token::Punct(closing) {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return open + offset + 1;
            }
        }
    }
    tokens.len()
}

/// The index just past the item that starts at `tokens[index]`, when it is one `declares` does not
/// read: a `#[cfg(test)]` module with a body, or an `impl` block.
fn unread_item_end(tokens: &[Token<'_>], index: usize) -> Option<usize> {
    const CFG_TEST: [Token<'_>; 7] = [
        Token::Punct('#'),
        Token::Punct('['),
        Token::Word("cfg"),
        Token::Punct('('),
        Token::Word("test"),
        Token::Punct(')'),
        Token::Punct(']'),
    ];
    if tokens[index..].starts_with(&CFG_TEST) {
        let mut at = index + CFG_TEST.len();
        loop {
            match tokens.get(at) {
                Some(Token::Punct('#')) if tokens.get(at + 1) == Some(&Token::Punct('[')) => {
                    at = past_group(tokens, at + 1, '[', ']');
                }
                Some(Token::Word("pub")) => {
                    at += 1;
                    if tokens.get(at) == Some(&Token::Punct('(')) {
                        at = past_group(tokens, at, '(', ')');
                    }
                }
                _ => break,
            }
        }
        let is_module = tokens.get(at) == Some(&Token::Word("mod"))
            && matches!(tokens.get(at + 1), Some(Token::Word(_)))
            && tokens.get(at + 2) == Some(&Token::Punct('{'));
        return is_module.then(|| past_group(tokens, at + 2, '{', '}'));
    }
    // An `impl` that starts an item follows the end of the previous one; one that follows `->`,
    // `:` or `(` is a type, and the `{` after it is a function body.
    let starts_item = matches!(
        index.checked_sub(1).map(|at| &tokens[at]),
        None | Some(Token::Punct(';' | '{' | '}' | ']') | Token::Word("unsafe"))
    );
    if tokens[index] != Token::Word("impl") || !starts_item {
        return None;
    }
    let open = tokens[index..]
        .iter()
        .take_while(|token| **token != Token::Punct(';'))
        .position(|token| *token == Token::Punct('{'))?;
    Some(past_group(tokens, index + open, '{', '}'))
}

/// `tokens` without the items `declares` does not read.
fn read_tokens<'a>(tokens: &[Token<'a>]) -> Vec<Token<'a>> {
    let mut kept = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while index < tokens.len() {
        if let Some(end) = unread_item_end(tokens, index) {
            index = end;
        } else {
            kept.push(tokens[index]);
            index += 1;
        }
    }
    kept
}

/// Whether the group that opens at `tokens[open]`, the `{` of `use a::{...}`, lists `self` itself
/// (not `self as name`, and not inside a nested group).
fn group_lists_self(tokens: &[Token<'_>], open: usize) -> bool {
    let mut depth = 0_usize;
    for (offset, token) in tokens[open..].iter().enumerate() {
        match token {
            Token::Punct('{') => depth += 1,
            Token::Punct('}') => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return false;
                }
            }
            Token::Word("self") if depth == 1 => {
                let at = open + offset;
                let after_open_or_comma =
                    matches!(tokens.get(at - 1), Some(Token::Punct('{' | ',')));
                let before_comma_or_close =
                    matches!(tokens.get(at + 1), Some(Token::Punct(',' | '}')));
                if after_open_or_comma && before_comma_or_close {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Whether `source` declares `item`: a declaring keyword before it, a `macro_rules!` definition, or
/// a re-export that brings the name in (the last segment of a path, the name after `as`, or `a` in
/// `a::{self}`). Items inside an `impl` block or a `#[cfg(test)]` module are not read.
fn declares(source: &str, item: &str) -> bool {
    let code = strip_non_code(source);
    let tokens = read_tokens(&tokenize(&code));
    let mut in_reexport = false;
    for (index, token) in tokens.iter().enumerate() {
        let previous = index.checked_sub(1).map(|at| &tokens[at]);
        match token {
            Token::Punct(';') => in_reexport = false,
            // A `)` before `use` closes the `pub(crate)`-style visibility.
            Token::Word("use") => {
                in_reexport = matches!(previous, Some(Token::Word("pub") | Token::Punct(')')));
            }
            Token::Word(word) if *word == item => {
                let before_that = index.checked_sub(2).map(|at| &tokens[at]);
                let after = tokens.get(index + 1);
                let after_that = tokens.get(index + 2);
                let keyword = matches!(previous, Some(Token::Word(kw)) if DECLARING.contains(kw));
                let macro_rule = previous == Some(&Token::Punct('!'))
                    && before_that == Some(&Token::Word("macro_rules"));
                let alias = in_reexport && previous == Some(&Token::Word("as"));
                let path_prefix =
                    after == Some(&Token::Punct(':')) && after_that == Some(&Token::Punct(':'));
                let grouped_self = in_reexport
                    && path_prefix
                    && tokens.get(index + 3) == Some(&Token::Punct('{'))
                    && group_lists_self(&tokens, index + 3);
                let renamed = after == Some(&Token::Word("as"));
                let imported = in_reexport && !path_prefix && !renamed;
                if keyword || macro_rule || alias || grouped_self || imported {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Every `.rs` file under `crates/` and every `Cargo.toml` of the workspace, concatenated, except
/// this crate's own: its tests spell stale names on purpose, and a guard cannot vouch for itself.
fn sources_text(root: &Path) -> String {
    let own = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut text = fs::read_to_string(root.join("Cargo.toml")).unwrap_or_default();
    for entry in WalkDir::new(root.join("crates"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && !entry.path().starts_with(own))
    {
        let name = entry.file_name().to_string_lossy();
        if name.ends_with(".rs") || name == "Cargo.toml" {
            text.push_str(&fs::read_to_string(entry.path()).unwrap_or_default());
            text.push('\n');
        }
    }
    text
}

/// Whether `name` is one of the deliberate bad examples `CLAUDE.md` may cite.
fn is_anti_example(name: &str) -> bool {
    ANTI_EXAMPLES.iter().any(|(listed, _)| *listed == name)
}

/// Why a module path does not resolve, or `Ok` when it does.
fn resolve_path(root: &Path, span: &str, modules: &[&str], rest: &[&str]) -> Result<(), String> {
    let found = (1..=modules.len())
        .rev()
        .map(|len| (len, module_files(root, &modules[..len])))
        .find(|(_, files)| !files.is_empty());
    let Some((len, files)) = found else {
        let first = modules[0];
        return Err(format!(
            "`{span}`: no workspace crate has a module `{first}` (looked for src/{first}.rs and src/{first}/mod.rs in each crate)",
        ));
    };
    let item = modules[len..].iter().chain(rest).next();
    let Some(item) = item else {
        return Ok(());
    };
    let defined = files
        .iter()
        .filter_map(|file| fs::read_to_string(file).ok())
        .any(|text| declares(&text, item));
    if defined {
        Ok(())
    } else {
        Err(format!(
            "`{span}`: module `{}` has no `{item}`",
            modules[..len].join("::"),
        ))
    }
}

/// Every module path or name in `text`, the content of the document `label`, that does not
/// resolve; one line each.
pub(super) fn problems(label: &str, text: &str, root: &Path) -> Vec<String> {
    let foreign = foreign_names(root);
    let corpus = OnceCell::new();
    let mut found = Vec::new();
    for span in code_spans(text) {
        let verdict = match parse(&span.text, &foreign) {
            None => continue,
            Some(Candidate::Path { modules, rest }) => {
                resolve_path(root, &span.text, &modules, &rest)
            }
            Some(Candidate::Name(name)) => {
                let sources = corpus.get_or_init(|| sources_text(root));
                if is_anti_example(name) || mentions(sources, name) {
                    Ok(())
                } else {
                    Err(format!(
                        "`{}`: appears in no `.rs` or `Cargo.toml` file of the workspace",
                        span.text
                    ))
                }
            }
        };
        if let Err(why) = verdict {
            found.push(format!("{label}:{}: cites {why}", span.line));
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

    use crate::workspace::root;

    fn foreign() -> BTreeSet<String> {
        ["airsl", "airsl_cli", "mlua", "serde_json", "tempfile"]
            .iter()
            .chain(NOT_A_WORKSPACE_MODULE.iter())
            .map(|name| (*name).to_owned())
            .collect()
    }

    #[test]
    fn module_paths_are_recognised_by_their_leading_lowercase_segments() {
        let foreign = foreign();
        assert_eq!(
            parse("extension::manifest::validate_entry", &foreign),
            Some(Candidate::Path {
                modules: vec!["extension", "manifest", "validate_entry"],
                rest: vec![]
            })
        );
        assert_eq!(
            parse("modules::stdlib()", &foreign),
            Some(Candidate::Path {
                modules: vec!["modules", "stdlib"],
                rest: vec![]
            })
        );
        assert_eq!(
            parse("engine::Engine::eval", &foreign),
            Some(Candidate::Path {
                modules: vec!["engine"],
                rest: vec!["Engine", "eval"]
            })
        );
    }

    #[test]
    fn a_single_snake_case_name_with_an_underscore_is_a_name() {
        let foreign = foreign();
        assert_eq!(
            parse("require_loader", &foreign),
            Some(Candidate::Name("require_loader"))
        );
        assert_eq!(
            parse("unwrap_used", &foreign),
            Some(Candidate::Name("unwrap_used"))
        );
    }

    #[test]
    fn spans_that_are_not_workspace_module_paths_are_not_read() {
        let foreign = foreign();
        for span in [
            "airsl::mlua",
            "airsl::modules::stdlib()",
            "airsl_cli::run",
            "mlua::Lua",
            "std::env::set_var",
            "core::fmt",
            "clippy::pedantic",
            "Policy::trusted()",
            "Engine::builder()",
            "Error::MemoryLimit",
            "eval_to::<T>",
            "&mlua::Lua",
            "hash.hash_file",
            "ext.on",
            "engine",
            "Engine",
            "dod-crate",
            "CARGO_TARGET_DIR",
            "a::",
            "::a",
        ] {
            assert_eq!(parse(span, &foreign), None, "{span}");
        }
    }

    #[test]
    fn dependency_names_are_the_keys_of_the_workspace_dependencies_table() {
        let manifest = "[workspace]\nmembers = [\"x\"]\n\n[workspace.dependencies]\n# a note\nmlua = { version = \"0.12\" }\nserde_json = \"1\"\nsome-crate = \"1\"\n\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\n";
        assert_eq!(
            dependency_names(manifest),
            vec!["mlua", "serde_json", "some_crate"]
        );
    }

    #[test]
    fn a_word_is_mentioned_only_when_whole() {
        assert!(mentions("fn validate_entry(", "validate_entry"));
        assert!(mentions("validate_entry", "validate_entry"));
        assert!(mentions("x.validate_entry()", "validate_entry"));
        assert!(!mentions("validate_entry_inner", "validate_entry"));
        assert!(!mentions("revalidate_entry", "validate_entry"));
        assert!(!mentions("", "validate_entry"));
    }

    /// A workspace with `a` holding `src/require/{mod,loader}.rs`, `src/engine.rs` and
    /// `src/modules/mod.rs`, and `b` holding `src/cli.rs`.
    fn workspace() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let files = [
            ("Cargo.toml", "[workspace.dependencies]\nmlua = \"1\"\n"),
            ("crates/a/Cargo.toml", "[lints]\nunwrap_used = \"deny\"\n"),
            (
                "crates/a/src/require/mod.rs",
                "//! The loader answers `require`, and the root table answers its own name.\n/// A script is not a module.\nmod loader;\n",
            ),
            ("crates/a/src/require/loader.rs", "fn recheck_entry() {}\n"),
            ("crates/a/src/engine.rs", "fn dispatch() {}\n"),
            ("crates/a/src/modules/mod.rs", "pub use stdlib::stdlib;\n"),
            ("crates/b/src/cli.rs", "fn main() {}\n"),
        ];
        for (rel, text) in files {
            let path = root.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        tmp
    }

    fn check(span: &str) -> Vec<String> {
        let tmp = workspace();
        problems("CLAUDE.md", &format!("Uses `{span}` here.\n"), tmp.path())
    }

    #[test]
    fn live_module_paths_and_unread_spans_report_nothing() {
        for span in [
            "require::loader",
            "require::loader::recheck_entry",
            "require::loader::recheck_entry()",
            "require::loader::recheck_entry::deeper",
            "engine::dispatch",
            "modules::stdlib()",
            "cli::main",
            "unwrap_used",
            "Policy::trusted()",
            "mlua::Lua",
            "a::engine",
            "std::fs",
        ] {
            assert_eq!(check(span), Vec::<String>::new(), "{span}");
        }
    }

    #[test]
    fn a_stale_module_path_fails_and_names_the_path() {
        let found = check("require_loader::resolve");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
        found[0].starts_with("CLAUDE.md:1: cites `require_loader::resolve`: no workspace crate has a module `require_loader`"),
        "{found:?}"
    );
    }

    #[test]
    fn a_renamed_item_in_a_live_module_fails() {
        let found = check("require::loader::recheck_gone");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("module `require::loader` has no `recheck_gone`"),
            "{found:?}"
        );
        let found = check("require::loading");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("module `require` has no `loading`"),
            "{found:?}"
        );
    }

    #[test]
    fn a_stale_snake_case_name_fails() {
        let found = check("require_loader");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("`require_loader`: appears in no `.rs` or `Cargo.toml` file"),
            "{found:?}"
        );
    }

    #[test]
    fn every_module_path_and_name_cited_in_claude_md_resolves() {
        let root = root();
        let text = fs::read_to_string(root.join("CLAUDE.md")).unwrap();
        let found = problems("CLAUDE.md", &text, &root);
        assert!(
            found.is_empty(),
            "stale module citations:\n{}",
            found.join("\n")
        );
    }

    #[test]
    fn the_guards_own_sources_do_not_vouch_for_a_name() {
        const SENTINEL: &str = "zz_self_vouching_sentinel";
        assert!(!sources_text(&root()).contains(SENTINEL));
    }

    #[test]
    fn a_word_that_only_prose_mentions_is_not_declared() {
        let source = r##"//! The loader and the root of it.
/// Returns the script.
fn real() {}
/* fn block() {} */
let s = "fn quoted()";
let r = r#"struct raw"#;
let b = br#"a " fn hidden_bytes() {}"#;
let c = cr#"a " fn hidden_c() {}"#;
let bs = b"fn bytes() {}";
let cs = c"fn cstr() {}";
fn after() {}
"##;
        for word in [
            "loader",
            "root",
            "script",
            "block",
            "quoted",
            "raw",
            "it",
            "hidden_bytes",
            "hidden_c",
            "bytes",
            "cstr",
        ] {
            assert!(!declares(source, word), "{word}");
        }
        assert!(declares(source, "real"));
        assert!(declares(source, "after"));
    }

    #[test]
    fn every_declaration_form_declares_its_name() {
        for (source, item) in [
            ("mod loader;", "loader"),
            ("pub(crate) mod loader { }", "loader"),
            ("pub fn recheck() {}", "recheck"),
            ("const fn recheck() {}", "recheck"),
            ("pub struct Engine;", "Engine"),
            ("enum Error { A }", "Error"),
            ("union Bits { a: u8 }", "Bits"),
            ("trait Install {}", "Install"),
            ("type Result<T> = T;", "Result"),
            ("pub const LIMIT: u8 = 1;", "LIMIT"),
            ("static TABLE: u8 = 1;", "TABLE"),
            ("macro_rules! check { () => {} }", "check"),
            ("pub use stdlib::stdlib;", "stdlib"),
            ("pub use a::{b, c as d};", "b"),
            ("pub use a::{b, c as d};", "d"),
            ("pub use a::b as renamed;", "renamed"),
        ] {
            assert!(declares(source, item), "{item} in `{source}`");
        }
    }

    #[test]
    fn a_use_path_prefix_and_a_renamed_original_do_not_declare() {
        assert!(!declares("pub use stdlib::stdlib;", "nothere"));
        assert!(!declares("pub use a::b as renamed;", "b"));
        assert!(!declares("pub use stdlib::other;", "stdlib"));
        assert!(!declares("let x = y as usize; call(item);", "item"));
        assert!(!declares("fn f<'static_like>(x: &'static str) {}", "str"));
    }

    #[test]
    fn only_a_pub_use_brings_a_name_in_and_a_private_import_does_not() {
        for source in [
            "pub use a::Shown;",
            "pub(crate) use a::Shown;",
            "pub(super) use a::Shown;",
            "pub(in crate::a) use a::Shown;",
            "#[doc(hidden)] pub use a::Shown;",
        ] {
            assert!(declares(source, "Shown"), "{source}");
        }
        for source in [
            "use a::Shown;",
            "use a::{b, Shown};",
            "use a::b as Shown;",
            "#[cfg(unix)] use a::Shown;",
        ] {
            assert!(!declares(source, "Shown"), "{source}");
        }
        assert!(declares("use a::Hidden; pub use b::Shown;", "Shown"));
        assert!(!declares("use a::Hidden; pub use b::Shown;", "Hidden"));
    }

    #[test]
    fn a_self_import_brings_in_the_module_it_groups() {
        assert!(declares("pub use a::{self};", "a"));
        assert!(declares("pub use a::b::{self, c};", "b"));
        assert!(declares("pub use a::{c, self};", "a"));
        assert!(declares("pub use a::{self as z};", "z"));
        assert!(!declares("pub use a::{self as z};", "a"));
        assert!(!declares("pub use a::{b, c};", "a"));
        assert!(!declares("pub use a::{b::{self}, c};", "a"));
        assert!(declares("pub use a::{b::{self}, c};", "b"));
        assert!(!declares("use a::{self};", "a"));
    }

    #[test]
    fn items_inside_a_test_module_are_not_declared() {
        let source = "fn real() {}\n#[cfg(test)]\nmod tests {\n    use super::*;\n    fn script() { if true { } }\n    struct Fixture;\n}\nfn after() {}\n";
        assert!(!declares(source, "script"));
        assert!(!declares(source, "Fixture"));
        assert!(!declares(source, "tests"));
        assert!(declares(source, "real"));
        assert!(declares(source, "after"));
        let attributes = "#[cfg(test)]\n#[allow(dead_code)]\npub(crate) mod tests { fn inner() {} }\nfn after() {}\n";
        assert!(!declares(attributes, "inner"));
        assert!(declares(attributes, "after"));
    }

    #[test]
    fn a_module_that_is_not_cfg_test_is_still_read() {
        for source in [
            "#[cfg(not(test))]\nmod live { fn inner() {} }",
            "#[cfg(unix)]\nmod live { fn inner() {} }",
            "mod live { fn inner() {} }",
        ] {
            assert!(declares(source, "inner"), "{source}");
        }
    }

    #[test]
    fn associated_items_of_an_impl_block_are_not_declared() {
        let source = "pub struct Engine;\nimpl Engine {\n    pub fn dispatch(&self) { if true { } }\n    const LIMIT: u8 = 1;\n    type Item = u8;\n}\nimpl<T: Clone> Trait for Wrapper<T> where T: Fn() -> u8 {\n    fn install() {}\n}\nunsafe impl Send for Engine {}\nfn after() {}\n";
        for word in ["dispatch", "LIMIT", "Item", "install"] {
            assert!(!declares(source, word), "{word}");
        }
        assert!(declares(source, "Engine"));
        assert!(declares(source, "after"));
    }

    #[test]
    fn an_impl_in_a_type_position_does_not_swallow_the_items_after_it() {
        let source = "fn first() -> impl Iterator<Item = u8> { 0..1 }\nfn second(x: impl Clone) {}\nfn third() {}\n";
        for word in ["first", "second", "third"] {
            assert!(declares(source, word), "{word}");
        }
    }

    #[test]
    fn a_comment_marker_inside_a_string_or_a_character_does_not_hide_code() {
        let source =
            "const URL: &str = \"http://x\"; fn after() {}\nconst Q: char = '\"'; fn next() {}\n";
        assert!(declares(source, "after"));
        assert!(declares(source, "next"));
    }

    #[test]
    fn a_module_whose_prose_mentions_the_word_but_declares_nothing_fails() {
        for span in ["require::script", "require::root", "require::the"] {
            let found = check(span);
            assert_eq!(found.len(), 1, "{span}: {found:?}");
            assert!(
                found[0].contains("module `require` has no"),
                "{span}: {found:?}"
            );
        }
    }

    #[test]
    fn a_renamed_module_file_fails_even_when_the_parent_still_names_it_in_prose() {
        let tmp = workspace();
        fs::remove_file(tmp.path().join("crates/a/src/require/loader.rs")).unwrap();
        fs::write(
            tmp.path().join("crates/a/src/require/mod.rs"),
            "//! The loader module.\n",
        )
        .unwrap();
        let found = problems("CLAUDE.md", "`require::loader`\n", tmp.path());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("module `require` has no `loader`"),
            "{found:?}"
        );
    }

    #[test]
    fn prose_only_items_are_refused_in_the_real_tree() {
        let found = problems(
            "CLAUDE.md",
            "`require::script` and `extension::manifest::the`\n",
            &root(),
        );
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found[0].contains("module `require` has no `script`"),
            "{found:?}"
        );
        assert!(
            found[1].contains("module `extension::manifest` has no `the`"),
            "{found:?}"
        );
    }

    #[test]
    fn a_listed_anti_example_is_not_looked_up_and_an_unlisted_name_still_is() {
        assert_eq!(check("test_confined"), Vec::<String>::new());
        assert_eq!(check("test_unlisted").len(), 1);
        for (name, reason) in ANTI_EXAMPLES {
            assert!(is_ident(name) && !reason.trim().is_empty(), "{name}");
        }
    }
}
