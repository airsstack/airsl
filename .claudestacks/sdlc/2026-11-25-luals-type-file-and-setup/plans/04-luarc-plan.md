---
status: approved
created: 2026-11-25
depends-on: [03]
built-on: 044432888b01
---

# Luarc Plan Implementation Plan

**Goal:** A pure `luarc::plan` decides, from a luarc's bytes or their absence, whether to leave it, rewrite it, or refuse.

**Architecture:** `luals/luarc.rs` is `pub(crate)`. Hosts reach it only through `setup` (plan 05). It has no filesystem access and works on `Option<&[u8]>` → `Result<LuarcPlan, String>`, where the error string is the refusal reason. `setup` wraps it into `LuarcRefused`. It parses with the workspace's `serde_json` (1.0.151, no `preserve_order`: `probes/cargo-tree-serde-json.txt`). This plan depends on 03 only so that two plans do not edit `luals/mod.rs` at once.

**Tech Stack:** Rust 1.94, crate `airsl`, `serde_json` 1.0.151, cargo-make.

**Content authority:** spec §6.1 (L1–L12, key paths, append rule, refusals), §9 (the `luarc.rs` rows), §10 (`luals/luarc.rs` tests).

---

## Files that change

```
crates/airsl/src/luals/luarc.rs   — create  LuarcPlan, plan(), the L1–L12 rules
crates/airsl/src/luals/mod.rs     — modify  mod luarc; (no public re-export)
```

## Order of work

### Task 1 — Plan the luarc edit from bytes

**Files:**
- Create `crates/airsl/src/luals/luarc.rs`: `pub(crate) enum LuarcPlan { Unchanged, Write(Vec<u8>) }` and `pub(crate) fn plan(existing: Option<&[u8]>) -> Result<LuarcPlan, String>`. The module doc follows the crate's shape.
- Modify `crates/airsl/src/luals/mod.rs`: add `mod luarc;` and nothing else.

**Change:**
- Apply L1–L12 in order. The first rule that decides wins, and refusal reasons are the exact §6.1 texts.
- A "spelling" is any of the five §6.1 key paths that reach `workspace.library` (D22), including the three `Lua` forms. L6 and L7 name the user's key path as `{spelled}`, each JSON key in backticks joined with ` → `.
- The entry is the string `.airsl/types`. "Listed" means an array element equal to exactly that string.
- Key paths for L9 follow §6.1: split each top-level key on `.`, descend into object values (splitting their keys on `.` too), stop at a non-object or an empty object, and drop a leading `Lua` segment. A key reaches every prefix of every path it walks. Two top-level keys conflict on a shared path of two or more segments. The refusal names the shortest shared path, joined with `.`, with ties broken by byte order.
- L10 appends to the one spelling present (any of the five), or adds `"library": [".airsl/types"]` under an existing top-level `workspace` object, or adds `"workspace.library": [".airsl/types"]`.
- L11: every `Write` is `serde_json::to_string_pretty` plus `\n`. L2 puts a stripped BOM back in front of it.

**Proof:** each test feeds bytes and asserts the exact plan or refusal text.
- `no_file_writes_the_minimal_luarc_bytes` (L1): `None` gives `Write` of exactly `{\n  "workspace.library": [\n    ".airsl/types"\n  ]\n}\n`.
- `a_bom_is_parsed_after_and_written_back` (L2): `EF BB BF {}` gives a `Write` whose bytes start with `EF BB BF` followed by the L1 text. `EF BB BF` before an already-listing file gives `Unchanged`.
- `an_empty_or_whitespace_file_is_treated_as_an_empty_object` (L3): `""` and `" \n\t"` give the L1 bytes.
- `a_file_that_is_not_plain_json_is_refused_with_the_serde_error` (L4): uses the probe inputs from `probes/serde-main.rs:3-4` and `probes/more-main.rs:7`.
  - The comment input `{\n  // keep me\n  "workspace.library": []\n}` gives `it is not plain JSON (key must be a string at line 2 column 3)`.
  - `{"a": 1,}` gives `it is not plain JSON (trailing comma at line 1 column 9)`.
  - `{"a":"<0xFF>"}` gives `it is not plain JSON (invalid unicode code point at line 1 column 8)`.
  - These texts come from `from_slice`, the reader L4 uses (`probes/probe-more.txt`).
- `a_non_object_top_level_is_refused` (L5): `[]` gives `its top level is not a JSON object`.
- `a_dotted_library_that_is_not_an_array_is_refused` (L6): ``{"workspace.library": "x"}`` gives `` `workspace.library` is not an array``.
- `a_workspace_that_is_not_an_object_or_a_nested_library_that_is_not_an_array_is_refused` (L6, L7):
  - `{"workspace": 1}` gives `` `workspace` is not an object``;
  - `{"Lua": 1}` gives `` `Lua` is not an object``;
  - `{"workspace": {"library": "x"}}` gives `` `workspace` → `library` is not an array``;
  - `{"Lua": {"workspace": {"library": 2}}}` gives `` `Lua` → `workspace` → `library` is not an array``.
