//! What shipped prose says about where things live, checked against the tree.
//!
//! A citation is a promise that a file, a module or a directive's target exists, and each kind has
//! its own grammar and its own rule for what it resolves against, so each has a file: `source`
//! for `.rs` paths, `module` for `a::b` module paths, `include` for the book's file-reading
//! directives. `span` is the one thing they share, the reader that finds the code spans of a
//! document.
//!
//! Citations name a symbol or a path, never a line number: a line citation is refused outright,
//! because it goes stale on the next edit that touches the file above it.
//!
//! Responsibilities:
//!
//! - `source`: requiring each `.rs` path in a code span to exist, and refusing a line suffix.
//! - `module`: requiring each module path and `snake_case` name in `CLAUDE.md` to resolve.
//! - `include`: requiring the target of each `{{#include}}`, `{{#rustdoc_include}}` and
//!   `{{#playground}}` directive in a book chapter to be a file.
//! - `span`: finding the code spans of a document, outside fenced blocks.
//!
//! Non-responsibilities: whether a cited symbol is inside the cited file, and the examples'
//! `## Output` blocks, which the sibling `examples` module owns.

mod include;
mod module;
mod source;
mod span;
