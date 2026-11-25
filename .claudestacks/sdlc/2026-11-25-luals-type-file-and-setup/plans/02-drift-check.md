---
status: approved
created: 2026-11-25
depends-on: [01]
built-on: 044432888b01
---

# Drift Check Implementation Plan

**Goal:** Every `HostModule` must describe its surface, and `luals::verify` fails the gate when the description and the installed keys disagree.

**Architecture:**
- `HostModule` gains a required `describe(&self) -> Result<ModuleDescription>` (§3.1). Every implementor in the workspace changes in the same commit, because a required method does not compile until all of them do.
- `luals/verify.rs` installs one module into a fresh `mlua::Lua::new()` table, using `InstallContext::new(&Policy::trusted(), root)`. That constructor is `pub(crate)` (`crates/airsl/src/modules/registry.rs:40`).
- `verify` compares the raw-walked keys with the described members and builds a structured `Drift`.
- `verify.rs` also owns a `pub(crate)` helper that calls `describe()` and wraps a failure as `DescribeFailed`. `render` (plan 03) reuses it.
- `luals/mod.rs` is export-only.

**Tech Stack:** Rust 1.94, crate `airsl`, `mlua` 0.12.0 (via `airsl::mlua`), cargo-make.

**Content authority:** spec §2, §3.1, §3.4, §5, §8 (`DescribeFailed`, `DescriptionDrift`), §9 (the `verify.rs` rows), §10 (`luals/verify.rs` tests, the per-module test, the stub list), §11 (the `verify` doctest).

---

## Files that change

```
crates/airsl/src/luals/mod.rs                    — create  export-only: mod verify; pub use verify, Drift
crates/airsl/src/luals/verify.rs                 — create  verify(), Drift, the describe-or-DescribeFailed helper
crates/airsl/src/lib.rs                          — modify  pub mod luals
crates/airsl/src/error.rs                        — modify  + DescribeFailed, + DescriptionDrift
crates/airsl/src/modules/registry.rs             — modify  required describe on HostModule; Greeter doctest and Stub implement it
crates/airsl/src/modules/env.rs                  — modify  describe() + its_description_matches_what_it_installs
crates/airsl/src/modules/ext.rs                  — modify  describe() with Granted records + test
crates/airsl/src/modules/fs.rs                   — modify  describe() with Stat record + test
crates/airsl/src/modules/glob.rs                 — modify  describe() + test
crates/airsl/src/modules/hash.rs                 — modify  describe() + test
crates/airsl/src/modules/hook.rs                 — modify  describe() + test
crates/airsl/src/modules/json.rs                 — modify  describe() + test
crates/airsl/src/modules/path.rs                 — modify  describe() (variadic join) + test
crates/airsl/src/modules/proc.rs                 — modify  describe() with Output record + test
crates/airsl/src/modules/regex.rs                — modify  describe() with Compiled record + test
crates/airsl/src/modules/stdio.rs                — modify  describe() + test
crates/airsl/src/modules/time.rs                 — modify  describe() + test
crates/airsl/src/engine.rs                       — modify  test stubs Boomerang, EvalBoomerang, Poisoner implement describe
crates/airsl/src/extension/host.rs               — modify  test stub Probe implements describe
crates/airsl/examples/custom-module/main.rs      — modify  Metrics implements describe
```

## Order of work

### Task 1 — Make `describe` required and check every module against it

**Files:**
- Create `crates/airsl/src/luals/verify.rs`.
- Create `crates/airsl/src/luals/mod.rs`.
- Modify `crates/airsl/src/lib.rs`.
- Modify `crates/airsl/src/error.rs`.
- Modify `crates/airsl/src/modules/registry.rs`.
- Modify each of the 12 built-in module files listed above.
- Modify `crates/airsl/src/engine.rs` and `crates/airsl/src/extension/host.rs`.
- Modify `crates/airsl/examples/custom-module/main.rs`.

**Change:**

The trait, the errors and `verify`:
- `HostModule::describe` is required. Its rustdoc is the §3.1 text, with an `# Errors` section naming what `ModuleDescriptionBuilder::build` returns.
- `Error::DescribeFailed { module: String, source: Box<Error> }` displays ``cannot describe host module `{module}`: {source}``.
- `Error::DescriptionDrift { module: String, drift: Drift }` displays ``host module `{module}` does not match its description: {drift}``.
- `Error::VerifyFailed { module: String, source: Box<mlua::Error> }` displays ``cannot verify host module `{module}`: {source}`` (D24). It wraps every `mlua::Error` from `verify`'s own calls: creating the table, iterating it, `Table::get` (whose `__index` may raise), and converting a key to text.
- `verify(module: &dyn HostModule, root: &RootTable) -> Result<()>` follows §5 steps 1–3:
  1. Describe the module, or return `DescribeFailed`.
  2. Install it into a fresh state. An install error is returned unchanged.
  3. Collect undescribed keys with a raw `Table::pairs` walk, missing names with `Table::get`, and the not-a-function and not-a-field kinds.
  - "Described names" are the members only. Record names are never expected on the table.
  - Key display text follows §5: a string as itself (non-UTF-8 through `String::from_utf8_lossy`), a number through `Value::to_string`, and any other key as `<{type_name}>`.
  - `{t}` is `Value::type_name()`.
