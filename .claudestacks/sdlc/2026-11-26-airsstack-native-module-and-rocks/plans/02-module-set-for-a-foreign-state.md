---
status: done
created: 2026-11-26
built-on: af7c0dcc7368
---

# Module Set for a Foreign State Implementation Plan

**Goal:** A Rust caller can install every built-in module except `ext` into an `mlua::Lua` that
airsl did not create.

**Architecture:** Today the install loop is private to `builder.rs` (`install_modules`,
`builder.rs:240-262`). It moves onto `ModuleSet` as `ModuleSet::install(&self, lua, policy:
&Policy, root_table) -> Result<mlua::Table>`, which is `pub(crate)`. It builds the
`InstallContext`, creates the root table, installs each module into its own subtable in set order,
and returns the root. Two callers share that one loop:
- `install_modules` calls it with the engine's policy, then does the engine-only steps itself:
  `require::root::register` and the global.
- The new public `ModuleSet::open(&self, lua: &mlua::Lua, root_table: &RootTable) ->
  Result<mlua::Table>` calls it with `Policy::trusted()`. It sets no global, registers nothing for
  `require`, and arms no ceiling.

`InstallContext::new` stays `pub(crate)`, and the type keeps `#[non_exhaustive]`.
`modules::standalone()` sits beside `stdlib()` in `stdlib.rs` and returns the built-ins without
`ext`, in `stdlib()` order. `stdlib()` becomes `standalone()` plus `Ext::new()` last, so there is
still one list of built-ins. `modules/mod.rs` re-exports both functions and stays export-only.

**Tech Stack:** Rust 1.94, crate `airsl`, `mlua` 0.12.0 under airsl's default build, cargo-make.
Cargo runs with `CARGO_BUILD_JOBS=2` (`CLAUDE.md`).

**Content authority:** spec §2.1 (`builder.rs`, `modules/mod.rs`, `registry.rs`, `stdlib.rs`),
§2.2 (rows for `registry.rs` and `stdlib.rs`), §2.6, §2.8 (modularity Rule 2, static dispatch, doctests),
§3.1, §9.1 (`registry.rs` and `stdlib.rs` tests; `builder.rs` tests unchanged).

---

## Files that change

```
crates/airsl/src/modules/registry.rs   — modify  + ModuleSet::install (pub(crate)) and ModuleSet::open (pub); doc and tests
crates/airsl/src/builder.rs            — modify  install_modules delegates its loop to ModuleSet::install
crates/airsl/src/modules/stdlib.rs     — modify  + standalone(); stdlib() built from it; doc and tests
crates/airsl/src/modules/mod.rs        — modify  pub use stdlib::{standalone, stdlib}
```

## Order of work

### Task 1 — Move the install loop onto `ModuleSet`

**Files:**
- Modify `crates/airsl/src/modules/registry.rs`:
  - add `ModuleSet::install` (`pub(crate)`). It builds the `InstallContext` from the given policy and
    root table, creates the root table and installs each module in set order. Every
    `mlua::Error` maps to `Error::ModuleInstall { module: <root table name>, reason }`, exactly as
    `install_modules` does today (`builder.rs:246-249`). A module's own `install` error passes
    through unchanged. It returns the root table and sets nothing else.
  - add `install` and `open` to the module doc's `Responsibilities:` (`registry.rs:8-12`). Keep the
    four-part doc shape.
- Modify `crates/airsl/src/builder.rs`: `install_modules` keeps its signature and its doc. It calls
  `modules.install(lua, policy, root_table)`, then registers the returned table with
  `crate::require::root::register` and sets the global, in today's order: register before the
  global, because setting the global moves the table (`builder.rs:258-260`).

**Change:** The engine behaves the same: the same modules, in the same order, under the same
global, registered the same way for `require`. The engine and any future caller now share one
install loop.

**Proof:**
- `install_hands_modules_the_policy_it_is_given`: written first, and seen failing because
  `install` does not exist yet. Install `Fs` through `ModuleSet::install` into a fresh `mlua::Lua`
  under `Policy::confined()`. An `fs.read` of an existing temp file from the returned table must
  raise the confined refusal.
- Every existing test in `builder.rs`'s `tests` module, and the engine tests elsewhere in the crate,
  still pass unchanged. They now run through `ModuleSet::install`.

**Done when:** `cargo test -p airsl modules::registry` and `cargo test -p airsl builder` pass, and
`cargo make dod` is green. Commit `refactor(airsl): move the module install loop onto ModuleSet`.

### Task 2 — Split `standalone()` out of `stdlib()`

