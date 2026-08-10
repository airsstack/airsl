//! The `airsstack.json` host module.
//!
//! Its own module because JSON is the wire format for everything the plugin scripts exchange —
//! hook payloads on stdin, hook results on stdout, and the derived index files the journal tooling
//! writes. Lua ships no JSON support at all, so without this the scripts would shell out to another
//! interpreter, which is the situation this crate exists to remove.
//!
//! Responsibilities: [`Json`], installing `encode`, `encode_pretty` and `decode`.
//!
//! Non-responsibilities: the conversion rules themselves, which live in the crate's internal
//! `convert` module and are shared with `airsstack.hook`.

use crate::convert;
use crate::error::{Error, Result};
use crate::modules::{HostModule, InstallContext};
use crate::types::ModuleName;

/// Installs `airsstack.json`.
#[derive(Debug)]
pub struct Json {
    name: ModuleName,
}

impl Json {
    /// Builds the module.
    ///
    /// # Panics
    ///
    /// Never in practice: the name is a literal that satisfies [`ModuleName`]'s rules, and the
    /// crate's own test suite covers it.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: ModuleName::new("json")
                .unwrap_or_else(|_| unreachable!("`json` is a valid module name")),
        }
    }
}

impl Default for Json {
    fn default() -> Self {
        Self::new()
    }
}

impl HostModule for Json {
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
            module: String::from("json"),
            reason: e.to_string(),
        };

        let encode = lua
            .create_function(|_, value: mlua::Value| {
                convert::to_json(&value).map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("encode", encode).map_err(fail)?;

        let encode_pretty = lua
            .create_function(|_, value: mlua::Value| {
                convert::to_json_pretty(&value).map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("encode_pretty", encode_pretty).map_err(fail)?;

        let decode = lua
            .create_function(|lua, text: mlua::LuaString| {
                let text = text.to_str()?;
                convert::from_json(lua, &text).map_err(mlua::Error::from)
            })
            .map_err(fail)?;
        table.set("decode", decode).map_err(fail)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Json;
    use crate::{Engine, HostModule as _, Policy, Script};

    fn eval(source: &str) -> String {
        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();
        let script = Script::from_source(source, "test").unwrap();
        engine.eval_to::<String>(&script).unwrap()
    }

    #[test]
    fn the_module_is_named_json() {
        assert_eq!(Json::new().name().as_str(), "json");
    }

    #[test]
    fn encode_produces_compact_json() {
        assert_eq!(eval("return airsstack.json.encode({a = 1})"), r#"{"a":1}"#);
    }

    #[test]
    fn decode_parses_objects_into_tables() {
        assert_eq!(eval(r#"return airsstack.json.decode('{"a":"b"}').a"#), "b");
    }

    #[test]
    fn decode_parses_arrays_into_sequences() {
        assert_eq!(eval(r#"return airsstack.json.decode('["x","y"]')[2]"#), "y");
    }

    #[test]
    fn a_decoded_integer_stays_an_integer() {
        assert_eq!(
            eval(r#"return math.type(airsstack.json.decode('{"n":3}').n)"#),
            "integer"
        );
    }

    #[test]
    fn a_decoded_float_stays_a_float() {
        assert_eq!(
            eval(r#"return math.type(airsstack.json.decode('{"n":3.5}').n)"#),
            "float"
        );
    }

    #[test]
    fn encode_pretty_is_indented() {
        let text = eval("return airsstack.json.encode_pretty({a = 1})");
        assert!(text.contains("\n  \"a\""), "{text:?}");
    }

    #[test]
    fn invalid_json_raises_a_catchable_lua_error() {
        assert_eq!(
            eval("local ok = pcall(airsstack.json.decode, '{oops'); return tostring(ok)"),
            "false"
        );
    }
}
