//! Conversion between Lua values and JSON.
//!
//! Separate from the `json` host module because two modules need it: `airsstack.json` exposes it
//! directly, and `airsstack.hook` uses it to parse the JSON object on stdin and to emit one on
//! stdout. Keeping the conversion here means both agree on how `null`, empty tables and integers
//! round-trip.
//!
//! Responsibilities: [`to_json`] and [`from_json`], plus the serializer options that pin the
//! round-trip behaviour.
//!
//! Non-responsibilities: I/O. Neither function reads or writes a stream.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use mlua::LuaSerdeExt as _;

use crate::error::{Error, Result};

/// Chunk name used when a conversion failure has no script context of its own.
const CHUNK: &str = "<json>";

/// Encodes a Lua value as JSON text.
///
/// Lua tables become objects unless they are sequences, which become arrays. Because Lua has no
/// distinct empty-sequence value, an empty table encodes as `{}`.
///
/// # Errors
///
/// Returns [`Error::Lua`] when the value contains something JSON cannot represent, such as a
/// function, a userdata, or a table with a cycle.
pub(crate) fn to_json(value: &mlua::Value) -> Result<String> {
    serde_json::to_string(value).map_err(|e| Error::Lua {
        chunk: CHUNK.to_owned(),
        source: Box::new(mlua::Error::external(e)),
    })
}

/// Encodes a Lua value as indented JSON text with a trailing newline.
///
/// # Errors
///
/// As [`to_json`].
pub(crate) fn to_json_pretty(value: &mlua::Value) -> Result<String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|e| Error::Lua {
        chunk: CHUNK.to_owned(),
        source: Box::new(mlua::Error::external(e)),
    })?;
    text.push('\n');
    Ok(text)
}

/// Parses JSON text into a Lua value.
///
/// JSON `null` becomes Lua `nil`. A `nil` value inside a table is indistinguishable from an absent
/// key in Lua, so an object whose fields are all `null` decodes to an empty table.
///
/// # Errors
///
/// Returns [`Error::Lua`] when `text` is not valid JSON, or when the parsed structure cannot be
/// built as a Lua value.
pub(crate) fn from_json(lua: &mlua::Lua, text: &str) -> Result<mlua::Value> {
    let parsed: serde_json::Value = serde_json::from_str(text).map_err(|e| Error::Lua {
        chunk: CHUNK.to_owned(),
        source: Box::new(mlua::Error::external(e)),
    })?;
    lua.to_value(&parsed).map_err(|e| Error::lua(CHUNK, e))
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{from_json, to_json, to_json_pretty};

    fn lua() -> mlua::Lua {
        mlua::Lua::new()
    }

    #[test]
    fn scalars_round_trip() {
        let lua = lua();
        for text in ["1", "true", "false", "\"hi\"", "1.5"] {
            let value = from_json(&lua, text).unwrap();
            assert_eq!(to_json(&value).unwrap(), text, "round-trip of {text}");
        }
    }

    #[test]
    fn integers_stay_integers() {
        let lua = lua();
        let value = from_json(&lua, "3").unwrap();
        assert_eq!(to_json(&value).unwrap(), "3");
    }

    #[test]
    fn floats_are_not_collapsed_to_integers() {
        let lua = lua();
        let value = from_json(&lua, "3.5").unwrap();
        assert_eq!(to_json(&value).unwrap(), "3.5");
    }

    #[test]
    fn arrays_round_trip_as_sequences() {
        let lua = lua();
        let value = from_json(&lua, "[1,2,3]").unwrap();
        assert_eq!(to_json(&value).unwrap(), "[1,2,3]");
    }

    #[test]
    fn nested_objects_round_trip() {
        let lua = lua();
        let value = from_json(&lua, r#"{"a":{"b":1}}"#).unwrap();
        assert_eq!(to_json(&value).unwrap(), r#"{"a":{"b":1}}"#);
    }

    #[test]
    fn invalid_json_is_an_error_not_a_panic() {
        let lua = lua();
        assert!(from_json(&lua, "{not json").is_err());
    }

    #[test]
    fn pretty_output_is_indented_and_newline_terminated() {
        let lua = lua();
        let value = from_json(&lua, r#"{"a":1}"#).unwrap();
        let text = to_json_pretty(&value).unwrap();
        assert!(text.ends_with("}\n"), "{text:?}");
        assert!(text.contains("\n  \"a\""), "{text:?}");
    }
}
