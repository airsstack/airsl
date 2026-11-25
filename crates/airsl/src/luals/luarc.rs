//! The luarc plan: decides from the bytes of a project's `.luarc.json`, or their absence, whether
//! to leave it alone, rewrite it with the type directory listed, or refuse.
//!
//! Its own module because the decision is a pure function of the file's content and has many small
//! cases: a stray comment or a doubled spelling of one setting changes the answer. Keeping it free
//! of the filesystem lets every case be tested with a byte string and lets the caller own the
//! reading, the writing and the wording of a refusal.
//!
//! Responsibilities:
//!
//! - [`plan`], which parses the file as plain JSON and returns a [`LuarcPlan`] or the reason the
//!   file cannot be edited.
//! - Recognising the six key paths under which the language server reads `workspace.library`.
//! - Refusing a file where a rewrite could change which spelling of one setting the server uses.
//! - Rendering every rewrite as sorted-key, two-space-indented JSON with a final newline.
//!
//! Non-responsibilities: reading or writing the file, and the final wording around a refusal. The
//! caller adds the path and the advice to add the entry by hand.

#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

/// What to do with the luarc file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LuarcPlan {
    /// The file already lists the entry; leave its bytes untouched.
    Unchanged,
    /// Replace the file with exactly these bytes.
    Write(Vec<u8>),
}

/// Why a luarc file cannot be edited, worded as a clause that completes a sentence beginning with
/// the file's path; the caller adds the path and the advice to edit the file by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Refusal(String);

impl Refusal {
    fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }

    /// The reason, as a clause such as `it is not plain JSON (...)`, handed over without a copy.
    pub(crate) fn into_reason(self) -> String {
        self.0
    }
}

/// The string the language server reads from `workspace.library` to find the type directory.
const ENTRY: &str = ".airsl/types";

/// A UTF-8 byte order mark, which the language server accepts and a JSON parser does not.
const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Every key path under which the language server reads `workspace.library`, in the order they are
/// checked. A key applies when it, or `Lua.` plus it, names a setting; only otherwise does the
/// server descend into an object value.
const SPELLINGS: [&[&str]; 6] = [
    &["workspace.library"],
    &["workspace", "library"],
    &["Lua.workspace.library"],
    &["Lua.workspace", "library"],
    &["Lua", "workspace", "library"],
    &["Lua", "workspace.library"],
];

/// Where one spelling leads in the parsed file.
enum Reach<'a> {
    /// Some key on the way is missing.
    Absent,
    /// A key on the way exists but is not an object; holds the keys up to and including it.
    Blocked(&'a [&'a str]),
    /// The path ends at this value.
    Found(&'a Value),
}

