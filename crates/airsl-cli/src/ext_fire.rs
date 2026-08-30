//! `airsl ext fire`: load one extension and dispatch one event.
//!
//! Its own module because this is the CLI's only path that *runs* extension code, and it goes
//! through `ExtensionHost` unchanged — so what `fire` does is, by construction, what a host
//! program does. Nothing here bypasses the load sequence.
//!
//! Responsibilities: payload parsing, the single load and dispatch, byte-stable output, exit codes.
//!
//! Non-responsibilities: reporting the negotiation (`ext_doctor`); the load sequence itself.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::io::{Read, Write};
use std::path::Path;

use airsl::extension::Ceiling;
use airsl::{EventName, ExtensionHost};
use serde_json::{Map, Value};

use crate::cli::{ExtFlags, resolve_ceiling};

/// Prefix every diagnostic `fire` writes to stderr.
const PREFIX: &str = "airsl ext fire: ";

/// Writes one diagnostic line to `stderr`, prefixed the same way every failure path in this
/// module reports one — the single call shape every `Err` arm below uses, so there is nothing
/// left to drift between them, and the write goes through the injected `stderr` so a test can
/// capture it instead of the process's real stderr.
fn report_error(stderr: &mut impl Write, error: impl core::fmt::Display) {
    let _ = writeln!(stderr, "{PREFIX}{error}");
}

/// Builds the host that `dir`'s extension loads into: `event_names` declared, `vars` available to
/// the manifest.
fn build_host(
    ceiling: Ceiling,
    event_names: &[String],
    vars: Vec<(String, String)>,
) -> airsl::Result<ExtensionHost> {
    ExtensionHost::builder()
        .ceiling(ceiling)
        .events(event_names)?
        .variables(vars)
        .build()
}

/// Parses the payload from `raw`: trimmed-empty stdin is an empty object, otherwise it must be
/// valid JSON.
fn parse_payload(raw: &str) -> Result<Value, serde_json::Error> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Ok(Value::Object(Map::default()))
    } else {
        serde_json::from_str(trimmed)
    }
}

/// Writes `value` (or `null`) to `stdout` with a trailing newline, and returns the exit code.
fn write_result(stdout: &mut impl Write, stderr: &mut impl Write, value: Option<Value>) -> i32 {
    // `serde_json::Value::Object` is a `BTreeMap` in this workspace (the `preserve_order` feature
    // is off), so `to_string` already renders object keys sorted at every depth — there is nothing
    // left for a Lua table's own construction order to leak into the output, and no sorter is
    // duplicated here.
    let rendered = match value {
        Some(value) => match serde_json::to_string(&value) {
            Ok(text) => text,
            Err(error) => {
                report_error(stderr, error);
                return 1;
            }
        },
        None => "null".to_owned(),
    };
    if let Err(error) = writeln!(stdout, "{rendered}") {
        report_error(stderr, error);
        return 1;
    }
    0
}

