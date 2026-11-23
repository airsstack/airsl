---
status: done
created: 2026-11-23
built-on: a4e85e09b81c
---

# Confined require module call Implementation Plan

**Goal:** Make the confined `require` run a module the way Lua 5.4's own `require` calls it, while still returning one value.

**Architecture:** All changes are in `crates/airsl/src/require_loader.rs` (spec §4.1). A new private
`compile` function holds the expression-first rule (spec §2.3). `run` computes the module's script
path once and uses it for both the chunk name and the second argument, then calls the compiled
function with `(name, path)` (spec §2.1). `load` returns the recorded value on a first load
(spec §2.2). Resolution, caching, cycle detection, `RequireDisposition` and every error are
untouched (spec §2.4, §5). No new file, type or dependency.

**Tech Stack:** Rust 2024, `mlua` 0.12.0 (`lua54, vendored, serialize, send`), `tempfile` in
tests, cargo-make.

**Content authority:** spec §2 (all), §3, §4, §5, §6.

---

## Files that change

```
crates/airsl/src/require_loader.rs   — modify: compile(); run() calls the module with (name, path); load() returns the recorded value; comments; module doc; tests
```

## Order of work

### Task 1 — Compile modules expression-first through a named function

**Files:**
- Modify `crates/airsl/src/require_loader.rs`:
  - add a private `compile` function;
  - make `run` use it;
  - add the tests below to `mod tests`.

**Change:**
- **`compile`** (new, private) takes the Lua state, the module source and the chunk name, and
  returns `mlua::Result<mlua::Function>`, compiled expression-first:
  1. Load `"return "` followed by the source under the chunk name. If it compiles, return that
     function.
  2. Otherwise load the source as written. Its compile error, not step 1's, is the error
     returned (spec §2.3).
  3. A comment beside the `"return "` prefix names its origin. It is `mlua::Chunk::eval`'s order,
     rebuilt here because `eval` passes no arguments and the expression step is private in mlua
     (spec §2.3, §4.3 `M-DOCUMENTED-MAGIC`).
- **`run`** replaces `.eval::<Value>()` with a call of the function `compile` returns. In this
  task it still passes **no arguments**, so behaviour is identical to 0.1.4. Task 2 adds them.
  Splitting it this way keeps `compile` in use, so there is no dead-code warning under `-D warnings`.
  Errors map with `Error::lua(target.as_str(), …)` as today.

**Proof:**

The `compile` unit tests call the private function directly. They are written first and seen
failing (the function does not exist), then passing. Each calls the compiled function with
`("pkg.mod", "/abs/m.lua")` and checks the result against spec §2.3's table:
- `compile_runs_a_block_that_returns_its_first_argument`: `return (...)` gives `"pkg.mod"`.
- `compile_passes_both_arguments`: `return select('#', ...)` gives `2`.
- `compile_runs_a_bare_vararg_expression`: `...` gives `"pkg.mod"`.
- `compile_runs_a_bare_table_expression`: `{ v = 'bare' }` gives a table whose `v` is `"bare"`.
- `compile_prefers_the_expression_reading`: `tostring(1)` gives `"1"`, not nil.
- `compile_reports_the_statement_parse_error`: `local x = \n` fails with a syntax error whose text
  contains `:2: unexpected symbol near <eof>`.
- `compile_keeps_only_the_first_of_several_return_values`: `return { v = 1 }, 'extra'`, converted
  to one `Value`, is the table.

The 37 existing tests matching `require` keep passing unchanged.

**Done when:** `cargo test -p airsl require_loader` passes and the Definition of Done
(`cargo make dod`) is green. Commit `refactor(airsl): compile required modules through one expression-first function`.

### Task 2 — Call a required module with its name and path

**Files:**
- Modify `crates/airsl/src/require_loader.rs`:
  - change `run` and `RequireLoader::install`'s closure (comment only);
  - add one `Responsibilities:` bullet to the module doc;
  - add the tests below to `mod tests`.

**Change:**
- **`run`**:
  - computes the module's script path once with `rules::native::to_script_string`;
  - uses that one string as the chunk name (prefixed with `@`) and as the second argument, so the
    two cannot differ (spec §4.1);
  - calls the compiled function with the target's `as_str()` and that path string, converting
    the result to one `Value`.
