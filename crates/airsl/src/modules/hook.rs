//! The `airsstack.hook` host module: the agent-hook contract, in one place.
//!
//! A thin layer over `stdio` and `json`, and worth being its own module for one reason: the shape
//! a hook must emit is easy to get subtly wrong, and before this existed every plugin script
//! rebuilt it by hand — in Python, in Node and in `printf`. The nesting is real:
//!
//! ```json
//! {"hookSpecificOutput": {"hookEventName": "PreToolUse", "additionalContext": "…"}}
//! ```
//!
//! `context` builds and emits that envelope. Three plugin scripts use it rather than assembling
//! the envelope by hand: `enforce.lua`'s `PreToolUse` hook, `concise-tracker.lua`'s
//! `UserPromptSubmit` hook, and `airsstack-journal`'s `session-start.lua`'s `SessionStart` hook. It
//! models only `hookEventName` and `additionalContext`, deliberately with no `permissionDecision`
//! field. This was watched directly against the CLI installed on the machine that built this
//! module, not read out of a specific build's documentation — no version is claimed, and the exact
//! conditions could shift release to release. What was watched: a hook returning
//! `permissionDecision: defer` can have the tool call it fired on swallowed outright — the response
//! carries no `tool_result` at all — when the session is non-interactive, the tool batch is solo,
//! and the abort signal is not already set, which strands the caller with no record the call ever
//! happened. Other cases still produce a result: an interactive session or a multi-tool batch just
//! warns and lets the tool run normally, and an already-aborted signal still pushes a `tool_result`
//! carrying a `cancelled` denial. `additionalContext` alone was confirmed to carry no such risk:
//! emitting an envelope with `hookEventName` and `additionalContext` but no `permissionDecision`
//! still injects the context into the model's turn while the tool call it fired on returns its
//! normal `tool_result`.
//!
//! Responsibilities: [`Hook`], installing `payload`, `emit` and `context`. `emit` writes an
//! arbitrary Lua value to stdout as JSON with no envelope shape imposed — the unmodelled escape
//! hatch for anything `context` doesn't cover.
//!
//! Non-responsibilities: deciding whether the hook should have fired, and exiting. A hook that
//! fails must not turn its failure into a non-zero exit — that is [`crate::FailurePolicy`]'s job,
//! and the CLI's `--fail-open`.

use crate::convert;
use crate::error::{Error, Result};
use crate::modules::stdio::read_stdin;
use crate::modules::{HostModule, InstallContext};
use crate::types::ModuleName;

/// Installs `airsstack.hook`.
#[derive(Debug)]
pub struct Hook {
    name: ModuleName,
}

impl Hook {
    /// Builds the module.
    ///
    /// # Panics
    ///
    /// Never in practice: the name is a literal that satisfies [`ModuleName`]'s rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: ModuleName::new("hook")
                .unwrap_or_else(|_| unreachable!("`hook` is a valid module name")),
        }
    }
}

impl Default for Hook {
    fn default() -> Self {
        Self::new()
    }
}

impl HostModule for Hook {
    fn name(&self) -> &ModuleName {
        &self.name
    }