- `Drift` holds the four sorted lists behind the §5 accessors `undescribed()`, `missing()`, `not_a_function()` (`(name, type)` pairs) and `not_a_field()`. It derives `Debug`, `Clone`, `PartialEq` and `Eq`. Its `Display` follows the §5 table: groups in order, names sorted by displayed text in byte order and backticked, singular and plural wording, parts joined with `; `, and empty groups omitted.
- Add a rustdoc doctest on `verify` that checks a one-function host module.

The implementors:
- **Each of the 12 built-ins:** `describe` lists every key its `install` sets, with the parameters, optional parameters, variadic and returns from `probes/surface-inventory.md`, and the record, map, array and `any` types fixed in §3.4.
  - `fs` declares record `Stat`.
  - `regex` declares `Compiled` with seven `fun(...)` fields.
  - `proc` declares `Output`.
  - `ext` declares `Granted`, `GrantedFs`, `GrantedProc` and `GrantedLimits`.
  - `path.join` is the only variadic.
  - `ext.on`'s handler is `fun(payload: any): any` (§3.1).
  - Docs are one-line English sentences. Parameter and field docs contain no line break, and no doc line starts with `@`.
  - No doc names the root table (`airsstack`), because the same descriptions render under any root. Plan 03's `a_host_root_names_the_file_and_classes` asserts it.
- **The test stubs and the example** (`Boomerang`, `EvalBoomerang`, `Poisoner`, `Probe`, `Stub`, the `Greeter` doctest in `registry.rs`, and `Metrics` in the example): each one's `describe` matches exactly what its `install` sets.

**Proof:**
- `luals/verify.rs`:
  - `a_matching_module_passes`: a stub that installs and describes `f` returns `Ok`.
  - `an_undescribed_key_is_drift`: a stub that installs `f` and `g` but describes only `f` displays ``host module `stub` does not match its description: installs `g` without describing it``.
  - `a_missing_key_is_drift`: describing `f` and `g` while installing `f` displays ``describes `g` but does not install it``.
  - `a_member_served_through_index_is_not_missing`: a stub that serves a described function through a metatable `__index` passes.
  - `keys_seen_only_through_pairs_are_not_walked`: a stub whose metatable `__pairs` yields an extra key passes.
  - `a_field_installed_as_a_function_is_drift`: the display contains ``describes `x` as a field but installs a function``.
  - `a_function_installed_as_a_value_is_drift`: installing `f = 1` displays ``describes `f` as a function but installs a value of type `integer` ``.
  - `a_table_key_is_shown_by_its_type`: a table-keyed entry is reported as ``installs `<table>` without describing it``.
  - `a_function_key_is_shown_by_its_type`: a function-keyed entry is reported as ``installs `<function>` without describing it``.
  - `a_non_utf8_key_is_shown_lossily`: a key made of the bytes `a\xFFb` is reported as ``installs `a\u{FFFD}b` without describing it``.
  - `a_vm_failure_during_verify_names_the_module`: a stub whose table's `__index` raises `boom` while a described name is read gives an error whose display starts with ``cannot verify host module `stub`: ``.
  - `a_module_that_installs_nothing_passes`: a stub that installs and describes nothing returns `Ok`.
  - `drift_display_orders_and_joins_parts`: a `Drift` with two undescribed keys, one missing key, one not-a-function and one not-a-field displays the four groups in §5 order, joined with `; `, using plural wording for the two-name group and names in byte order.
  - `a_describe_error_names_the_module`: a stub whose `describe` returns `Error::InvalidDescription { reason: "x".into() }` displays exactly ``cannot describe host module `stub`: invalid module description: x``.
  - `a_described_record_is_not_reported_missing`: a stub that describes a record and one installed function passes.
- **Each of the 12 built-in module files:** `its_description_matches_what_it_installs` calls `verify` with that file's module built by its `new()` (as `crates/airsl/src/modules/stdlib.rs:24-35` builds them) and `RootTable::default()` and asserts `Ok`. These 12 tests are the drift gate (D2).
- The existing tests of `engine.rs`, `extension/host.rs` and `registry.rs` pass unchanged with the stubs' new `describe`.

**Done when:**
- `cargo test -p airsl luals::verify` passes.
- `cargo test -p airsl its_description_matches_what_it_installs` passes 12 tests.
- `cargo run -p airsl --example custom-module` exits 0.
- `cargo make dod` is green.

Commit `feat(airsl)!: require HostModule::describe and verify it against install`, with a body that says implementors must add `describe`.

---

## Risks

- **This is the riskiest step in the set, and a large single commit.** A required trait method breaks every implementor at once, so the trait change, the 12 built-ins, the 5 test stubs, the `Greeter` doctest, the example and `verify` cannot be split into separately green commits. Writing `verify` and its stub tests first inside the task gives the coder a red-to-green loop before the 12 built-ins are filled in.
- **Descriptions can be wrong about types without failing the gate.** `verify` checks names and kinds only (spec §13). The types come from `surface-inventory.md` and §3.4. Plan 06's acceptance check exercises them in LuaLS.
- **The breaking change reaches downstream hosts.** The commit is marked `!`. The book and README updates are plan 07.

## Proof

- `cargo make dod` is green, which includes the 12 per-module drift tests.
- `git grep -n "impl HostModule for" -- crates` lists only types that now implement `describe`. The build would fail otherwise.