- **Why one value (comment).** At `RequireLoader::install`'s closure, where the Lua function's
  result is produced, a comment says why `require` returns only the module and never the path as
  a second value: a second value spills into multi-value positions, such as a table
  constructor's last field or a call's last argument, and breaks scripts that rely on one result.
  The module reads its own path from `...` instead (spec §2.2, §4.3 `doc-comment-discipline.md`
  row). The comment has no issue or spec references.
- **Module doc:** `Responsibilities:` gains one bullet: running a module, expression first, called
  with its name and path, the arguments Lua's own `require` passes. The four-part shape is kept
  (spec §4.1).

**Proof:**

Engine-level tests run through `Engine` and `Script::from_file` over `tempfile` fixtures, as the
existing tests do. The expected path is built the way `resolve` builds it (`canonicalize`,
`strip_verbatim`, `to_script_string`).

Written first and seen failing (the module gets no arguments yet), then passing:
- `a_required_module_receives_its_name_and_path_as_its_arguments`: inside the module,
  `select('#', ...) == 2`, the first argument is the target exactly as written, and the second
  equals the expected path.
- `a_module_reaches_its_sibling_through_its_own_name_from_either_root`:
  - Fixtures: the issue's three files, with the exact content of
    `probes/01-airsl-0.1.4-behaviour.sh` lines 8–10 (`main.lua`, `core/hooks/enforce.lua`,
    `core/hooks/lib/x.lua`), plus a script in `core/hooks/` that does
    `require("enforce")` and returns its function's result. Line 14's `from_hooks.lua` is the
    model.
  - Each run uses **its own fresh `Engine`** (spec §6).
  - From root `R` (entry `main.lua`, returning `f()`), the result is `lib=ok base=[core.hooks.]`.
  - From the `core/hooks/` script, the result is `lib=ok base=[]`.

Guards. Each passes before and after this task and pins spec §2.3's no-regression rows. To see
each fail, temporarily switch `compile` to block-first or to a plain block load, then revert:
- `a_module_written_as_a_bare_expression_still_loads`: `{ v = 'bare' }` through `require` returns
  a table with `v == "bare"`. A plain block load makes it fail.
- `a_module_that_is_both_an_expression_and_a_statement_returns_the_expression`: `tostring(1)`
  makes `require` return `"1"`. Block-first makes it fail.
- `a_syntax_error_in_a_module_reports_the_statement_parse`: a module `local x = \n` fails with
  `Error::Lua` whose text contains `:2: unexpected symbol near <eof>`. Block-first makes it fail.
- `a_module_returning_several_values_yields_only_the_first`: with `return { v = 1 }, 'extra'`,
  `select('#', require("m")) == 1` and the value is the table. Returning the call's whole result
  list from the closure makes it fail.

**Done when:** `cargo test -p airsl require_loader` passes and `cargo make dod` is green. Commit
`fix(airsl): pass a required module its name and path`.

### Task 3 — Pin the result shape and caching that must not change

**Files:**
- Test `crates/airsl/src/require_loader.rs`: add the tests below to `mod tests`.

**Change:** None in code. These tests lock in spec §2.2 and §2.4: `require` returns one value,
one file loads once per engine under whichever name reached it first, and a `false` module loads
once.

**Proof:**

Each test passes when written, on the code from Task 2. To see it fail, apply the breakage named
for it, then revert:
- `require_returns_exactly_one_value_on_the_first_load_and_on_a_cache_hit`:
  `select('#', require("m"))` is `1` on both calls, and both calls return the same table.
  - Breakage: the install closure returns `(value, path)`.
- `a_require_in_a_multi_value_position_contributes_one_value`:
  `{ require("a"), require("b") }` has 2 elements, and `table.insert({}, require("a"))` succeeds.
  - Breakage: the same `(value, path)` change.
- `a_module_returning_false_yields_false_and_runs_once`: both `require`s return `false`, and a
  global counter the module increments reads `1`.
  - Breakage: `load` treats a cached `false` as not loaded, as stock Lua does.
- `a_file_reached_under_two_names_runs_once_and_sees_the_first_name`: on one engine,
  `require("lib")` then `require("lib.init")` (fixture `lib/init.lua` records `...` in its table
  and bumps a counter). The module ran once, both calls return the same table, and the recorded
  name is `lib`.
  - Breakage: key the cache by the target name instead of the path.

