---
status: approved
created: 2026-11-25
built-on: 044432888b01
---

# Description Model Implementation Plan

**Goal:** A host can build a validated `ModuleDescription` of a module's Lua surface.

**Architecture:** `LuaName` joins the other parsed names in `types/`, and the reserved-word list moves into it. The new `description/` folder holds one file per role: `lua_type.rs` (the type expression and its LuaLS spelling), `signature.rs` (`Function`, `Signature`, `Param`, `Presence`), `record.rs` (`Record`, `Field`) and `module.rs` (`ModuleDescription`, its builder and `build()`, the single validation point). Builder pieces hold raw text. Only `build()` parses names through `LuaName` and checks the §3.3 rules. Every description type exposes its parts through `pub(crate)` read accessors that `luals` uses later; no reader is public (spec §3.1). `description/mod.rs` is export-only.

**Tech Stack:** Rust 1.94 (workspace pin), crate `airsl`, `thiserror` (the existing `Error` enum), cargo-make.

**Content authority:** spec §2, §3.1, §3.2, §3.3, §8 (`InvalidDescription`), §10 (the `types/` and `description/` tests), §11 (the `LuaName::new` and `ModuleDescription::builder` doctests).

---

## Files that change

```
crates/airsl/src/types/lua_name.rs          — create  LuaName and RESERVED_WORDS
crates/airsl/src/types/root_table.rs        — modify  use RESERVED_WORDS from lua_name.rs
crates/airsl/src/types/mod.rs               — modify  declare lua_name, re-export LuaName
crates/airsl/src/lib.rs                     — modify  pub use types::LuaName; pub mod description
crates/airsl/src/description/mod.rs         — create  export-only table of contents
crates/airsl/src/description/lua_type.rs    — create  LuaType and its LuaLS spelling
crates/airsl/src/description/signature.rs   — create  Function, Signature, Param, Presence
crates/airsl/src/description/record.rs      — create  Record, Field
crates/airsl/src/description/module.rs      — create  ModuleDescription, ModuleDescriptionBuilder, build()
crates/airsl/src/error.rs                   — modify  + Error::InvalidDescription { reason }
```

## Order of work

### Task 1 — Add `LuaName` and move the reserved words into it

**Files:**
- Create `crates/airsl/src/types/lua_name.rs`: the `LuaName` newtype, `LuaName::new(&str) -> Result<LuaName>`, `as_str`, `Display`, `Debug`. It also holds `pub(crate) const RESERVED_WORDS`, the 22 words now at `crates/airsl/src/types/root_table.rs:24`, with their doc comment adjusted to this file.
- Modify `crates/airsl/src/types/root_table.rs`: delete its own `RESERVED_WORDS` and import the one from `lua_name.rs`. Behaviour is unchanged.
- Modify `crates/airsl/src/types/mod.rs`:
  - add `pub mod lua_name;` and `pub use lua_name::LuaName;`, keeping the file's existing order and style;
  - update its module doc, whose count of validated types and `Responsibilities:` list must include `LuaName`.
- Modify `crates/airsl/src/lib.rs`: add `LuaName` to the existing `pub use types::{…}` list.

**Change:** `LuaName::new` accepts an ASCII identifier (a letter or `_`, then letters, digits or `_`) that is not one of the 22 lowercase reserved words. It refuses anything else with the existing `Error::InvalidName` with `kind: "Lua name"`, and the four reasons are the exact texts in the spec's §3.3 name table. The module doc follows the crate's shape: what it is, why it is its own module, `Responsibilities:`, `Non-responsibilities:`. Add a rustdoc doctest on `LuaName::new` showing one accepted name and one refused name.

**Proof:**
- `accepts_identifiers_with_mixed_case_and_underscores`: `End`, `_x` and `hookEventName` are accepted, and `as_str` returns the input.
- `refuses_an_empty_name`: the display equals ``invalid Lua name ``: must not be empty``.
- `refuses_a_leading_digit`: for `9x`, the display equals the §3.3 text.
- `refuses_a_hyphen`: for `a-b`, the display equals the §3.3 text.
- `refuses_every_reserved_word`: each of the 22 words is refused with ``must not be a Lua reserved word``.
- `accepts_a_capitalised_reserved_word`: `End` and `Nil` are accepted.
- The existing `types/root_table.rs` tests pass unchanged.

