//! The `airsstack.stdio` host module.
//!
//! Below `Full` there is no `io` table at all, and every plugin hook receives its payload on
//! stdin — so without this the hooks cannot be ported to any policy worth running them under.
//!
//! Needs no grant. The three standard streams are the ones the host handed this process when it
//! started it, so reading and writing them is not authority over anything the host did not
//! already choose to give. That is a narrower claim than "I/O needs no grant", and it is why
//! opening a *file* is `fs`'s business rather than this module's.
//!
//! Responsibilities: [`Stdio`], installing `read`, `lines`, `write`, `error` and `isatty`.
//!
//! Non-responsibilities: interpreting what arrives. `hook` does that.

use std::io::{IsTerminal as _, Read as _, Write as _};

use crate::error::{Error, Result};
use crate::modules::{HostModule, InstallContext};
use crate::types::ModuleName;

/// Installs `airsstack.stdio`.
#[derive(Debug)]
pub struct Stdio {
    name: ModuleName,
}

impl Stdio {
    /// Builds the module.
    ///
    /// # Panics
    ///
    /// Never in practice: the name is a literal that satisfies [`ModuleName`]'s rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: ModuleName::new("stdio")
                .unwrap_or_else(|_| unreachable!("`stdio` is a valid module name")),
        }
    }
}

impl Default for Stdio {
    fn default() -> Self {
        Self::new()
    }
}

/// Reads standard input to end of stream.
pub(crate) fn read_stdin() -> Result<String> {
    let mut buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|source| Error::Io {
            operation: "read",
            path: String::from("<stdin>"),
            source,
        })?;
    Ok(buffer)
}

impl HostModule for Stdio {
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
            module: String::from("stdio"),
            reason: e.to_string(),
        };

        let read = lua
            .create_function(|_, ()| read_stdin().map_err(mlua::Error::from))
            .map_err(fail)?;
        table.set("read", read).map_err(fail)?;

        let lines = lua
            .create_function(|lua, ()| {
                let text = read_stdin()?;
                let body = text.strip_suffix('\n').unwrap_or(&text);
                let lines: Vec<&str> = if body.is_empty() {
                    Vec::new()
                } else {
                    body.split('\n')
                        .map(|line| line.strip_suffix('\r').unwrap_or(line))
                        .collect()
                };
                lua.create_sequence_from(lines)
            })
            .map_err(fail)?;
        table.set("lines", lines).map_err(fail)?;

        // No trailing newline is added. A hook's output is a single JSON document read by a parser
        // that cares what the bytes are, so this module writes exactly what it is given.
        let write = lua
            .create_function(|_, body: mlua::LuaString| {
                let mut out = std::io::stdout().lock();
                out.write_all(&body.as_bytes())
                    .map_err(|source| Error::Io {
                        operation: "write",
                        path: String::from("<stdout>"),
                        source,
                    })?;
                out.flush().map_err(|source| Error::Io {
                    operation: "write",
                    path: String::from("<stdout>"),
                    source,
                })?;
                Ok(())
            })
            .map_err(fail)?;
        table.set("write", write).map_err(fail)?;

        let error = lua
            .create_function(|_, body: mlua::LuaString| {
                let mut out = std::io::stderr().lock();
                out.write_all(&body.as_bytes())
                    .map_err(|source| Error::Io {
                        operation: "write",
                        path: String::from("<stderr>"),
                        source,
                    })?;
                out.flush().map_err(|source| Error::Io {
                    operation: "write",
                    path: String::from("<stderr>"),
                    source,
                })?;
                Ok(())
            })
            .map_err(fail)?;
        table.set("error", error).map_err(fail)?;

        // Lets a script tell "a person is watching" from "my output is being parsed", which is the
        // difference between printing a progress line and corrupting a JSON document.
        let isatty = lua
            .create_function(|_, stream: Option<mlua::LuaString>| {
                let name = match stream.as_ref() {
                    Some(text) => text.to_str()?.to_owned(),
                    None => String::from("stdout"),
                };
                Ok(match name.as_str() {
                    "stdin" => std::io::stdin().is_terminal(),
                    "stderr" => std::io::stderr().is_terminal(),
                    _ => std::io::stdout().is_terminal(),
                })
            })
            .map_err(fail)?;
        table.set("isatty", isatty).map_err(fail)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Stdio;
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
    fn the_module_is_named_stdio() {
        assert_eq!(Stdio::new().name().as_str(), "stdio");
    }

    #[test]
    fn every_function_is_installed_under_a_confined_policy() {
        // The point of the module: `Restricted` has no `io` table at all, so a hook that reads its
        // payload has nothing else to reach for.
        for name in ["read", "lines", "write", "error", "isatty"] {
            assert_eq!(
                eval(&format!("return type(airsstack.stdio.{name})")),
                "function",
                "{name}"
            );
        }
    }

    #[test]
    fn stdio_is_present_even_under_a_pure_policy() {
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        let found = engine
            .eval_to::<String>(
                &Script::from_source("return type(airsstack.stdio.write)", "p").unwrap(),
            )
            .unwrap();
        assert_eq!(found, "function");
    }

    #[test]
    fn isatty_answers_for_each_named_stream() {
        // Under `cargo test` none of the three is a terminal, so the useful assertion is that the
        // call answers with a boolean for every name rather than raising on an unknown one.
        for stream in ["stdin", "stdout", "stderr", "something-else"] {
            assert_eq!(
                eval(&format!("return type(airsstack.stdio.isatty('{stream}'))")),
                "boolean",
                "{stream}"
            );
        }
    }

    #[test]
    fn isatty_defaults_to_stdout() {
        assert_eq!(eval("return type(airsstack.stdio.isatty())"), "boolean");
    }

    #[test]
    fn writing_does_not_raise() {
        assert_eq!(
            eval("airsstack.stdio.write(''); airsstack.stdio.error(''); return 'ok'"),
            "ok"
        );
    }
}
