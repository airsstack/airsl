---
status: done
created: 2026-11-25
depends-on: [03]
built-on: 044432888b01
disproves: 044432888b01
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

## Review findings

There was one review: 0🔴 3🟡 7🔵. The plan's checks pass: 22 named tests with exact texts, L4 runs before any shape rule, L8 runs before L9, and the L9 walk and the L10 targets match the plan. The full report is at `.airsstack/cc/plugins/claudestacks/handoff/20261125-164553-edd2/02-reviewer-plan04.md`.

- **Fixed** (blocking 🟡): a sixth spelling, `Lua` → `workspace.library`, was missed. `{"Lua": {"workspace.library": ["lib"]}}` gained a top-level `workspace.library` beside it, so one setting was written in two spellings. It is now in `SPELLINGS`. New tests: `a_lua_object_with_a_dotted_library_is_a_spelling` and `a_lua_object_with_a_dotted_library_that_is_not_an_array_is_refused`.
- **Fixed** 🟡: the leading `Lua` strip in L9 was unguarded. New test `a_lua_prefixed_and_a_plain_spelling_conflict`.
- **Fixed** 🟡: the byte-order tie-break was unguarded. New test `the_shortest_shared_path_tie_breaks_by_byte_order`.
- **Fixed** 🔵: the doc on the `written` test helper, the missing doc on `split`, and the segment count, which was recomputed after `join`.
- For all of the above, `cargo test -p airsl luals::luarc` → `test result: ok. 26 passed; 0 failed`.
- Fixed in the follow-up round 🔵 (was open): when a spelling is not an array and another is blocked by a non-object parent, the not-an-array refusal is reported first. No test pins this order. The plan sets no order between L6 and L7.
- Fixed in the follow-up round 🔵 (was open): `Result<LuarcPlan, String>` carries the refusal as a bare string. This is the signature the plan asks for.
- Fixed in the follow-up round 🔵 (was open): `map_err(|e| e.to_string())` on the serialization, which cannot fail, returns a reason that is not shaped like "it …".
- Fixed in the follow-up round 🔵 (was open): `u8::is_ascii_whitespace` counts form feed as whitespace, so a file holding only form feed (0x0C) is treated as empty (L3) instead of being refused by L4.

## Deviations

- 2026-11-25. The spec §6.1 (D22) claim was: "So `workspace.library` has five spellings". It is disproved by the source the spec itself cites, `probes/luals-config-207-224.lua` (LuaLS 3.19.2 `expand`). That code joins nested keys with `.` and applies `fullKey` or `Lua.` + `fullKey` when either is a setting. `{"Lua": {"workspace.library": …}}` therefore gives `fullKey = "Lua.workspace.library"`, a setting. The key paths that join to `[Lua.]workspace.library` are six: the five listed plus `Lua` → `workspace.library`. Six spellings were built, and the new tests above fail on the five-spelling code. `disproves: 044432888b01` is set, and `design` clears it with a one-word fix to the spec (five → six, plus the sixth bullet).
- 2026-11-25. There was no task-briefer pass. This is a one-task plan, and the coder read the task section directly.
- 2026-11-25. The review ran while plan 05's coder was editing `luals/setup.rs`. A clippy failure in setup.rs during the review was out of scope. The review checked luarc.rs in a scratch crate with the workspace's lint table instead.
- 2026-11-25. No commit was made. Execute commits nothing unless told to.

## Follow-up round (2026-11-25, at the author's request: "fix all of findings, fix the spec")

- Every open 🔵 above is fixed:
  - `Refusal(String)` with `into_reason` replaces the bare string.
  - L3 treats only space, tab, LF and CR as empty, and a lone form feed is an L4 refusal: `it is not plain JSON (expected value at line 1 column 1)`.
  - The L6-before-L7 order is pinned with `{"workspace": 1, "Lua.workspace.library": "x"}`.
  - A serialization failure gives `it could not be written as JSON (…)`.
- **Fixed 🔴** (spec review): L9 refused only when two *top-level keys* reached one setting. The sixth spelling shares the `Lua` key with `Lua` → `workspace` → `library`, so `{"Lua": {"workspace": {"library": []}, "workspace.library": []}}` was rewritten, and LuaLS still reported `Undefined global` (`probes/luals-lua-both.out`). L9 now compares the JSON-key spellings at any depth. New tests: `two_spellings_under_one_lua_object_are_refused`, `a_map_valued_setting_in_two_spellings_under_lua_is_refused`, `a_maps_own_keys_are_not_two_spellings`, `several_non_array_spellings_name_the_first_in_list_order` and `several_blocked_parents_name_the_first_in_list_order`. After the fix, the same LuaLS case is refused and the file is left untouched (same record).
- The spec was redesigned to match: `spec-superseded-2026-11-25-3.md` is the old text, and the new `spec.md` is a draft awaiting the author's approval.
- Deviation: plan 04's Architecture and Task 1 still name `Result<LuarcPlan, String>` and "five spellings". This plan is kept as executed, and the spec carries the current signature (`Refusal`) and the six spellings.

## Follow-up round 2 (2026-11-25, post-commit review of `889ce3f`, at the author's request: "fix review findings")

- **Fixed 🔴**: L9 split a map's own dotted keys, so `{"runtime.builtin": {"jit.profile": …, "jit.util": …}}` was refused as ``it sets `runtime.builtin.jit` in two spellings`` (`probes/luals-map-keys.out`, "Before the fix"). L9 now compares whole setting names: two keys conflict only when their JSON keys join to the same name. The walk is a segment tree in `luarc.rs` (`Names`, `Segment`), which replaces `Trail` and `Reached`, so the path and its spelling no longer share one type. After the fix, setup adds the entry and LuaLS still applies the map (same record, "After the fix").
- **Fixed 🟡**: memory grew with the square of a long dotted key. `probes/long-key-cost.out` (debug builds): the superseded walk took 13.24 s and 745,799,680 bytes on 8,000 dots; the tree takes 0.03 s and 10,240,000 bytes on the same file, and 0.04 s and 47,366,144 bytes on 100,000 dots. Test `a_key_with_many_dots_is_planned` uses 100,000 dots.
- **Fixed 🟡**: the fewest-segments pick had no test. Added `the_path_with_the_fewest_segments_is_named`.
- New tests: `dotted_map_keys_are_not_two_spellings`, `dotted_keys_that_share_a_prefix_are_not_two_spellings` and `map_keys_that_join_to_one_name_are_refused`. Changed expectations: `key_paths_split_nested_dotted_keys` now names `workspace.a.b`, and `the_shortest_shared_path_tie_breaks_by_byte_order` names `a.b`, because the tie-break now compares segment by segment; it is renamed `names_of_equal_length_are_compared_segment_by_segment`. Added `empty_segments_make_a_different_name` after the spec review.
- Red first: with the old walk and the new tests, 5 tests fail (`cargo test -p airsl luals::luarc`: `test result: FAILED. 34 passed; 5 failed`). With the new walk: `test result: ok. 39 passed; 0 failed`, and 40 after the spec review's test.
- `cargo make dod`: exit 0 (757 lib tests, 92 CLI tests, 19 doctests).
- The spec was redesigned to match: `spec-superseded-2026-11-25-4.md` is the old text, and the new `spec.md` is a draft awaiting the author's approval.