**Done when:** `cargo test -p airsl types::` passes and `cargo make dod` is green. Commit `feat(airsl): add LuaName and share the Lua reserved words`.

### Task 2 — Add `LuaType` and signatures with their LuaLS spelling

**Files:**
- Create `crates/airsl/src/description/lua_type.rs`: `LuaType`, an opaque `pub struct` with a private representation (D25). It has the associated constants `NIL`, `BOOLEAN`, `INTEGER`, `NUMBER`, `STRING`, `ANY` and `TABLE`, and the constructors `literal`, `array`, `map`, `optional`, `function(Signature)`, `record(name)` and `or`. Also a `pub(crate)` spelling function that takes the record prefix `<root>.<module>` and returns the LuaLS text.
- Create `crates/airsl/src/description/signature.rs`: `Presence { Required, Optional }`; `Param` (name, type, presence, doc); `Signature` with `new`, `param`, `optional`, `variadic` and `returns`, and no docs; `Function` with `new(name, doc)`, `param`, `optional`, `variadic(ty, doc)` and `returns`. Each has `pub(crate)` readers for its parts. A `Function` owns a `Signature`-shaped parameter list plus per-parameter docs and a variadic doc.
- Create `crates/airsl/src/description/mod.rs`: `//!` docs, `mod lua_type; mod signature;`, and `#[doc(inline)] pub use` for `LuaType`, `Function`, `Signature`, `Param` and `Presence`.
- Modify `crates/airsl/src/lib.rs`: add `pub mod description;`.

**Change:**
- Every public type in this plan derives `Debug`, `Clone`, `PartialEq` and `Eq` (D25, M-PUBLIC-DEBUG). That covers `LuaType`, `Function`, `Signature`, `Param`, `Presence`, `Record`, `Field`, `ModuleDescription` and `ModuleDescriptionBuilder`. In the tests below, `String`, `Integer` and so on stand for `LuaType::STRING`, `LuaType::INTEGER` and the rest.
- The two files reference each other (`LuaType::function` holds a `Signature`, and parameters hold `LuaType`s), so they land together.
- `LuaType::optional` is idempotent.
- `or` builds a union that is never empty. Following D19 (§3.2):
  - nested unions flatten into one member list, kept in left-to-right order;
  - an optional member makes the whole union optional, so `?` never appears inside a union.
- `literal("")` is allowed and spells `""`.
- The spelling follows the §3.2 table exactly:
  - a function member of a union is parenthesised;
  - an array of a union, an optional or a function is parenthesised;
  - an optional of a union or a function is parenthesised;
  - `fun(a: T, b?: U, ...: V): R1, R2` omits the `: …` part when there are no returns;
  - `record("Stat")` spells `<prefix>.Stat`.
- No validation happens here. Raw text is stored as given, and `build()` (Task 4) checks it.

**Proof:**
- `primitives_spell_as_their_lua_names`: the seven unit variants spell `nil boolean integer number string any table`.
- `a_literal_spells_quoted`: `literal("sha1")` spells `"sha1"`.
- `a_union_joins_members_with_a_bar`: `String.or(Integer)` spells `string|integer`.
- `an_array_appends_brackets`: `array(String)` spells `string[]`, and `array(String.or(Integer))` spells `(string|integer)[]`.
- `a_map_spells_as_table_of_key_and_value`: `map(String, String)` spells `table<string, string>`, and `optional(map(Integer.or(String), String))` spells `table<integer|string, string>?`.
- `an_optional_appends_a_question_mark`: `optional(String.or(Integer))` spells `(string|integer)?`, and the optional of `fun(x: string): boolean` spells `(fun(x: string): boolean)?`.
- `a_function_spells_params_and_returns`: a signature with `a: string`, optional `b: integer`, variadic `number` and returns `string, integer` spells `fun(a: string, b?: integer, ...: number): string, integer`. With no returns it spells `fun(a: string, b?: integer)`.
- `a_record_spells_with_the_module_prefix`: `record("Stat")` with prefix `airsstack.fs` spells `airsstack.fs.Stat`.
- `optional_of_an_optional_is_one_optional`: `optional(optional(String))` equals `optional(String)` and spells `string?`.
- `a_function_in_a_union_is_parenthesised`: `function(fun(x: string): boolean).or(String)` spells `(fun(x: string): boolean)|string`.
- `an_array_of_an_optional_is_parenthesised`: `array(optional(String))` spells `(string?)[]`, and an array of `fun(x: string): boolean` spells `(fun(x: string): boolean)[]`.
- `an_optional_array_needs_no_parentheses`: `optional(array(String))` spells `string[]?`, and `array(map(String, String))` spells `table<string, string>[]`.
- `a_union_of_unions_is_flattened`: `String.or(Integer.or(Boolean))` and `String.or(Integer).or(Boolean)` both spell `string|integer|boolean`.
- `an_optional_member_makes_the_whole_union_optional`: `String.or(optional(Integer))` and `optional(String).or(Integer)` both spell `(string|integer)?`.
- `an_empty_literal_spells_as_two_quotes`: `literal("")` spells `""`, and `optional(literal("a"))` spells `"a"?`.
- `param_optional_and_variadic_record_their_presence`: `param` reads back `Required`, `optional` reads back `Optional`, and `variadic` is stored as the tail, not as a named parameter.
- `returns_accumulate_in_order`: three `returns` calls read back in call order.
- `a_signature_keeps_parameters_in_declaration_order`: parameters read back in the order added.

