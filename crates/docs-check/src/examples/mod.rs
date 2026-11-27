//! Example READMEs against the examples themselves.
//!
//! An example README's `## Output` block is a promise about what a program prints, and the only
//! way to keep it is to run the program. There are two kinds of example, run in two different
//! ways, and one rule for what their READMEs say, so each has a file: `rust` for the examples of
//! the `airsl` crate, `lua` for the Lua examples of the native module, and `readme` for the
//! promise both make.
//!
//! Responsibilities:
//!
//! - `rust`: running each `airsl` example through cargo and comparing its stdout with its README.
//! - `lua`: running each native-module example under the interpreter or interpreters it is for,
//!   against the built module, and comparing its stdout with its README; the Neovim ones are
//!   compared by a test that only an explicit request runs.
//! - `readme`: reading an example directory's `## Output` blocks and marker files.
//!
//! Non-responsibilities: the Lua transcripts of the book appendices and compiling the shipped
//! `.lua` files, which stay with `cargo make examples`.

mod lua;
mod readme;
mod rust;