**Files:**
- Modify `crates/airsl/src/modules/stdlib.rs`:
  - add the public `standalone() -> Result<ModuleSet>`. It holds every insert `stdlib()` has today
    except `Ext`, in today's order (`stdlib.rs:24-34`).
  - make `stdlib()` call `standalone()` and then insert `Ext::new()` last (today `stdlib.rs:35`).
  - give `standalone` rustdoc: what it holds, that `ext` exists only inside airsl, an `# Errors`
    section naming `Error::DuplicateModule`, and a happy-path doctest asserting that `ext` is not
    among the names.
  - update the module doc's `Responsibilities:` to name both functions.
- Modify `crates/airsl/src/modules/mod.rs`: change `pub use stdlib::stdlib;` to
  `pub use stdlib::{standalone, stdlib};` (`modules/mod.rs:62`), the form in
  `probes/230-real-module-set-stock-lua/crates_airsl_src_modules_mod.rs.diff`.

**Change:** `airsl::modules::standalone()` is the eleven modules `json`, `path`, `fs`, `env`, `proc`,
`regex`, `hash`, `time`, `glob`, `stdio` and `hook`, in that order. `stdlib()` is unchanged for every
caller: the same twelve modules, in the same order. That keeps `doctor` output, the type file and
the engine the same.

**Proof:** Each test is written first and seen failing because `standalone` does not exist yet.
- `standalone_is_stdlib_without_ext_in_the_same_order`: `standalone().names()` equals
  `stdlib().names()` with `ext` removed, order kept.
- `stdlib_is_standalone_with_ext_last`: the last name of `stdlib()` is `ext`, and the names before it
  equal `standalone().names()`.
- The three existing `stdlib.rs` tests still pass.
- The `standalone` doctest passes.

**Done when:** `cargo test -p airsl modules::stdlib` and `cargo test -p airsl --doc` pass, and
`cargo make dod` is green. Commit `feat(airsl): add modules::standalone, the built-ins without ext`.

### Task 3 — Add `ModuleSet::open` for a state airsl did not create

**Files:**
- Modify `crates/airsl/src/modules/registry.rs`: add the public `ModuleSet::open(&self, lua:
  &mlua::Lua, root_table: &RootTable) -> Result<mlua::Table>`. It calls `install` with
  `Policy::trusted()` (`crates/airsl/src/sandbox/policy.rs:61`).

**Change:** `open` returns the root table holding one subtable per module, in set order, with
trusted authority. It does not set a global. It does not call `require::root::register`. It does not
touch `package.loaded`, arm a memory or instruction ceiling, or load, withhold or reshape any Lua
library. It takes no `&Policy`, because a `Policy`'s `LanguageSurface` and `ResourceLimits` cannot
be applied to a state airsl did not create (F6; §2.6 "Why this shape").