**Done when:** `cargo test -p airsl description::` passes and `cargo make dod` is green. Commit `feat(airsl): add LuaType and function signatures with LuaLS spelling`.

### Task 3 — Add records

**Files:**
- Create `crates/airsl/src/description/record.rs`: `Record::new(name, doc)`, `Record::field(name, ty, doc)`, and `Field` (name, type, doc), each with `pub(crate)` readers.
- Modify `crates/airsl/src/description/mod.rs`: add `mod record;` and re-export `Record` and `Field`.

**Change:** A record is a named table shape whose fields keep their declaration order. It holds raw text only. Validation is Task 4.

**Proof:**
- `fields_keep_declaration_order`: three fields read back in the order added.
- `a_record_carries_its_name_and_doc`: the name and doc read back unchanged.

**Done when:** `cargo test -p airsl description::record` passes and `cargo make dod` is green. Commit `feat(airsl): add record descriptions`.

### Task 4 — Add `ModuleDescription` and its validating `build()`

**Files:**
- Create `crates/airsl/src/description/module.rs`: `ModuleDescription::builder(doc)`, then `ModuleDescriptionBuilder::{function, field, record, build}`. `ModuleDescription` has `pub(crate)` readers for the doc, the members in insertion order (each a function or a field), and the records in insertion order.
- Modify `crates/airsl/src/description/mod.rs`: add `mod module;` and re-export `ModuleDescription` and `ModuleDescriptionBuilder`.
- Modify `crates/airsl/src/error.rs`: add `InvalidDescription { reason: String }`, displayed as `invalid module description: {reason}`, with a doc sentence. Its `# Errors` references point at it.

**Change:**
- `build()` walks the description in the fixed order of §3.3 (module doc, then members, then records, then the cross-checks) and returns the first problem.
- Detection points follow §3.3: a duplicate is reported at its second occurrence, a parameter after the variadic at that parameter, and a `LuaType::record(n)` name only in the step-4 reference check.
- It also enforces the §3.3 rows added by D20 and D21:
  - a module field whose type is a function type, or the optional of one, is refused;
  - inside a function type's parameters, variadic and returns, a function type, a union outside a map's `<…>`, or a type spelled with a leading `(` is refused;
  - a function type inside a map key or value is refused;
  - a doc line whose first character after spaces, tabs, vertical tabs and form feeds is `@` is refused;
  - a literal refusal writes `{text}` with `escape_debug`.
- Names go through `LuaName::new`. Every other problem is `InvalidDescription`, using the exact reason texts and `{owner}` / `{fn}` forms in §3.3. That includes ``field `x` of record `R` `` for a `Signature` nested in a record field's type.
- Accepted on purpose: a required parameter after an optional one, and an unused record.
- Add a rustdoc doctest on `ModuleDescription::builder` that builds the spec's `hash.hash_file` example from §3.1.

