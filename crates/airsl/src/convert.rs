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

/// Prepares `value` for serialisation with object keys in sorted order.
///
/// Lua table iteration is hash order, which varies between runs, so encoding a table straight into
/// `serde_json` produced a different byte string each time. Anything that writes an index, a
/// lockfile or a cached artifact needs the opposite, and a caller cannot recover insertion order
/// afterwards because Lua never had it. Sorting is therefore the behaviour rather than an option.
fn sorted(value: &mlua::Value) -> mlua::SerializableValue<'_> {
    value.to_serializable().sort_keys(true)
}

/// Encodes a Lua value as JSON text, with object keys in sorted order.
///
/// Lua tables become objects unless they are sequences, which become arrays. Because Lua has no
/// distinct empty-sequence value, an empty table encodes as `{}`.
///
/// # Errors
///
/// Returns [`Error::Lua`] when the value contains something JSON cannot represent, such as a
/// function, a userdata, or a table with a cycle.
pub(crate) fn to_json(value: &mlua::Value) -> Result<String> {
    serde_json::to_string(&sorted(value)).map_err(|e| Error::Lua {
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
    let mut text = serde_json::to_string_pretty(&sorted(value)).map_err(|e| Error::Lua {
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
    fn object_keys_are_emitted_in_sorted_order() {
        let lua = lua();
        let table: mlua::Value = lua
            .load("return {kappa=1,alpha=1,beta=1,gamma=1,zeta=1,omega=1,mid=1,delta=1}")
            .eval()
            .unwrap();
        assert_eq!(
            to_json(&table).unwrap(),
            r#"{"alpha":1,"beta":1,"delta":1,"gamma":1,"kappa":1,"mid":1,"omega":1,"zeta":1}"#
        );
    }

    #[test]
    fn the_same_table_encodes_to_the_same_bytes_every_time() {
        let lua = lua();
        let source = "return {kappa=1,alpha=1,beta=1,gamma=1,zeta=1,omega=1,mid=1,delta=1}";
        let first = to_json(&lua.load(source).eval::<mlua::Value>().unwrap()).unwrap();
        for _ in 0..16 {
            let table: mlua::Value = lua.load(source).eval().unwrap();
            assert_eq!(to_json(&table).unwrap(), first);
        }
    }

    #[test]
    fn pretty_output_is_sorted_too() {
        let lua = lua();
        let table: mlua::Value = lua.load("return {b=1,a=1}").eval().unwrap();
        assert_eq!(
            to_json_pretty(&table).unwrap(),
            "{\n  \"a\": 1,\n  \"b\": 1\n}\n"
        );
    }

    #[test]
    fn a_cyclic_table_is_refused_rather_than_recursing() {
        let lua = lua();
        let table: mlua::Value = lua.load("local t = {} t.self = t return t").eval().unwrap();
        let err = to_json(&table).unwrap_err();
        assert!(err.to_string().contains("recursive"), "{err}");
    }

    #[test]
    fn arrays_keep_their_order_rather_than_being_sorted() {
        let lua = lua();
        let table: mlua::Value = lua.load("return {'c','a','b'}").eval().unwrap();
        assert_eq!(to_json(&table).unwrap(), r#"["c","a","b"]"#);
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
