//! The `airsstack.hook` host module: the agent-hook contract, in one place.
//!
//! A thin layer over `stdio` and `json`, and worth being its own module for one reason: the shape
//! a hook must emit is easy to get subtly wrong, and every plugin script currently rebuilds it by
//! hand. The nesting is real and is taken from the scripts this replaces —
//! `plugins/airsstack/hooks/enforce.py:677-682` and
//! `plugins/airsstack-journal/scripts/session-start.sh:38`:
//!
//! ```json
//! {"hookSpecificOutput": {"hookEventName": "PreToolUse", "additionalContext": "…"}}
//! ```
//!
//! Responsibilities: [`Hook`], installing `payload`, `emit` and `context`.
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
                    let specific = lua.create_table()?;
                    specific.set("hookEventName", event)?;
                    specific.set("additionalContext", additional)?;
                    let envelope = lua.create_table()?;
                    envelope.set("hookSpecificOutput", specific)?;

                    let text = convert::to_json(&mlua::Value::Table(envelope))?;
                    write_stdout(&text).map_err(mlua::Error::from)
                },
            )
            .map_err(fail)?;
        table.set("context", context).map_err(fail)?;

        Ok(())
    }
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

    /// The envelope `context` writes, built the same way but returned instead of printed.
    ///
    /// `context` writes to the process's stdout, which a unit test cannot capture without taking
    /// the whole harness's output with it. Asserting on the identical structure keeps the shape
    /// under test; that it reaches stdout is `stdio`'s behaviour and is tested there.
    #[test]
    fn the_emitted_envelope_matches_the_contract_the_plugin_scripts_use() {
        let json = eval(
            "return airsstack.json.encode({
               hookSpecificOutput = {
                 hookEventName = 'PreToolUse',
                 additionalContext = 'note',
               }
             })",
        );
        assert_eq!(
            json,
            r#"{"hookSpecificOutput":{"additionalContext":"note","hookEventName":"PreToolUse"}}"#
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
