//! Guards that keep the shipped prose true to the shipped code.
//!
//! Its own crate, and unpublished, because the claims it guards belong to no runtime module and to
//! no one crate: an example README's `## Output` block is a promise about what a program prints, a
//! source path or module path cited in `CLAUDE.md` or a README is a promise about where code
//! lives, a book chapter's `{{#include}}` is a promise that a file exists, a link into the
//! published book is a promise that a chapter keeps its name, a rockspec is a promise that it
//! follows the workspace version, and a `cargo make` task named in prose is a promise that
//! `Makefile.toml` declares it. Each rots the day a message is reworded or a file moves, and
//! nothing in the build notices. They read the whole workspace (the repository `CLAUDE.md`, the
//! book, every crate's READMEs), so homing them in one published crate would ship workspace-wide
//! checks inside that crate's package. As a member of its own they run under every form of
//! `cargo test --workspace`, and so under the gate and the CI matrix on every operating system,
//! with no shell script or extra gate step.
//!
//! The crate has no runtime code: its modules are compiled for tests only, and each module's
//! tests are the guard.
//!
//! Responsibilities:
//!
//! - `examples`: running each example, the Rust ones of `airsl` and the Lua ones of the native
//!   module, and requiring its README's `## Output` block to equal what it printed, and its marker
//!   files to be named in the expected lists; and keeping the `examples` task of `Makefile.toml`
//!   from comparing output with README text.
//! - `citations`: requiring the source paths, module paths and book include targets that prose
//!   cites to exist, and refusing line-number citations.
//! - `book_links`: requiring each address into the published book that a source file, `CLAUDE.md`
//!   or a README carries to name a chapter the book lists, and its anchor to be a heading of it.
//! - `rockspecs`: requiring the rockspec file names, the `package`, `version` and `tag` inside
//!   them, and the published members' versions to follow the workspace version.
//! - `tasks`: requiring every `cargo make` task that `CLAUDE.md`, a README or a book chapter runs
//!   to be declared in `Makefile.toml`, and `CLAUDE.md` to run every step of `dod` and of the
//!   steps it lists as `cargo make <step>`.
//! - `workspace`: locating the workspace root, its crates and the documents the guards read.
//!
//! Non-responsibilities: the Lua transcripts of the book appendices and the compile-and-run sweep
//! of shipped `.lua` files, which stay with `cargo make examples`; and whether the book builds,
//! which is the `mdbook build` step of CI.

#[cfg(test)]
mod book_links;
#[cfg(test)]
mod citations;
#[cfg(test)]
mod examples;
#[cfg(test)]
mod rockspecs;
#[cfg(test)]
mod tasks;
#[cfg(test)]
mod workspace;