**Proof:** each test asserts the full `Display` of the returned error, as written in §3.3.
- `a_member_described_twice_is_refused`: a function and a field named `f` give ``invalid module description: `f` is described twice``.
- `a_record_sharing_a_member_name_is_refused`: the §3.3 "member and record" text.
- `a_parameter_described_twice_is_refused`: two `p` on function `f` give ``parameter `p` of `f` is described twice``.
- `two_variadics_are_refused`: ``f` has two variadic parameters``.
- `a_parameter_after_the_variadic_is_refused`: ``parameter `p` of `f` comes after the variadic parameter``.
- `a_record_described_twice_is_refused`: ``record `R` is described twice``.
- `a_record_field_described_twice_is_refused`: ``field `x` of record `R` is described twice``.
- `an_undescribed_record_reference_is_refused`: `LuaType::record("Nope")` gives ``record `Nope` is used but not described``.
- `a_carriage_return_in_a_doc_is_refused`: one case per owner form (the module, a function, a parameter, the variadic parameter, a field, a record, a record field), each naming its `{owner}`.
- `a_doc_line_starting_with_at_is_refused`: a module doc line `  @deprecated` gives ``a line in the doc of the module starts with `@` ``.
- `a_multi_line_parameter_doc_is_refused`: a parameter, the variadic, a module field and a record field with `\n` in their docs each give ``the doc of {owner} must be one line``. Multi-line module, function and record docs are accepted.
- `a_literal_with_a_quote_backslash_or_newline_is_refused`: each of `"`, `\`, `\n` and `\r` gives the §3.3 literal text.
- `an_invalid_name_is_reported_as_an_invalid_lua_name`: a function named `end` gives ``invalid Lua name `end`: must not be a Lua reserved word``.
- `a_signature_problem_in_a_record_field_names_the_field_and_record`: a record `Compiled` whose field `find` is typed `fun(text: string, text: string)` gives ``invalid module description: parameter `text` of field `find` of record `Compiled` is described twice``.
- `a_signature_problem_in_a_module_field_or_nested_parameter_names_the_member`: a module field `on` typed `fun(a: any, a: any)` gives ``parameter `a` of `on` is described twice``. A function `f` with a parameter typed `fun(...: any, ...: any)` gives ``invalid module description: `f` has two variadic parameters``.
- `a_function_typed_module_field_is_refused`: `field("on", function(…), …)` gives ``invalid module description: field `on` has a function type; describe it with `function` instead``.
- `a_function_type_inside_a_function_type_is_refused`: a function `f` with a parameter typed `fun(cb: fun(): string, x: integer)` gives ``a function type in parameter `p` of `f` must not hold a function, a union or a parenthesised type as a parameter or return type`` (with the module-description prefix).
- `a_union_or_parenthesised_type_inside_a_function_type_is_refused`: one case with a return typed `optional(String.or(Integer))` inside a `fun(...)`, and one with a parameter typed `String.or(Integer)` inside a `fun(...)`. Both give the D20 text.
- `a_function_type_inside_a_map_is_refused`: a field typed `map(String, function(…))` gives ``a map type in field `x` must not hold a function type``.
- `a_doc_line_starting_with_at_after_a_tab_is_refused`: a module doc `\t@deprecated` gives the `@` text.
- `a_literal_refusal_escapes_its_text`: `literal("a\nb")` gives a message with no line break, containing `a\nb` as two characters `\` and `n`.
- `build_reports_the_first_problem_in_declaration_order`: a description with a bad module doc and a later duplicate member reports the module doc. With only a duplicate member and an undescribed record reference, it reports the duplicate member.
- `a_required_parameter_may_follow_an_optional_one`: builds.
- `an_unused_record_is_accepted`: builds.
- `a_valid_description_builds`: the §3.1 `hash` example builds, and its readers return the members in order.

**Done when:** `cargo test -p airsl description::` passes and `cargo make dod` is green. Commit `feat(airsl): add ModuleDescription with validating build`.

---

## Risks

- **Union normalisation is observable in the type file.** The two D19 tests pin the order and the lifted `?`.
- **Mutual dependency between `lua_type.rs` and `signature.rs`.** Splitting them across tasks would leave one half uncompilable, so Task 2 lands both.
- **The validation order is observable** through "first problem wins". A walk in a different order passes every single-problem test but fails `build_reports_the_first_problem_in_declaration_order`. That test is the guard.
- **Not done here:** nothing calls `describe` yet, and nothing renders. Those come in plans 02 and 03.

## Proof

- `cargo make dod` is green after each task: fmt-check, clippy with `-D warnings`, rustdoc with `-D warnings`, the tests, and the doctests.
- `cargo test -p airsl types:: description::` runs every test named above.