/// Decides what to do with a luarc whose current bytes are `existing`, `None` when there is no
/// file.
///
/// A leading byte order mark is parsed around and written back, an empty file or one holding only
/// JSON whitespace counts as `{}`, and a rewrite is plain JSON with sorted keys, so comments and
/// key order are not kept.
///
/// # Errors
///
/// Returns a [`Refusal`] holding the reason the file cannot be edited safely, as a sentence
/// fragment the caller embeds in its own message: the file is not plain JSON, its top level is not
/// an object, a key on the way to `workspace.library` has the wrong type, or one setting is
/// spelled two ways and a rewrite could change which spelling the language server uses.
///
/// When several things are wrong the first in this order is reported: parse failures and a
/// non-object top level, then a present spelling whose value is not an array, then a parent that is
/// not an object, then a setting spelled two ways. A rewrite is never refused for the last reason
/// when the entry is already listed in every spelling present.
pub(crate) fn plan(existing: Option<&[u8]>) -> Result<LuarcPlan, Refusal> {
    let (has_bom, mut root) = match existing {
        None => (false, Map::new()),
        Some(bytes) => {
            let (has_bom, body) = bytes
                .strip_prefix(&BOM)
                .map_or((false, bytes), |rest| (true, rest));
            (has_bom, parse_object(body)?)
        }
    };

    let mut found = Vec::new();
    let mut blocked = Vec::new();
    for keys in SPELLINGS {
        match reach(&root, keys) {
            Reach::Absent => {}
            Reach::Blocked(prefix) => blocked.push(prefix),
            Reach::Found(value) => found.push((keys, value)),
        }
    }
    for (keys, value) in &found {
        if !value.is_array() {
            return Err(Refusal::new(format!("{} is not an array", spelled(keys))));
        }
    }
    if let Some(prefix) = blocked.first() {
        return Err(Refusal::new(format!(
            "{} is not an object",
            spelled(prefix)
        )));
    }

    if !found.is_empty() && found.iter().all(|(_, value)| lists_entry(value)) {
        return Ok(LuarcPlan::Unchanged);
    }

    if let Some(path) = shared_setting_path(&root) {
        return Err(Refusal::new(format!("it sets `{path}` in two spellings")));
    }

    let target = found.first().map(|(keys, _)| *keys);
    match target.and_then(|keys| array_mut(&mut root, keys)) {
        Some(list) => list.push(Value::String(ENTRY.to_owned())),
        None => add_entry(&mut root),
    }

    let mut text = serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|error| Refusal::new(format!("it could not be written as JSON ({error})")))?;
    text.push('\n');
    let mut bytes = Vec::with_capacity(BOM.len() + text.len());
    if has_bom {
        bytes.extend_from_slice(&BOM);
    }
    bytes.extend_from_slice(text.as_bytes());
    Ok(LuarcPlan::Write(bytes))
}

/// Parses the file body as a JSON object; a blank body is `{}`.
fn parse_object(body: &[u8]) -> Result<Map<String, Value>, Refusal> {
    if body.iter().all(|byte| is_json_whitespace(*byte)) {
        return Ok(Map::new());
    }
    match serde_json::from_slice::<Value>(body) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(Refusal::new("its top level is not a JSON object")),
        Err(error) => Err(Refusal::new(format!("it is not plain JSON ({error})"))),
    }
}

/// Whether `byte` is whitespace as JSON defines it: space, tab, line feed or carriage return. Form
/// feed is not, so a file holding only one is refused by the parser rather than read as empty.
const fn is_json_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

/// Follows `keys` from the top level of the file.
fn reach<'a>(root: &'a Map<String, Value>, keys: &'a [&'a str]) -> Reach<'a> {
    let mut object = root;
    for (index, key) in keys.iter().enumerate() {
        let Some(value) = object.get(*key) else {
            return Reach::Absent;
        };
        if index + 1 == keys.len() {
            return Reach::Found(value);
        }
        match value.as_object() {
            Some(inner) => object = inner,
            None => return Reach::Blocked(&keys[..=index]),
        }
    }
    Reach::Absent
}

/// Follows `keys` to an array that can be edited in place.
fn array_mut<'a>(root: &'a mut Map<String, Value>, keys: &[&str]) -> Option<&'a mut Vec<Value>> {
    let (first, rest) = keys.split_first()?;
    let mut value = root.get_mut(*first)?;
    for key in rest {
        value = value.as_object_mut()?.get_mut(*key)?;
    }
    value.as_array_mut()
}

/// Adds the entry where no spelling of the setting exists yet: inside a top-level `workspace`
/// object when there is one, as a dotted key otherwise.
fn add_entry(root: &mut Map<String, Value>) {
    let list = Value::Array(vec![Value::String(ENTRY.to_owned())]);
    match root.get_mut("workspace").and_then(Value::as_object_mut) {
        Some(workspace) => {
            workspace.insert(String::from("library"), list);
        }
        None => {
            root.insert(String::from("workspace.library"), list);
        }
    }
}

/// Whether the array holds exactly the entry string.
fn lists_entry(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(ENTRY)))
}

