---
status: approved
created: 2026-11-25
depends-on: [02]
built-on: 044432888b01
---

# Type File Render Implementation Plan

**Goal:** `TypeFile::render` turns a `ModuleSet` and a `RootTable` into byte-stable LuaLS type-file text.

**Architecture:** `luals/render.rs` is pure: no I/O, no clock, no version. It calls each module's `describe()` through plan 02's `pub(crate)` helper in `luals/verify.rs`, which wraps a failure as `DescribeFailed`. It reads descriptions only through the `pub(crate)` readers from plan 01, and spells types with `LuaType`'s `pub(crate)` spelling, passing the record prefix `<root>.<module>`. `luals/mod.rs` gains one `mod` line and one re-export.

**Tech Stack:** Rust 1.94, crate `airsl`, cargo-make.

**Content authority:** spec §4, §9 (the `render.rs` rows), §10 (`luals/render.rs` tests), §11 (the `TypeFile::render` doctest).

---

## Files that change

```
crates/airsl/src/luals/render.rs   — create  TypeFile: render, file_name, as_str
crates/airsl/src/luals/mod.rs      — modify  mod render; pub use TypeFile
```

## Order of work

### Task 1 — Render described modules as a LuaLS type file

**Files:**
- Create `crates/airsl/src/luals/render.rs`: `TypeFile` (derives `Debug`, `Clone`, `PartialEq`, `Eq`), `TypeFile::render(&ModuleSet, &RootTable) -> Result<TypeFile>`, `file_name() -> String` (`"<root>.lua"`) and `as_str() -> &str`. The module doc follows the crate's shape.
- Modify `crates/airsl/src/luals/mod.rs`: add `mod render;` and `#[doc(inline)] pub use render::TypeFile;`.

**Change:**
- The output is exactly the §4 layout:
  - the two header lines;
  - the root class block;
  - then, per module in `ModuleSet` insertion order: the module class with one `---@field` per described field, then one class block per record, then one block per function.
- Blocks are separated by one empty line, and the file ends with a single `\n`.
- A multi-line doc gives one `---<line>` per line, and an empty doc gives nothing.
- A parameter, variadic or field doc follows its type after ` # `, only when it is non-empty (D21).
- An optional parameter is written `<name>?`, and the variadic `...`.
- A module whose name is a Lua reserved word uses `<root>["<m>"] = {}` and `<root>["<m>"].<f> = function(<params>) end`. A `ModuleName` may be a reserved word (`crates/airsl/src/types/module_name.rs:34-57`), so the check is the `RESERVED_WORDS` list in `types/lua_name.rs`.
- Records use the prefix `<root>.<module>`.
- Add a rustdoc doctest on `TypeFile::render` that renders a one-module set and checks `file_name()`.

**Proof:**
- `renders_the_header_and_root_class`: with an empty `ModuleSet`, the text is exactly the two header lines, a blank line, and the three-line root block ending in `\n`. This also covers the §9 "empty module set" row.
- `renders_a_reserved_word_module_with_brackets`: a stub module named `end` with function `hello(name)` renders `airsstack["end"] = {}` and `airsstack["end"].hello = function(name) end`.
- `renders_module_fields_on_the_module_class`: a described field `version: string` appears as `---@field version string # The host's version.` between `---@class airsstack.info` and `airsstack.info = {}`.
- `renders_optional_and_variadic_params`: an optional `algorithm` renders `---@param algorithm? "sha256"|"sha1" # Defaults to sha256.`, and an undocumented variadic renders `---@param ... string`, with the function line ending `(...) end`.
- `omits_empty_docs`: a function with an empty doc and an undocumented parameter renders no `---` doc line, no `#`, and no trailing space after the parameter type.
- `inline_docs_follow_a_hash`: a field doc `| none when unset` renders as `---@field x string # | none when unset`.
- `renders_multiple_returns`: two `returns` give two `---@return` lines in order.
- `renders_records_before_functions`: a module with a function added before a record still renders the record class block before the function block.
- `the_stdlib_type_file_compiles_as_lua`: rendering `modules::stdlib()?` and passing the text to `Engine::check` (`crates/airsl/src/engine.rs:321`) through `Script::from_source` succeeds.
- `rendering_twice_is_byte_identical`: two renders of `stdlib()` give equal strings.
- `a_host_root_names_the_file_and_classes`: under `RootTable::new("agentstacks")`, `file_name()` is `agentstacks.lua`, the text contains `---@class agentstacks.json`, and it never contains `airsstack`.
- `a_describe_error_names_the_module`: a stub whose `describe` returns `Error::InvalidDescription { reason: "x".into() }` makes `render` return an error that displays exactly ``cannot describe host module `stub`: invalid module description: x``.

The text of every line above is the §4 layout with D21 applied.

**Done when:** `cargo test -p airsl luals::render` passes and `cargo make dod` is green. Commit `feat(airsl): render described modules as a LuaLS type file`.

---

## Risks

- **Whitespace is the contract.** Byte-stability is what makes setup's "unchanged" report true. The exact-text tests (`renders_the_header_and_root_class`, `omits_empty_docs`) are the guard against a stray trailing space or doubled blank line.
- **Not done here:** writing the file (plan 05), and the LuaLS acceptance run on the generated stdlib file (plan 06).

## Proof

- `cargo make dod` is green.
- `the_stdlib_type_file_compiles_as_lua` proves the rendered stdlib is valid Lua on the checkout.
