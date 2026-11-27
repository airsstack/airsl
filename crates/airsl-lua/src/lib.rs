//! The airsstack host modules as a loadable Lua library, installed through `LuaRocks`.
//!
//! A Lua script that runs under a stock `lua` or `luajit`, not under `airsl`, writes
//! `require("airsstack")` and gets the same `json`, `path`, `fs`, `env`, `proc`, `regex`, `hash`,
//! `time`, `glob`, `stdio` and `hook` tables an `airsl` script sees. This is its own crate because
//! it builds as a `cdylib` against the interpreter's Lua instead of a Lua of its own, and that
//! build cannot share a Cargo build with the rest of the workspace.
//!
//! What a script gets differs from `airsl` in four ways:
//!
//! - It runs with trusted authority: no grant limits a module, and there is no memory or
//!   instruction ceiling, because the interpreter, not this crate, owns the Lua state.
//! - `ext` is absent. It registers handlers that only an `airsl` engine dispatches.
//! - No global is set: `require` returns the table and `package.loaded.airsstack` holds it.
//! - Process-wide state, such as the `env` overlay and the origin of `time`, is shared between
//!   every Lua state in the process.
//!
//! No inline tests, per the unit-test-mandate exemption #1 (export-only module): this file holds
//! only a `mod` statement. The crate has no Rust tests at all, because a module-mode build cannot
//! link one; `lua/check.lua` loads the built library into a real interpreter and tests it there.
//!
//! Responsibilities: the crate documentation, and exposing the entry point.
//!
//! Non-responsibilities: the modules themselves, which live in `airsl`.

mod entry;