/// A key path as the user wrote it: each key in backticks, joined with an arrow.
fn spelled(keys: &[&str]) -> String {
    keys.iter()
        .map(|key| format!("`{key}`"))
        .collect::<Vec<_>>()
        .join(" \u{2192} ")
}

/// The shortest setting path of two or more segments that is reached through two different
/// spellings, ties broken by byte order of the joined path.
///
/// A rewrite sorts keys, which can flip which of two spellings of one setting is textually last,
/// and the language server uses the last. Two spellings can sit under different top-level keys or
/// inside one, such as `Lua` holding both `workspace` and `workspace.library`.
fn shared_setting_path(root: &Map<String, Value>) -> Option<String> {
    let mut reached = BTreeMap::new();
    for (key, value) in root {
        Trail::start(key).walk(value, &mut reached);
    }
    reached
        .into_iter()
        .filter(|(_, spellings)| spellings.len() >= 2)
        .map(|(path, _)| (path.len(), path.join(".")))
        .min()
        .map(|(_, joined)| joined)
}

/// Every setting path reached, with each distinct spelling that reached it. A spelling is the JSON
/// keys taken until their segments cover the path.
type Reached = BTreeMap<Vec<String>, BTreeSet<Vec<String>>>;

/// The JSON keys one walk has taken so far and the dot-separated segments they spell.
#[derive(Clone)]
struct Trail {
    /// The keys taken, outermost first.
    keys: Vec<String>,
    /// Every key's segments joined in order.
    segments: Vec<String>,
    /// For each key, how many segments there are once it is taken.
    ends: Vec<usize>,
}

impl Trail {
    fn start(key: &str) -> Self {
        Self {
            keys: Vec::new(),
            segments: Vec::new(),
            ends: Vec::new(),
        }
        .then(key)
    }

    /// The trail after also taking `key`, whose dots split it into further segments.
    fn then(mut self, key: &str) -> Self {
        self.keys.push(key.to_owned());
        self.segments.extend(key.split('.').map(str::to_owned));
        self.ends.push(self.segments.len());
        self
    }