/// Loads the extension at `dir` and dispatches `event` to it, reading the payload from `stdin`
/// and writing the result to `stdout`. Returns the process exit code.
///
/// A resource-limit breach is reported the same way every other failure is: `fire` has no
/// fail-open mode to swallow it under, so [`airsl::Error::exhausted_limit`] never needs a special
/// case here the way [`crate::run::run`]'s does. Every diagnostic goes through the injected
/// `stderr`, the same `&mut impl Write` shape `stdout` already uses, so a unit test can assert on
/// what `fire` reports without touching the process's real stderr.
pub(crate) fn run(
    dir: &Path,
    event: &str,
    mut flags: ExtFlags,
    mut stdin: impl Read,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    let mut raw = String::new();
    if let Err(error) = stdin.read_to_string(&mut raw) {
        report_error(stderr, error);
        return 1;
    }
    let payload = match parse_payload(&raw) {
        Ok(payload) => payload,
        Err(error) => {
            report_error(stderr, error);
            return 1;
        }
    };

    let event_name = match EventName::new(event) {
        Ok(event_name) => event_name,
        Err(error) => {
            report_error(stderr, &error);
            return 1;
        }
    };

    let vars = std::mem::take(&mut flags.vars);
    let mut event_names = vec![event.to_owned()];
    event_names.extend(std::mem::take(&mut flags.events));

    let ceiling = match resolve_ceiling(flags) {
        Ok(ceiling) => ceiling,
        Err(error) => {
            report_error(stderr, &error);
            return 1;
        }
    };

    let mut host = match build_host(ceiling, &event_names, vars) {
        Ok(host) => host,
        Err(error) => {
            report_error(stderr, &error);
            return 1;
        }
    };

    let extension = match host.load(dir) {
        Ok(extension) => extension,
        Err(error) => {
            report_error(stderr, &error);
            return 1;
        }
    };

    match extension.call(&event_name, &payload) {
        Ok(value) => write_result(stdout, stderr, value),
        Err(error) => {
            report_error(stderr, &error);
            1
        }
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use crate::test_support::script_literal;
    use std::fs;
    use std::io::Cursor;
    use std::path::Path;

    use tempfile::TempDir;

    use super::{PREFIX, run};
    use crate::cli::{ExtFlags, Grants, LimitOverride};
    use airsl::InstructionLimit;

    /// Writes `extension.toml` (extended by `manifest_extra`) + `main.lua` into a fresh directory.
    fn fixture(name: &str, manifest_extra: &str, lua: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("extension.toml"),
            format!(
                "[extension]\nname = \"{name}\"\nversion = \"0.1.0\"\nentry = \"main.lua\"\napi = 1\n{manifest_extra}"
            ),
        )
        .unwrap();
        fs::write(dir.path().join("main.lua"), lua).unwrap();
        dir
    }

    /// Runs `fire` against `dir` and returns `(exit code, stdout, stderr)`.
    fn fire(dir: &Path, event: &str, flags: ExtFlags, input: &str) -> (i32, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run(
            dir,
            event,
            flags,
            Cursor::new(input.as_bytes()),
            &mut out,
            &mut err,
        );
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn fire_round_trips_a_payload_through_the_handler() {
        let dir = fixture(
            "echo",
            "",
            r#"airsstack.ext.on("count", function(p) return { echo = p.n * 2 } end)"#,
        );

        let (code, out, _err) = fire(dir.path(), "count", ExtFlags::default(), r#"{"n": 21}"#);

        assert_eq!(code, 0);
        assert_eq!(out, "{\"echo\":42}\n");
    }

    #[test]
    fn empty_stdin_is_an_empty_object() {
        let dir = fixture(
            "echo",
            "",
            r#"airsstack.ext.on("count", function(p) return p end)"#,
        );

        let (code, out, _err) = fire(dir.path(), "count", ExtFlags::default(), "");

        assert_eq!(code, 0);
        assert_eq!(out, "{}\n");
    }

    #[test]
    fn whitespace_only_stdin_is_an_empty_object() {
        let dir = fixture(
            "echo",
            "",
            r#"airsstack.ext.on("count", function(p) return p end)"#,
        );

        let (code, out, _err) = fire(dir.path(), "count", ExtFlags::default(), "   \n\t  ");

        assert_eq!(code, 0);
        assert_eq!(out, "{}\n");
    }

    #[test]
    fn no_handler_prints_null() {
        let dir = fixture("quiet", "", "return 1");

        let (code, out, _err) = fire(dir.path(), "count", ExtFlags::default(), "{}");

        assert_eq!(code, 0);
        assert_eq!(out, "null\n");
    }

    #[test]
    fn keys_are_sorted_in_the_output() {
        let dir = fixture(
            "sorter",
            "",
            r#"airsstack.ext.on("count", function()
                return { b = 1, a = 2, nested = { z = 1, y = 2 } }
            end)"#,
        );

        let (code, out, _err) = fire(dir.path(), "count", ExtFlags::default(), "{}");

        assert_eq!(code, 0);
        assert_eq!(out, "{\"a\":2,\"b\":1,\"nested\":{\"y\":2,\"z\":1}}\n");
    }

    #[test]
    fn a_handler_error_is_exit_one_with_the_message_on_stderr() {
        let dir = fixture(
            "erroring",
            "",
            r#"airsstack.ext.on("count", function() error("boom") end)"#,
        );

        let (code, out, err) = fire(dir.path(), "count", ExtFlags::default(), "{}");

        assert_eq!(code, 1);
        assert!(out.is_empty(), "{out}");
        assert!(err.starts_with(PREFIX), "{err}");
        assert!(err.contains("boom"), "{err}");
    }

    #[test]
    fn a_limit_breach_is_always_reported() {
        let dir = fixture(
            "runaway",
            "",
            r#"airsstack.ext.on("count", function() while true do end end)"#,
        );
        let flags = ExtFlags {
            instruction_limit: Some(LimitOverride::Of(InstructionLimit::count(1000))),
            ..ExtFlags::default()
        };

        let (code, out, err) = fire(dir.path(), "count", flags, "{}");

        assert_eq!(code, 1);
        assert!(out.is_empty(), "{out}");
        assert!(err.starts_with(PREFIX), "{err}");
        assert!(err.contains("instruction"), "{err}");
    }

    #[test]
    fn invalid_json_on_stdin_is_exit_one_before_any_load() {
        let dir = TempDir::new().unwrap();
        let marker = dir.path().join("marker");
        fs::write(
            dir.path().join("extension.toml"),
            format!(
                "[extension]\nname = \"marker\"\nversion = \"0.1.0\"\nentry = \"main.lua\"\napi = 1\n\
                 [capabilities]\nfs.write = [\"{}\"]\n",
                script_literal(dir.path())
            ),
        )
        .unwrap();
        fs::write(
            dir.path().join("main.lua"),
            format!("airsstack.fs.write('{}', 'ran')\n", script_literal(&marker)),
        )
        .unwrap();
        let flags = ExtFlags {
            grants: Grants {
                write: vec![dir.path().to_path_buf()],
                ..Grants::default()
            },
            ..ExtFlags::default()
        };

        let (code, out, _err) = fire(dir.path(), "count", flags, "not json");

        assert_eq!(code, 1);
        assert!(out.is_empty(), "{out}");
        assert!(!marker.exists(), "the entry must not have run");
    }

    #[test]
    fn a_granted_write_root_reaches_the_handler() {
        let dir = TempDir::new().unwrap();
        let marker = dir.path().join("marker");
        fs::write(
            dir.path().join("extension.toml"),
            format!(
                "[extension]\nname = \"writer\"\nversion = \"0.1.0\"\nentry = \"main.lua\"\napi = 1\n\
                 [capabilities]\nfs.write = [\"{}\"]\n",
                script_literal(dir.path())
            ),
        )
        .unwrap();
        fs::write(
            dir.path().join("main.lua"),
            format!(
                r#"airsstack.ext.on("count", function() airsstack.fs.write("{}", "ran") end)"#,
                script_literal(&marker)
            ),
        )
        .unwrap();
        let flags = ExtFlags {
            grants: Grants {
                write: vec![dir.path().to_path_buf()],
                ..Grants::default()
            },
            ..ExtFlags::default()
        };

        let (code, out, _err) = fire(dir.path(), "count", flags, "{}");

        assert_eq!(code, 0, "{out}");
        assert!(
            marker.exists(),
            "the handler's write must have reached the granted root"
        );
    }

    #[test]
    fn a_var_flag_resolves_a_manifest_variable() {
        let dir = fixture(
            "vartest",
            "[capabilities]\n[capabilities.optional]\nfs.read = [\"$APP_HOME/data\"]\n",
            "return 1",
        );

        // Without `--var`, the manifest cannot resolve `$APP_HOME`, so the load never happens.
        let (without_code, without_out, _without_err) =
            fire(dir.path(), "count", ExtFlags::default(), "{}");
        assert_eq!(without_code, 1, "{without_out}");

        // With `--var APP_HOME=<dir>`, the manifest resolves; the unfunded optional `fs.read` is
        // merely a reduction, not a denial, so the extension still loads and dispatches.
        let flags = ExtFlags {
            vars: vec![("APP_HOME".to_owned(), dir.path().display().to_string())],
            ..ExtFlags::default()
        };
        let (with_code, with_out, _with_err) = fire(dir.path(), "count", flags, "{}");
        assert_eq!(with_code, 0, "{with_out}");
        assert_eq!(with_out, "null\n");
    }

    #[test]
    fn extra_events_from_flags_are_declared_too() {
        let dir = fixture(
            "multi",
            "",
            r#"
            airsstack.ext.on("count", function() return 1 end)
            airsstack.ext.on("other", function() return 2 end)
            "#,
        );
        let flags = ExtFlags {
            events: vec!["other".to_owned()],
            ..ExtFlags::default()
        };

        let (code, out, _err) = fire(dir.path(), "count", flags, "{}");

        assert_eq!(code, 0);
        assert_eq!(out, "1\n");
    }
}
