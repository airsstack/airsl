---
status: approved
created: 2026-11-25
depends-on: [01]
built-on: ae381756f52b
---

# Root Answer Implementation Plan

**Goal:** `require("<root>")` returns the engine's own root table under every `require` disposition.

**Architecture:**
- New `crates/airsl/src/require/root.rs` is the only place that knows where the root is kept for
  `require` and how a name is matched (spec §4.1, §4.2, §7.1):
  - The registry key is `airsl.require.root`.
  - `register(lua, name: &RootTable, table: &mlua::Table, surface: LanguageSurface) -> Result<()>`
    stores the table under that key on every surface, and also writes
    `package.loaded[name] = table` on `LanguageSurface::Full` only. It matches `surface`
    exhaustively, with no wildcard. A failure maps to
    `Error::EngineSetup { stage: "registering the root table", source }`.
  - `answer(lua, name: &RootTable, requested: &[u8]) -> mlua::Result<Option<mlua::Table>>` returns
    the registry table when `requested` equals `name.as_str()` byte for byte.
- `builder.rs` `install_modules` (`builder.rs:240-260`) calls `register` immediately before the
  line that sets the global (`builder.rs:258`), because that line moves `root` (spec §4.1).
- `require/loader.rs` (from plan 01) has three installers, each capturing a clone of the engine's
  `RootTable` and asking `root::answer` first (spec §3 rules, §4.3):
  - `RequireLoader::new(dir, root).install`: converts the argument to a Lua string as today,
    answers the root, then validates and resolves as today.
  - `install_refusal(lua, root, detail)`: reads the first argument without converting it. A Lua
    string equal to the root returns it. Anything else gets today's `RequireDenied`.
  - `install_root_only(lua, root)`: in order, converts, answers the root, validates as a
    `RequireTarget` (today's `InvalidName`), then raises `RequireNoDirectory`. A failure to
    install maps to `Error::EngineSetup { stage: "root-only require", source }`.
  - `RequireLoader::remove` is deleted.
- `RequireDisposition::Absent` is renamed `RootOnly`. `decide` keeps its signature and its
  exhaustive match.
- `engine.rs` `set_require` (`engine.rs:283-297`) passes the engine's `RootTable` (`engine.rs:53`)
  to each installer. Its `Native` arm still installs nothing.
- `require/mod.rs` declares `pub(crate) mod root;`, so `builder.rs` names
  `crate::require::root::register`, the path spec §4.1 uses. The crate already does this with
  `pub(crate) mod access;` (`modules/mod.rs:33`).
- `root.rs` opens with the project's module-doc shape: what it is, why it exists as its own module,
  `Responsibilities:`, `Non-responsibilities:` (spec §7.3). It carries the
  `#![expect(clippy::redundant_pub_crate, reason = …)]` attribute only if the lint fires there.
- Every doc comment that describes the old behaviour is rewritten in the task that changes that
  behaviour, as the "Doc comments" lines in Tasks 2 and 3 list. Plan 01 moved these passages
  verbatim, so each is cited by its item name and its old `require_loader.rs` line.

**Tech Stack:** Rust 1.94, crate `airsl`, `mlua` 0.12.0 (via `airsl::mlua`), cargo-make.

**Content authority:** spec §1 (D1, D2, D3, D4, D6, D7), §3 (every row and rule), §4, §5, §7.1,
§7.2, §8 (every test except `luals/render.rs`), §9 (the `script.rs` and `engine.rs` doc rows).

---

## Files that change

```
crates/airsl/src/require/root.rs          — create  register, answer, the registry key, their tests
crates/airsl/src/require/mod.rs           — modify  + mod root; doc lists it
crates/airsl/src/require/loader.rs        — modify  installers answer the root first; install_root_only; remove() deleted; tests
crates/airsl/src/require/disposition.rs   — modify  Absent → RootOnly; its doc and test
crates/airsl/src/builder.rs               — modify  install_modules calls require::root::register
crates/airsl/src/engine.rs                — modify  set_require passes the RootTable; RootOnly arm; set_require doc; tests
crates/airsl/src/error.rs                 — modify  + RequireNoDirectory; RequireDenied doc; test
crates/airsl/src/script.rs                — modify  Script::from_source rustdoc (:32)
crates/airsl/src/extension/loaded.rs      — modify  test only
```

## Order of work

### Task 1 — Register the root and answer it under `trusted`

**Files:**
- Create `crates/airsl/src/require/root.rs`.
- Modify `crates/airsl/src/require/mod.rs` and `crates/airsl/src/builder.rs`.
- Test `crates/airsl/src/engine.rs`.

**Change:**
- Add `register` as described in Architecture, with its registry key. `answer` is not added here:
  it lands in Task 2 with its first caller, because an uncalled function fails the gate's
  `dead_code` check.
- `install_modules` calls `register` with `policy.language()`.
- After this task, the `Full` surface answers `require("<root>")` through Lua's own `require`, as
  spec D1 describes.

**Proof:**
- `register_keeps_the_engine_s_table_in_the_registry`: after `register`, the named registry value
  is the same table (`rawequal`).
- `register_seeds_package_loaded_on_the_full_surface_only`: on a `Full` state,
  `package.loaded[name]` is the table. On `Restricted` and `Minimal` states, `register` returns
  `Ok` and `package` is `nil`.
- `register_overwrites_a_package_loaded_entry_of_the_same_name`: with root `utf8` on `Full`,
  `package.loaded.utf8` becomes the table (D6).
- In `engine.rs`:
  - `a_trusted_script_that_clears_package_loaded_gets_lua_s_own_lookup`: with `package.path` on a
    folder holding `airsstack.lua`, after `package.loaded.airsstack = nil` the result is the user
    file's table (D3).
  - `a_trusted_script_that_replaces_package_loaded_gets_its_replacement`: after
    `package.loaded.airsstack = mine`, `require("airsstack")` is `mine` (D3).
  - `a_trusted_require_of_the_root_ignores_a_second_argument`:
    `select('#', require('airsstack', 'x')) == 1`, and the value is the global.

**Done when:** `cargo make dod-crate airsl` passes and the Definition of Done is green. Commit
`feat(airsl): register the root table for require`.

### Task 2 — The confined and refusing `require` answer the root first

**Files:**
- Modify `crates/airsl/src/require/root.rs`, `crates/airsl/src/require/loader.rs`,
  `crates/airsl/src/require/disposition.rs` (the `Refused` doc only) and
  `crates/airsl/src/engine.rs`.
- Modify `crates/airsl/src/error.rs` (the `RequireDenied` doc only, `error.rs:304-308`).
- Test `crates/airsl/src/extension/loaded.rs`.

**Change:**
- Add `root::answer` as described in Architecture.
- `RequireLoader::new` takes the root alongside the directory, and `install` answers the root
  before `RequireTarget::new`. That happens after the existing `LuaString` conversion, so `nil`,
  tables and the no-argument call raise today's conversion error.
- `install_refusal` takes the root, reads the first argument as an `mlua::Value`, and returns the
  root for a Lua string equal to it.
- `set_require` passes `self.root.clone()` to both.
- `RequireDenied`'s doc says the call is refused for every name except the root table's (spec §5).
- Doc comments that become false in this task, each rewritten to say the refusal answers the root
  name and refuses every other call:
  - the `install_refusal` doc (old `require_loader.rs:148-151`, "looks at none of them");
  - the `RequireDisposition::Refused` variant doc (old `:59`, "refuses every call");
  - in the `//!` blocks of `require/mod.rs` and `require/loader.rs`, the sentence from old `:6-7`
    ("installs a function that refuses instead").

**Proof:**
- In `require/root.rs`:
  - `answer_matches_the_exact_root_name_only`: with root `airsstack`, `airsstack` gives `Some` of
    the registry table. `Airsstack`, `airsstack.fs`, ` airsstack`, `airsstack ` and `airsstac`
    each give `None`.
  - `answer_returns_the_registry_table_after_the_global_is_replaced`: after the global is set to
    `nil`, `answer` still returns the registered table.
- In `require/loader.rs`:
  - `the_refusing_require_ignores_its_argument` is renamed
    `the_refusing_require_refuses_every_name_but_the_root`. `require()`, `require({})`,
    `require('x')` and `require(1)` are each refused with
    ``require denied: `require` is not granted — no access is granted``.
    `require('airsstack')` returns the global table.
  - `a_confined_require_answers_the_root_before_looking_for_a_file`: `airsstack.lua` and
    `airsstack/init.lua` exist beside the script and each sets a global flag when run. The result
    is the root table, and neither flag is set.
- In `engine.rs`:
  - `a_custom_root_is_required_by_its_own_name_and_the_default_name_is_ordinary`: under root
    `agentstacks` and `confined`, `require('agentstacks')` is the global, and
    `require('airsstack')` raises `RequireNotFound`.
  - `a_user_file_named_after_the_root_is_never_loaded_by_require`: a user `airsstack.lua` is never
    run. This holds under `trusted` (with `package.path` on the folder), under `confined`, and
    under `pure` with the `require` member.
  - `require_returns_the_engine_s_table_after_the_global_is_replaced`: under `trusted` and under
    `confined`, after `airsstack = {}` the result is the original table. The script stashes the
    original in a local first.
  - `a_dispatched_handler_requires_the_root`: a script registers a handler with `ext.on` that
    returns `require(root)`. `Engine::dispatch` of that event yields a value equal to the root.
  - `a_pure_policy_gives_a_refusing_require_to_a_script_on_disk` is unchanged and passes.
- In `extension/loaded.rs`: `an_extension_requires_its_host_s_root_by_name`. An extension loaded
  under a host whose root is `myapp` gets the root table from `require("myapp")`.

**Done when:** `cargo make dod-crate airsl` passes and the Definition of Done is green. Commit
`feat(airsl): the confined and refusing require answer the root table`.

### Task 3 — A script from source gets a `require` that answers only the root

**Files:**
- Modify `crates/airsl/src/require/disposition.rs`, `crates/airsl/src/require/loader.rs`,
  `crates/airsl/src/engine.rs`, `crates/airsl/src/error.rs` and `crates/airsl/src/script.rs`.

**Change:**
- Rename `RequireDisposition::Absent` to `RootOnly` and update its doc. It is still chosen below
  `Full` when the member is granted and there is no directory.
- Add `Error::RequireNoDirectory { module: String }` beside `RequireNotFound`, with spec §5's
  doc, field doc and `#[error]` text verbatim.
- Add `RequireLoader::install_root_only`. Delete `RequireLoader::remove`.
- `set_require`'s `RootOnly` arm installs it. The doc at `engine.rs:274-278` changes: a script
  from source under a granting policy gets a `require` that answers only the root, and a refusing
  `require` still answers the root.
- The rustdoc of `Script::from_source` (`script.rs:32`) says its `require` answers only the root
  table's name, and points to `Script::from_file` or `with_root` for loading siblings.
- Doc comments that become false in this task, each rewritten to name the root-only `require` in
  place of "no `require`":
  - the `RequireDisposition` doc (old `require_loader.rs:41-49`: "granted with no directory gives
    no `require` at all", "a missing one");
  - the `decide` doc (old `:73-74`: "a script built from source gets none");
  - in the `//!` blocks of `require/mod.rs` and `require/loader.rs`, the responsibility from old
    `:12-13` ("removes the global"), which becomes "installs the `require` that answers only the
    root".

**Proof:**
- `error.rs`: `require_no_directory_names_the_module`. `module: "lib"` displays
  ``module `lib` cannot be loaded: the script has no directory to load modules from``.
- `require/disposition.rs`:
  `a_script_with_no_root_has_nothing_to_resolve_against_and_gets_no_require` is renamed
  `a_granted_script_with_no_directory_gets_a_require_that_answers_only_the_root` and asserts
  `RootOnly` on `Restricted` and `Minimal`.
- `require/loader.rs`:
  - `the_root_only_require_answers_the_root_and_refuses_other_names_for_want_of_a_directory`:
    `require('airsstack')` is the root. `require('lib')` and `require(1)` raise
    `RequireNoDirectory` with the exact text, for modules `lib` and `1`.
  - `the_root_only_require_rejects_an_invalid_target_as_the_confined_one_does`:
    `require('../x')` raises the same `InvalidName` message under `RootOnly` and under `Confined`.
  - `the_root_only_require_rejects_a_non_string_as_the_confined_one_does`: `require()`,
    `require(nil)` and `require({})` give the same message under both.
  - `a_root_require_returns_exactly_one_value`: `select('#', require('airsstack')) == 1` under
    `Confined`, `Refused` and `RootOnly`.
  - `a_root_require_ignores_arguments_after_the_first`: `require('airsstack', 'x')` is the root
    under `Confined`, `Refused` and `RootOnly`.
- `engine.rs`:
  - `a_script_from_source_has_no_require_at_all` is replaced by
    `a_script_from_source_requires_the_root_and_nothing_else`: `type(require)` is `"function"`,
    `require('airsstack')` is the global, and `require('lib')` raises `RequireNoDirectory`.
  - `require_returns_the_root_table_under_every_preset`: under `trusted`, `confined`, `pure`, and
    `pure` with the `require` member, from a file and from source,
    `rawequal(require('airsstack'), airsstack)` is true.

**Done when:** `cargo make dod-crate airsl` passes and the Definition of Done is green. Commit
`feat(airsl): a script from source can require the root table`.

---

## Risks

- **Riskiest step: Task 2's `Refused` change.** It overturns a 0.2.0 guarantee (spec §10). The
  renamed test keeps every other refusal's exact text pinned.
- `register` runs at build. An `EngineSetup` failure there stops `Engine::builder().build()` for
  every policy, so its stage text must stay as spec §4.1 gives it.
- Not done here: documentation beyond the rustdoc named above (plan 04), and the type file
  (plan 03).

## Proof

- `cargo make dod` is green.
- `cargo test -p airsl require::` and `cargo test -p airsl engine::` pass.
- Every test named in spec §8, apart from the `luals/render.rs` ones, exists under the name given
  there.
- `git grep -n "looks at none of them\|no \`require\` at all\|a missing one\|gets none\|removes the global\|is unavailable to it" -- crates/airsl/src`
  prints nothing. On the parent commit it prints six lines: `require_loader.rs:13`, `:49`, `:75`
  and `:150`, `script.rs:32` and `engine.rs:278`. Each is a passage this plan rewrites.
