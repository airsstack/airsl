//! `airsl lua`: the subcommands that prepare a project for Lua tooling.
//!
//! A folder because dispatch, the native-module flow and the process boundary are distinct roles,
//! each with its own failure modes and tests; one file would mix what to do, what to run and how
//! to run it. Export-only per unit-test exemption #1: this file holds declarations and no logic.
//!
//! Responsibilities:
//!
//! - [`command`] — the `lua` dispatch: runs the subcommand, prints its report and maps failures
//!   to an exit code.
//! - [`luarocks`] — the only process spawn in the CLI, behind a trait.
//! - [`native`] — decides which native package to install and what each failure says.
//!
//! Non-responsibilities: parsing arguments (`cli`).

#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

mod command;
mod luarocks;
mod native;

pub(crate) use command::run;