- `a_listed_entry_is_unchanged_and_bytes_untouched` (L8): a dotted listing, a nested listing, and a dotted listing beside unrelated keys (`{"runtime.version": "Lua 5.4", "workspace.library": [".airsl/types"]}`) all give `Unchanged`.
- `the_same_path_in_two_spellings_is_refused_when_a_rewrite_is_needed` (L9): ``{"workspace": {"library": []}, "workspace.library": []}`` gives ``it sets `workspace.library` in two spellings``.
- `a_listed_entry_in_one_of_two_spellings_is_refused`: ``{"workspace": {"library": [".airsl/types"]}, "workspace.library": []}`` gives the same refusal (§9).
- `a_map_valued_setting_in_two_spellings_is_refused`: the §6.1 example ``{"diagnostics.severity": {"a": "Error"}, "diagnostics": {"severity": {"b": "Hint"}}}`` gives ``it sets `diagnostics.severity` in two spellings``.
- `a_lua_prefixed_spelling_is_appended_to`: ``{"Lua.workspace.library": ["lib"]}`` becomes ``{"Lua.workspace.library": ["lib", ".airsl/types"]}`` and gains no `workspace.library` key. ``{"Lua": {"workspace": {"library": []}}}`` gets the entry appended inside.
- `a_lua_prefixed_listing_is_unchanged`: ``{"Lua.workspace.library": [".airsl/types"]}`` gives `Unchanged`.
- `the_entry_is_appended_or_added_in_the_spelling_present` (L10): each `Write`, parsed back, equals the expected value.
  - `{"workspace.library": ["x"]}` becomes `{"workspace.library": ["x", ".airsl/types"]}`.
  - `{"workspace": {"library": ["x"]}}` becomes `{"workspace": {"library": ["x", ".airsl/types"]}}`.
  - `{"workspace": {"checkThirdParty": false}}` becomes `{"workspace": {"checkThirdParty": false, "library": [".airsl/types"]}}`.
  - `{"a": 1}` becomes `{"a": 1, "workspace.library": [".airsl/types"]}`.
- `a_rewrite_is_pretty_printed_with_sorted_keys_and_a_trailing_newline` (L11): ``{"z":1,"a":{"y":1,"b":2}}`` comes back with keys sorted at every depth, 2-space indent, and a final `\n`.
- `duplicate_keys_keep_the_last_value` (L12): ``{"workspace.library": [], "workspace.library": [".airsl/types"]}`` gives `Unchanged`.
- `parse_refusals_win_over_shape_refusals`: `[1, 2,]`, a non-object with a trailing comma, is refused with exactly `it is not plain JSON (trailing comma at line 1 column 7)` (`probe-more.txt`), not with the L5 text.
- `a_listed_entry_in_both_spellings_is_unchanged`: listed both dotted and nested gives `Unchanged` (§9).
- `disjoint_spellings_are_rewritten`: dotted `workspace.library` without the entry plus ``workspace: {checkThirdParty: false}`` gives a `Write` that keeps `checkThirdParty` and appends the entry (§9).
- `key_paths_split_nested_dotted_keys`: one file, ``{"workspace": {"a.b": 1}, "workspace.a": {"b": 2}}``. Both top-level keys reach `workspace.a`, the shortest shared path of two segments. It has no library entry, so a rewrite is needed, and it is refused with ``it sets `workspace.a` in two spellings``.
- `a_dot_slash_entry_is_not_listed`: ``["./.airsl/types"]`` gets `.airsl/types` appended (§9).
- `non_string_library_elements_are_kept`: ``[1, null, ".x"]`` keeps all three and appends the entry (§9).

**Done when:** `cargo test -p airsl luals::luarc` passes and `cargo make dod` is green. Commit `feat(airsl): plan luarc edits without touching the filesystem`.

---

## Risks

- **Rule order is the semantics.** L4 must run before any shape rule, and L8 before L9. `parse_refusals_win_over_shape_refusals` and `a_listed_entry_in_both_spellings_is_unchanged` pin the two orderings most likely to be swapped.
- **The serde error texts are dependency output.** They are pinned by `serde_json = 1.0.151` in `Cargo.lock`. An upgrade that rewords them fails the L4 test, which is the intended signal.
- **Option not taken:** keeping key order or comments. That is a non-goal (§12).

## Proof

- `cargo make dod` is green.
- `cargo test -p airsl luals::luarc` runs one test per L-rule plus the §9 cases.
