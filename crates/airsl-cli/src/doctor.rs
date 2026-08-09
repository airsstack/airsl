//! The `airsl doctor` report.
//!
//! Exists because the plugin hooks that call this binary fail open when it is missing — a hook
//! whose runtime is absent does nothing and says nothing, which is correct behaviour and terrible
//! diagnostics. `doctor` is the one command that answers "is it installed, and what can scripts
//! see" without needing a script to run.
//!
//! Responsibilities: [`report`], which renders the runtime summary.
//!
//! Non-responsibilities: fixing anything. The report is read-only.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use core::fmt::Write as _;

use airsl::{Engine, Sandbox};

/// Renders the runtime summary.
///
/// Returns the text to print rather than printing it, so the shape is testable. A runtime that
/// cannot be built is reported as a status line rather than an error, because the whole point of
/// the command is to describe a broken installation.
#[must_use]
pub(crate) fn report() -> String {
    let mut out = format!("airsl {}\n", env!("CARGO_PKG_VERSION"));

    match Engine::builder().sandbox(Sandbox::Restricted).build() {
        Ok(engine) => {
            let modules: Vec<_> = engine
                .module_names()
                .iter()
                .map(ToString::to_string)
                .collect();
            let _ = writeln!(out, "  lua:     {}", lua_version(&engine));
            out.push_str("  sandbox: restricted\n");
            let _ = writeln!(out, "  modules: {}", modules.join(", "));
        }
        Err(error) => {
            let _ = writeln!(out, "  status:  unusable ({error})");
        }
    }
    out
}

/// Asks the Lua state for its own version string.
fn lua_version(engine: &Engine) -> String {
    engine
        .lua()
        .globals()
        .get::<String>("_VERSION")
        .unwrap_or_else(|_| String::from("unknown"))
}

#[cfg(test)]
mod tests {
    use super::report;

    #[test]
    fn the_report_names_the_binary_and_its_version() {
        let text = report();
        assert!(text.starts_with("airsl "), "{text}");
        assert!(text.contains(env!("CARGO_PKG_VERSION")), "{text}");
    }

    #[test]
    fn the_report_names_the_lua_version() {
        let text = report();
        assert!(text.contains("Lua 5.4"), "{text}");
    }

    #[test]
    fn the_report_lists_the_installed_modules() {
        let text = report();
        assert!(text.contains("modules:"), "{text}");
        assert!(text.contains("json"), "{text}");
    }

    #[test]
    fn the_report_states_the_sandbox_it_probed() {
        assert!(report().contains("sandbox: restricted"));
    }

    #[test]
    fn the_report_ends_with_a_newline() {
        assert!(report().ends_with('\n'));
    }
}