The rustdoc covers:
- the trusted authority;
- the absence of ceilings and of a global;
- that process-wide state (the `env` overlay, `time`'s origin) is shared with every other state in
  the process (§2.8 "DLL state isolation");
- an `# Errors` section naming `Error::ModuleInstall` and a module's own install error;
- a happy-path doctest under the default features. The doctest creates an `mlua::Lua` through
  `airsl::mlua`, opens `airsl::modules::standalone()` (Task 2) into it, as the native entry does
  (§2.6), and reads one module subtable. It does not open `stdlib()`, whose `ext` exists only inside
  airsl.

**Proof:** Each test is written first and seen failing because `open` does not exist yet.
- `open_returns_one_subtable_per_module_in_set_order`: the returned table has exactly one table per
  name of the set, and `ModuleSet::names()` gives the same order.
- `open_sets_no_global`: after `open` with the default root table, the global `airsstack` is nil.
- `open_leaves_package_loaded_and_the_require_registry_untouched`: after `open`,
  `package.loaded.airsstack` is nil. The named registry value that `require::root::register`
  writes (its key is `ROOT_KEY` in `crates/airsl/src/require/root.rs`) is also absent.
- `open_installs_with_trusted_authority`: through the opened `fs` subtable, `fs.read` of a temp file
  returns that file's content.
- `open_on_an_empty_set_returns_an_empty_table`: `ModuleSet::new().open(…)` returns a table with no
  keys.
- `open_propagates_a_module_install_error`: a test stub whose `install` returns an error makes
  `open` return that same error.
- The `open` doctest passes under `cargo test -p airsl --doc`.

**Done when:** `cargo test -p airsl modules::registry` and `cargo test -p airsl --doc` pass, and
`cargo make dod` is green. Commit `feat(airsl): install the built-in modules into a foreign Lua state`.

---

## Risks

- Task 1 is the riskiest step, because every engine installs through the moved loop. The existing
  builder and engine tests are the guard, together with the order rule: register first, global
  last.
- `open` makes `mlua::Lua` and `mlua::Table` part of a new public signature. Both are already part
  of the contract through `pub use mlua;` (`crates/airsl/src/lib.rs:65`; M-DONT-LEAK-TYPES), so
  this adds no new dependency exposure.
- Not done here: the native entry point that calls `standalone()?.open(…)`. Plan 04 owns it.

## Proof

- `cargo test -p airsl` and `cargo test -p airsl --doc` pass.
- `cargo make dod` is green.
- The grep `grep -rn "Box<dyn\|Arc<dyn" crates/airsl/src/modules/registry.rs` shows only the
  existing `// dyn:` seam (static-dispatch.md § DoD grep).

## Review findings

- Reviewer (25-reviewer-p02): SPEC COMPLIANT, blocking none; code 1🟡 6🔵, spec 0. DoD: `cargo make dod` → exit 0 (864+101+66+1 tests, 27 doctests); the `Box<dyn\|Arc<dyn` grep shows the same 4 seam hits as HEAD.
- 🟡 `crates/airsl/src/modules/registry.rs`:469 — "in set order" was promised by docs and the test name, but the test sorted the keys, so a reversed install loop passed every test — fixed — new test `open_installs_modules_in_set_order` (Recorder stub logging into an `Arc<Mutex<Vec>>`, asserts the log equals `set.names()`); with the loop reversed → "14 passed; 1 failed" (`open_installs_modules_in_set_order ... FAILED`), restored → `cargo test -p airsl modules::` 266 passed; `cargo make dod` → exit 0.
- 🔵 `crates/airsl/src/modules/registry.rs`:11 (and the type doc at :25) — `InstallContext` described as "what the engine tells a module", untrue for `ModuleSet::open` — fixed — reworded; `cargo make dod` → exit 0.
- 🔵 `crates/airsl/src/modules/stdlib.rs`:10 — Non-responsibilities said the engine installs the modules — fixed — now points at `ModuleSet` (install and open); `cargo make dod` → exit 0.
- 🔵 `crates/airsl/src/modules/stdlib.rs`:21 — doc line 113 columns and not a sentence — fixed — rewrapped and reworded; `cargo make dod` → exit 0.
- 🔵 `crates/airsl/src/modules/mod.rs`:27 (and the overview at :5) — `stdlib` entry did not mention `standalone`; overview said every module is installed under the `airsstack` global — fixed — overview says the engine publishes the global and `open` hands the table back; entry mentions `standalone`; `cargo make dod` → exit 0.
- 🔵 `crates/airsl/src/builder.rs`:246 — the `Error::ModuleInstall { module: root_table, .. }` mapping closure is written twice (here and in `ModuleSet::install`) and can drift — open — nit, left as is.
- 🔵 `crates/airsl/src/modules/env.rs`:8 (pre-existing, outside the diff) — module doc says "per-engine overlay" but `env::overlay()` is a process-wide `OnceLock` — open — pre-existing, outside this plan's files.

## Deviations

- 2026-11-26: Task 1's test asserts the real confined refusal text "outside the granted read roots" (from "fs.read denied: `…` is outside the granted read roots: none are granted"); the message has no word "confined". Reviewer accepted it: under trusted the read succeeds and `unwrap_err` panics.
- 2026-11-26: the open test's key check is unordered (sorted keys), because a Lua table has no order. Install order is proven instead by a separate test: the planned `open_returns_one_subtable_per_module_in_set_order` was split into `open_returns_one_subtable_per_module` (unordered keys) and `open_installs_modules_in_set_order` (Recorder stub; `Arc<Mutex<Vec>>` because `HostModule` is `Send + Sync`). Reversed loop → "14 passed; 1 failed"; restored → `cargo test -p airsl modules::` 266 passed.
- 2026-11-26: the check that `open` never registers the `require` root is `require::root::answer(…).is_err()`, because `ROOT_KEY` is private and the task's file scope is limited. Reviewer accepted it: only `register` writes that key.
- 2026-11-26: the Tasks 1–3 coder reported inline and wrote no handoff file. Its result: `cargo make dod` → exit 0; registry tests 14 passed; `--doc` 27 passed; stdlib tests 5 passed.
- 2026-11-26: No commit was made: the execute skill commits only when the instruction to execute says so; the plan's per-task commit messages remain for the author.