    /// Records every path of two or more segments this trail reaches, then continues into `value`
    /// when it is an object. A leading `Lua` segment is not part of any path, but the key that
    /// carries it stays in the spelling.
    fn walk(&self, value: &Value, reached: &mut Reached) {
        let skip = usize::from(self.segments.first().is_some_and(|first| first == "Lua"));
        for end in skip + 2..=self.segments.len() {
            let last_key = self.ends.partition_point(|covered| *covered < end);
            reached
                .entry(self.segments[skip..end].to_vec())
                .or_default()
                .insert(self.keys[..=last_key].to_vec());
        }
        if let Some(object) = value.as_object() {
            for (key, inner) in object {
                self.clone().then(key).walk(inner, reached);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{LuarcPlan, Refusal, plan};
    use serde_json::{Value, json};

    const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];
    const MINIMAL: &str = "{\n  \"workspace.library\": [\n    \".airsl/types\"\n  ]\n}\n";

    fn planned(text: &str) -> Result<LuarcPlan, Refusal> {
        plan(Some(text.as_bytes()))
    }

    fn refusal(text: &str) -> String {
        planned(text).unwrap_err().into_reason()
    }

    /// The bytes of a `Write` plan; fails the test when the plan is `Unchanged`.
    fn written(text: &str) -> Vec<u8> {
        let plan = planned(text).unwrap();
        assert_ne!(plan, LuarcPlan::Unchanged, "expected a rewrite of {text}");
        match plan {
            LuarcPlan::Write(bytes) => bytes,
            LuarcPlan::Unchanged => Vec::new(),
        }
    }

    fn written_value(text: &str) -> Value {
        serde_json::from_slice(&written(text)).unwrap()
    }

    #[test]
    fn no_file_writes_the_minimal_luarc_bytes() {
        assert_eq!(
            plan(None).unwrap(),
            LuarcPlan::Write(MINIMAL.as_bytes().to_vec())
        );
    }

    #[test]
    fn a_bom_is_parsed_after_and_written_back() {
        let mut input = BOM.to_vec();
        input.extend_from_slice(b"{}");
        let mut expected = BOM.to_vec();
        expected.extend_from_slice(MINIMAL.as_bytes());
        assert_eq!(plan(Some(&input)).unwrap(), LuarcPlan::Write(expected));

        let mut listed = BOM.to_vec();
        listed.extend_from_slice(br#"{"workspace.library": [".airsl/types"]}"#);
        assert_eq!(plan(Some(&listed)).unwrap(), LuarcPlan::Unchanged);
    }

    #[test]
    fn an_empty_or_whitespace_file_is_treated_as_an_empty_object() {
        let expected = LuarcPlan::Write(MINIMAL.as_bytes().to_vec());
        assert_eq!(planned("").unwrap(), expected);
        assert_eq!(planned(" \n\t").unwrap(), expected);
    }

    #[test]
    fn a_file_that_is_not_plain_json_is_refused_with_the_serde_error() {
        assert_eq!(
            refusal("{\n  // keep me\n  \"workspace.library\": []\n}"),
            "it is not plain JSON (key must be a string at line 2 column 3)"
        );
        assert_eq!(
            refusal(r#"{"a": 1,}"#),
            "it is not plain JSON (trailing comma at line 1 column 9)"
        );
        assert_eq!(
            plan(Some(b"{\"a\":\"\xFF\"}")).unwrap_err().into_reason(),
            "it is not plain JSON (invalid unicode code point at line 1 column 8)"
        );
    }

    #[test]
    fn a_form_feed_only_file_is_not_plain_json() {
        assert_eq!(
            refusal("\x0C"),
            "it is not plain JSON (expected value at line 1 column 1)"
        );
    }

    #[test]
    fn json_whitespace_alone_is_an_empty_object() {
        assert_eq!(
            planned(" \t\r\n").unwrap(),
            LuarcPlan::Write(MINIMAL.as_bytes().to_vec())
        );
    }

    #[test]
    fn a_non_array_spelling_is_reported_before_a_non_object_parent() {
        assert_eq!(
            refusal(r#"{"workspace": 1, "Lua.workspace.library": "x"}"#),
            "`Lua.workspace.library` is not an array"
        );
    }

    #[test]
    fn a_non_object_top_level_is_refused() {
        assert_eq!(refusal("[]"), "its top level is not a JSON object");
    }

    #[test]
    fn a_dotted_library_that_is_not_an_array_is_refused() {
        assert_eq!(
            refusal(r#"{"workspace.library": "x"}"#),
            "`workspace.library` is not an array"
        );
    }

    #[test]
    fn a_workspace_that_is_not_an_object_or_a_nested_library_that_is_not_an_array_is_refused() {
        assert_eq!(
            refusal(r#"{"workspace": 1}"#),
            "`workspace` is not an object"
        );
        assert_eq!(refusal(r#"{"Lua": 1}"#), "`Lua` is not an object");
        assert_eq!(
            refusal(r#"{"workspace": {"library": "x"}}"#),
            "`workspace` \u{2192} `library` is not an array"
        );
        assert_eq!(
            refusal(r#"{"Lua": {"workspace": {"library": 2}}}"#),
            "`Lua` \u{2192} `workspace` \u{2192} `library` is not an array"
        );
    }

    #[test]
    fn a_listed_entry_is_unchanged_and_bytes_untouched() {
        for text in [
            r#"{"workspace.library": [".airsl/types"]}"#,
            r#"{"workspace": {"library": [".airsl/types"]}}"#,
            r#"{"runtime.version": "Lua 5.4", "workspace.library": [".airsl/types"]}"#,
        ] {
            assert_eq!(planned(text).unwrap(), LuarcPlan::Unchanged, "{text}");
        }
    }

    #[test]
    fn the_same_path_in_two_spellings_is_refused_when_a_rewrite_is_needed() {
        assert_eq!(
            refusal(r#"{"workspace": {"library": []}, "workspace.library": []}"#),
            "it sets `workspace.library` in two spellings"
        );
    }

    #[test]
    fn a_listed_entry_in_one_of_two_spellings_is_refused() {
        assert_eq!(
            refusal(r#"{"workspace": {"library": [".airsl/types"]}, "workspace.library": []}"#),
            "it sets `workspace.library` in two spellings"
        );
    }

    #[test]
    fn a_map_valued_setting_in_two_spellings_is_refused() {
        assert_eq!(
            refusal(
                r#"{"diagnostics.severity": {"a": "Error"}, "diagnostics": {"severity": {"b": "Hint"}}}"#
            ),
            "it sets `diagnostics.severity` in two spellings"
        );
    }

    #[test]
    fn two_spellings_under_one_lua_object_are_refused() {
        assert_eq!(
            refusal(r#"{"Lua": {"workspace": {"library": []}, "workspace.library": []}}"#),
            "it sets `workspace.library` in two spellings"
        );
        assert_eq!(
            refusal(
                r#"{"Lua": {"workspace": {"library": [".airsl/types"]}, "workspace.library": []}}"#
            ),
            "it sets `workspace.library` in two spellings"
        );
    }

    #[test]
    fn a_map_valued_setting_in_two_spellings_under_lua_is_refused() {
        assert_eq!(
            refusal(
                r#"{"Lua": {"diagnostics.severity": {"a": "Error"}, "diagnostics": {"severity": {"b": "Hint"}}}}"#
            ),
            "it sets `diagnostics.severity` in two spellings"
        );
    }

    #[test]
    fn a_maps_own_keys_are_not_two_spellings() {
        assert_eq!(
            written_value(r#"{"diagnostics": {"severity": {"a": "Error", "b": "Hint"}}}"#),
            json!({
                "diagnostics": {"severity": {"a": "Error", "b": "Hint"}},
                "workspace.library": [".airsl/types"]
            })
        );
    }

    #[test]
    fn several_non_array_spellings_name_the_first_in_list_order() {
        assert_eq!(
            refusal(r#"{"Lua.workspace.library": 1, "workspace.library": "x"}"#),
            "`workspace.library` is not an array"
        );
    }

    #[test]
    fn several_blocked_parents_name_the_first_in_list_order() {
        assert_eq!(
            refusal(r#"{"workspace": 1, "Lua": 1}"#),
            "`workspace` is not an object"
        );
    }

    #[test]
    fn a_lua_prefixed_spelling_is_appended_to() {
        assert_eq!(
            written_value(r#"{"Lua.workspace.library": ["lib"]}"#),
            json!({"Lua.workspace.library": ["lib", ".airsl/types"]})
        );
        assert_eq!(
            written_value(r#"{"Lua": {"workspace": {"library": []}}}"#),
            json!({"Lua": {"workspace": {"library": [".airsl/types"]}}})
        );
        assert_eq!(
            written_value(r#"{"Lua.workspace": {"library": ["lib"]}}"#),
            json!({"Lua.workspace": {"library": ["lib", ".airsl/types"]}})
        );
    }

    #[test]
    fn a_lua_object_with_a_dotted_library_is_a_spelling() {
        assert_eq!(
            written_value(r#"{"Lua": {"workspace.library": ["lib"]}}"#),
            json!({"Lua": {"workspace.library": ["lib", ".airsl/types"]}})
        );
        assert_eq!(
            planned(r#"{"Lua": {"workspace.library": [".airsl/types"]}}"#).unwrap(),
            LuarcPlan::Unchanged
        );
    }

    #[test]
    fn a_lua_object_with_a_dotted_library_that_is_not_an_array_is_refused() {
        assert_eq!(
            refusal(r#"{"Lua": {"workspace.library": "x"}}"#),
            "`Lua` \u{2192} `workspace.library` is not an array"
        );
    }

    #[test]
    fn a_lua_prefixed_and_a_plain_spelling_conflict() {
        assert_eq!(
            refusal(r#"{"Lua.workspace.library": [], "workspace.library": []}"#),
            "it sets `workspace.library` in two spellings"
        );
    }

    #[test]
    fn the_shortest_shared_path_tie_breaks_by_byte_order() {
        assert_eq!(
            refusal(
                r#"{"a-": {"b": {"c": 1}}, "a-.b": {"c": 2}, "a": {"b": {"z": 1}}, "a.b": {"z": 2}}"#
            ),
            "it sets `a-.b` in two spellings"
        );
    }

    #[test]
    fn a_lua_prefixed_listing_is_unchanged() {
        assert_eq!(
            planned(r#"{"Lua.workspace.library": [".airsl/types"]}"#).unwrap(),
            LuarcPlan::Unchanged
        );
    }

    #[test]
    fn the_entry_is_appended_or_added_in_the_spelling_present() {
        assert_eq!(
            written_value(r#"{"workspace.library": ["x"]}"#),
            json!({"workspace.library": ["x", ".airsl/types"]})
        );
        assert_eq!(
            written_value(r#"{"workspace": {"library": ["x"]}}"#),
            json!({"workspace": {"library": ["x", ".airsl/types"]}})
        );
        assert_eq!(
            written_value(r#"{"workspace": {"checkThirdParty": false}}"#),
            json!({"workspace": {"checkThirdParty": false, "library": [".airsl/types"]}})
        );
        assert_eq!(
            written_value(r#"{"a": 1}"#),
            json!({"a": 1, "workspace.library": [".airsl/types"]})
        );
    }

    #[test]
    fn a_rewrite_is_pretty_printed_with_sorted_keys_and_a_trailing_newline() {
        assert_eq!(
            String::from_utf8(written(r#"{"z":1,"a":{"y":1,"b":2}}"#)).unwrap(),
            "{\n  \"a\": {\n    \"b\": 2,\n    \"y\": 1\n  },\n  \"workspace.library\": [\n    \".airsl/types\"\n  ],\n  \"z\": 1\n}\n"
        );
    }

    #[test]
    fn duplicate_keys_keep_the_last_value() {
        assert_eq!(
            planned(r#"{"workspace.library": [], "workspace.library": [".airsl/types"]}"#).unwrap(),
            LuarcPlan::Unchanged
        );
    }

    #[test]
    fn parse_refusals_win_over_shape_refusals() {
        assert_eq!(
            refusal("[1, 2,]"),
            "it is not plain JSON (trailing comma at line 1 column 7)"
        );
    }

    #[test]
    fn a_listed_entry_in_both_spellings_is_unchanged() {
        assert_eq!(
            planned(
                r#"{"workspace.library": [".airsl/types"], "workspace": {"library": [".airsl/types"]}}"#
            )
            .unwrap(),
            LuarcPlan::Unchanged
        );
    }

    #[test]
    fn disjoint_spellings_are_rewritten() {
        assert_eq!(
            written_value(r#"{"workspace.library": [], "workspace": {"checkThirdParty": false}}"#),
            json!({
                "workspace.library": [".airsl/types"],
                "workspace": {"checkThirdParty": false}
            })
        );
    }

    #[test]
    fn key_paths_split_nested_dotted_keys() {
        assert_eq!(
            refusal(r#"{"workspace": {"a.b": 1}, "workspace.a": {"b": 2}}"#),
            "it sets `workspace.a` in two spellings"
        );
    }

    #[test]
    fn a_dot_slash_entry_is_not_listed() {
        assert_eq!(
            written_value(r#"{"workspace.library": ["./.airsl/types"]}"#),
            json!({"workspace.library": ["./.airsl/types", ".airsl/types"]})
        );
    }

    #[test]
    fn non_string_library_elements_are_kept() {
        assert_eq!(
            written_value(r#"{"workspace.library": [1, null, ".x"]}"#),
            json!({"workspace.library": [1, null, ".x", ".airsl/types"]})
        );
    }
}