**Done when:** `cargo test -p airsl require_loader` passes and `cargo make dod` is green. Commit
`test(airsl): pin require's single result and per-path caching`.

### Task 4 — Return true on the first require of a module that returns nothing

**Files:**
- Modify `crates/airsl/src/require_loader.rs`: change `load`'s success arm, and add a test to
  `mod tests`.

**Change:** On a successful first load, `load` returns the value it records in the cache, not
the module's raw value. For a module that returns nothing that is `true`, so the first `require`
yields `true`, as every later one already does and as stock Lua does (spec §2.2). The existing
comment on recording `true` is extended to say the same value is what the caller receives. Every
other result is unchanged, because the recorded value equals the raw value whenever the raw value
is not nil.

**Proof:**

Written first and seen failing (on the current code the first call returns nil), then passing:
- `a_module_returning_nothing_yields_true_on_its_first_require`: a module with no `return`. The
  first and second `require` both return `true`, one value each.

**Done when:** `cargo test -p airsl require_loader` passes and `cargo make dod` is green. Commit
`fix(airsl): return true from the first require of a module that returns nothing`.

---

## Risks

- **Task 2 is the riskiest step.** Building the chunk name and the path from two separate
  `to_script_string` calls would let them drift on Windows. Task 2 requires one string for both
  uses, and the Windows CI leg of `dod` is where a divergence would show.
- If `compile` keeps step 1's error instead of step 2's, every module with a syntax error reports
  a misleading `return`-prefixed parse error. `compile_reports_the_statement_parse_error` and
  `a_syntax_error_in_a_module_reports_the_statement_parse` catch this.
- The sibling test fails against correct code if it shares one engine between the two roots,
  because the cache keeps the first name. Task 2 requires a fresh engine per root.
- Not done here: the book, the CHANGELOG and the version bump (plans 02 and 03).

## Proof

- `cargo make dod` green on the workspace after Task 4.
- `cargo test -q -p airsl --lib require` shows the 37 pre-existing tests plus this plan's new ones,
  none failing.

## Review findings

- T1 🟡 statement-path `set_name` unguarded — fixed in T2: `compile_reports_the_statement_parse_error` asserts `/abs/m.lua:2:`; removing `set_name` from the statement branch fails only that test. `cargo test -p airsl require_loader` → 23 passed.
- T1 🔵 comment omits eval's bytecode branch — declined: modules are read with `read_to_string`, so bytecode never reaches `compile`.
- T1 🔵 `use mlua::Value;` for one test; test helper named `compile`; consts between tests — declined: style only.
- T1 🔵 `compile_keeps_only_the_first_of_several_return_values` guards mlua's conversion, not airsl code — declined: the plan names this test; the engine-level guard is `a_module_returning_several_values_yields_only_the_first`.
- T2 🔵 install-closure comment readable two ways — fixed: reworded ("Lua's own `require` also returns … this one returns only the module").
- T2 🔵 `to_script_string` called twice in `run` — fixed: computed once, reused by the `ScriptRead` error.
- T2 🔵 "the name a traceback shows" inexact for long chunk ids — fixed: "the chunk name and the path".
- T2 🔵 `resolve` comment omits the module's second argument — fixed.
- T2 🔵 several-values guard checked count and value on different calls — fixed: `local a, b = require('m')` checks both on the first call (`nil:1`).
- T3 🔵 identity check compared two cache hits — fixed: first-load value compared with a cache hit (`1|1|true`).
- T4: no findings.
- After fixes: `cargo test -q -p airsl --lib require` → `test result: ok. 55 passed; 0 failed` (37 pre-existing + 18 new); `cargo make dod` green (reviewer re-run, 571 + 86 + 9 tests, 0 warnings).

## Deviations

- 2026-11-23 — No commits made. The execute instruction did not say to commit, so the per-task commits the plan names are left to the author; the working tree holds all four tasks (patches per task: scratchpad `p01-after-t1.patch`, `p01-t2.patch`, `p01-t3.patch`, `p01-t4.patch`).
- 2026-11-23 — No `task-briefer` spawned: the plan was already in the orchestrator's context; each coder read its own `### Task N` section.
- 2026-11-23 — Tasks 3 and 4 were reviewed in one reviewer pass (same file, sequential tasks).
- 2026-11-23 — The T1 review's 🟡 was folded into the Task 2 coder brief rather than a separate fix spawn.