    fn install(
        &self,
        lua: &mlua::Lua,
        table: &mlua::Table,
        _context: &InstallContext<'_>,
    ) -> Result<()> {
        let fail = |e: mlua::Error| Error::ModuleInstall {
            module: String::from("hook"),
            reason: e.to_string(),
        };

        // Reads and decodes in one call, because every hook's first two lines were the same pair
        // and the failure mode of getting them wrong is a hook that silently does nothing.
        let payload = lua
            .create_function(|lua, ()| {
                let text = read_stdin()?;
                if text.trim().is_empty() {
                    // No payload is not a parse failure. A hook invoked by hand, or one whose event
                    // carries nothing, should see an empty table rather than an error it has to
                    // guard every call site against.
                    return Ok(mlua::Value::Table(lua.create_table()?));
                }
                convert::from_json(lua, &text).map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("payload", payload).map_err(fail)?;

        let emit = lua
            .create_function(|_, value: mlua::Value| {
                let text = convert::to_json(&value)?;
                write_stdout(&text).map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("emit", emit).map_err(fail)?;

        // The nesting exists once here so that no script has to remember it. `hookEventName` is
        // required by the contract and has no sensible default, so it is an argument rather than
        // something this module guesses.
        let context = lua
            .create_function(
                |lua, (event, additional): (mlua::LuaString, mlua::LuaString)| {
                    let text = context_envelope(lua, event, additional)?;
                    write_stdout(&text).map_err(mlua::Error::from)
                },
            )
            .map_err(fail)?;
        table.set("context", context).map_err(fail)?;

        Ok(())
    }
}

/// Builds the envelope `context` writes to stdout, as JSON text.
///
/// Returned as a value, not written directly, so callers and tests can inspect the envelope
/// itself rather than only its side effect on the process's stdout.
fn context_envelope(
    lua: &mlua::Lua,
    event: mlua::LuaString,
    additional: mlua::LuaString,
) -> mlua::Result<String> {
    let specific = lua.create_table()?;
    specific.set("hookEventName", event)?;
    specific.set("additionalContext", additional)?;
    let envelope = lua.create_table()?;
    envelope.set("hookSpecificOutput", specific)?;

    convert::to_json(&mlua::Value::Table(envelope)).map_err(mlua::Error::from)
}

/// Writes `text` to standard output, flushing it.
fn write_stdout(text: &str) -> Result<()> {
    use std::io::Write as _;

    let mut out = std::io::stdout().lock();
    out.write_all(text.as_bytes()).map_err(|source| Error::Io {
        operation: "emit",
        path: String::from("<stdout>"),
        source,
    })?;
    out.flush().map_err(|source| Error::Io {
        operation: "emit",
        path: String::from("<stdout>"),
        source,
    })
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Hook;
    use crate::{Engine, HostModule as _, Policy, Script};

    fn eval(source: &str) -> String {
        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();
        engine
            .eval_to::<String>(&Script::from_source(source, "test").unwrap())
            .unwrap()
    }

    #[test]
    fn the_module_is_named_hook() {
        assert_eq!(Hook::new().name().as_str(), "hook");
    }

    #[test]
    fn the_three_functions_are_installed() {
        for name in ["payload", "emit", "context"] {
            assert_eq!(
                eval(&format!("return type(airsstack.hook.{name})")),
                "function",
                "{name}"
            );
        }
    }

    #[test]
    fn the_module_is_available_under_a_confined_policy() {
        // Hooks are the reason `confined` is the CLI default, so the contract has to be reachable
        // from it without a grant.
        assert_eq!(eval("return type(airsstack.hook)"), "table");
    }

    /// The envelope `context` writes, via [`super::context_envelope`] — the function `context`'s
    /// closure calls, not a hand-built stand-in — matches the contract the plugin scripts use.
    ///
    /// `context` writes to the process's stdout, which a unit test cannot capture without taking
    /// the whole harness's output with it. Calling the extracted builder directly gets the real
    /// bytes without that side effect.
    #[test]
    fn the_emitted_envelope_matches_the_contract_the_plugin_scripts_use() {
        let lua = mlua::Lua::new();
        let event = lua.create_string("PreToolUse").unwrap();
        let additional = lua.create_string("note").unwrap();
        let json = super::context_envelope(&lua, event, additional).unwrap();
        assert_eq!(
            json,
            r#"{"hookSpecificOutput":{"additionalContext":"note","hookEventName":"PreToolUse"}}"#
        );
    }

    /// `context` must never add a `permissionDecision` field: the CLI honours that field on a path
    /// that can swallow the tool call the hook fired on (see the module doc for the mechanism), so
    /// a hand-rolled decision field must not silently reappear in the envelope this module builds.
    ///
    /// Goes through [`super::context_envelope`] — the real function `context`'s closure calls —
    /// rather than a hand-built literal, so a decision field added to that function's
    /// implementation turns this test red instead of leaving it green against its own fixture.
    #[test]
    fn the_context_envelope_carries_no_permission_decision_field() {
        let lua = mlua::Lua::new();
        let event = lua.create_string("PreToolUse").unwrap();
        let additional = lua.create_string("note").unwrap();
        let json = super::context_envelope(&lua, event, additional).unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&json).unwrap();
        let specific = decoded["hookSpecificOutput"].as_object().unwrap();
        assert!(
            !specific.contains_key("permissionDecision"),
            "envelope must not carry a permissionDecision field: {specific:?}"
        );
    }

    #[test]
    fn emit_accepts_a_table_and_does_not_raise() {
        assert_eq!(
            eval("airsstack.hook.emit({ok = true}); return 'done'"),
            "done"
        );
    }

    #[test]
    fn context_accepts_the_event_name_and_the_text() {
        assert_eq!(
            eval("airsstack.hook.context('SessionStart', 'hello'); return 'done'"),
            "done"
        );
    }
}
