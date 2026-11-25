---
status: done
created: 2026-11-25
built-on: d8c7bb7f05b4
---

# Minimal surface without output or clock-seeded randomness Implementation Plan

**Goal:** Close the `Minimal` surface's last routes to the host's stdout and clock.

**Architecture:** `language_surface.rs` declares what `Minimal` withholds and whether it seeds
(spec §4.1). `builder.rs` applies both after the libraries load, beside `withhold_unsafe_globals`
(`builder.rs:108-111`, `:130-149`), and before `ResourceLimits::apply` (`builder.rs:112`):

1. it removes `print`;
2. it calls Lua's own `math.randomseed(0)`;
3. it replaces `math.randomseed` with a Rust function that holds the original and refuses only the
   no-argument form (spec §4.2).

The wrapper is an `mlua` function that captures an `mlua::Function`; no `dyn` is involved.
`Restricted` and `Full` are untouched.

**Tech Stack:** Rust 2024, `mlua` 0.12.0 (Lua 5.4.8), cargo-make.

**Content authority:** spec §4.1, §4.2, §9 (`math.randomseed` and `print` rows), §10.1
(`language_surface.rs`, `builder.rs`), §11.1 (`language_surface.rs` / `builder.rs` row).

---

## Files that change

```
crates/airsl/src/sandbox/language_surface.rs   — modify: Minimal's withheld name and seeding predicate; Minimal doc; tests
crates/airsl/src/builder.rs                    — modify: apply print withholding, the seed and the randomseed wrapper; tests
```

## Order of work

### Task 1 — Withhold `print` on `Minimal`

**Files:**
- Modify `crates/airsl/src/sandbox/language_surface.rs`:
  - a `pub(crate)` constant naming the globals `Minimal` withholds (`print`), beside
    `CHUNK_LOADERS` (`:18-36`);
  - a `pub(crate) const fn` answering whether a surface withholds them;
  - the `Minimal` variant doc (`:58-64`) is corrected per spec §4.1;
  - tests.
- Modify `crates/airsl/src/builder.rs`: `build` removes those globals when the surface asks, right
  after `withhold_unsafe_globals`; tests.

**Change:** For `Minimal` only, every name in the new constant is set to `nil` in the globals
table. Failure maps to `Error::EngineSetup` with a stage name, as `withhold_unsafe_globals` does.

**Proof:**
- `a_pure_engine_withholds_print`: under `Policy::pure()`, `type(print)` is `nil`. Calling it
  raises an error whose text contains `attempt to call a nil value (global 'print')` (E15).
- `restricted_keeps_print_and_lua_seeding` (first half): under `confined` and `trusted`,
  `type(print)` is `function` (E2).
- `only_minimal_withholds_print`: the predicate is true for `Minimal` and false for the other two.

**Done when:** `cargo make dod-crate airsl` passes, and `cargo make dod` is green. Commit
`feat(airsl): withhold print on the minimal surface`.

### Task 2 — Seed `math.random` and refuse a seedless `randomseed` on `Minimal`

**Files:**
- Modify `crates/airsl/src/sandbox/language_surface.rs`: a `pub(crate)` seed constant whose comment
  says why `0` (spec §4.2; `M-DOCUMENTED-MAGIC`); the predicate from Task 1 also covers seeding;
  tests.
- Modify `crates/airsl/src/builder.rs`: a private function that seeds and wraps, called for
  `Minimal` before `ResourceLimits::apply`; tests.

**Change:**
- The builder reads `math.randomseed` from the `math` table and calls it with the seed constant.
- It then sets `math.randomseed` to a Rust function created with `create_function`, which takes
  `mlua::MultiValue`:
  - empty arguments → `Error::Denied { module: "math", operation: "randomseed", detail: "a seed is
    required — the no-argument form reads the clock" }`, converted with `mlua::Error::from`;
  - otherwise → call the captured original with the same values and return its `MultiValue`.
- Failures while seeding or wrapping map to `Error::EngineSetup`.

**Proof:**
- `minimal_random_is_the_same_sequence_on_every_fresh_engine`: two engines built from
  `Policy::pure()` return the same `math.random(1, 1000000)`, `275394` (E6, E7, E15).
- `minimal_random_continues_across_evaluations_on_one_engine`: on one engine, the second evaluation
  returns `909833`, not the first value again (E15).
- `minimal_refuses_randomseed_without_a_seed`: `math.randomseed()` raises the spec §9 text.
- `minimal_randomseed_with_a_seed_repeats`: `math.randomseed(42)` returns `42, 0`, and two draws
  after re-seeding with 42 are equal (E7).
- `minimal_randomseed_passes_lua_s_own_argument_error_through`: `math.randomseed(nil)` raises an
  error containing `bad argument #1 to '?' (number expected, got nil)` (E7).
- `restricted_keeps_print_and_lua_seeding` (second half, the same test extended): under
  `confined`, `math.randomseed()` succeeds and returns two integers (E8).

**Done when:** `cargo make dod-crate airsl` passes, and `cargo make dod` is green. Commit
`feat(airsl): make math.random repeatable on the minimal surface`.

---

## Risks

- **The instruction ceiling.** Seeding runs before `ResourceLimits::apply`, so the ceiling cannot
  count it. Placing it after would charge a script for the host's set-up.
- **Diagnostics on `Minimal`.** Argument errors to `math.randomseed` there now name `'?'` and carry
  the `runtime error: ` prefix. The author accepted this (spec §4.2).

## Proof

- `cargo make dod` is green.
- `minimal_random_is_the_same_sequence_on_every_fresh_engine` is the plan-wide proof. The CLI run of
  the same check is in plan 08.

## Review findings

- 🔵 language_surface.rs:77-79, :112 — doc overstated "last route to stdout" and a 120-col line. Fixed (route that needs no host module; rewrapped). `cargo make dod` exit 0 (2026-11-25, after all fixes): airsl 643 lib tests + 16 doctests, airsl-cli 86 tests; fmt, clippy -D warnings, rustdoc -D warnings green.
- 🔵 test renamed `only_minimal_withholds_print` → `only_minimal_withholds_print_and_seeds_random` (Task 2 extends the predicate) — accepted.
- 🔵 asserts restating constants — fixed (removed). Seeding-before-limits order — fixed: call-site comment says why, and that no test can observe a move. Helper placement in builder.rs tests — fixed.
- Reviewer reproduced 275394 / 909833, `42,0,true`, the randomseed refusal text and `bad argument #1 to '?' (number expected, got nil)`.
- CLI: `airsl run --policy pure` → `math.random(1,1000000)` = 275394; `math.randomseed()` → "math.randomseed denied: a seed is required — the no-argument form reads the clock"; `type(print)` = nil.

## Deviations

- 2026-11-25 — Ledger taken with `grep -n '^### Task'` instead of a `task-briefer` ledger spawn; briefs were spawned per task as specified.
- 2026-11-25 — Plans 01–06 ran in one execute run on one working tree, with file-disjoint tasks from different plans coded concurrently; each plan was reviewed from a frozen patch snapshot of its own files.
- 2026-11-25 — Not committed: the execute instruction did not ask for commits, so the per-task commit messages the plan names were not created. All changes sit in the working tree.
- 2026-11-25 — Disk at 100% (242 MB free) mid-run; regenerable probe `target/` dirs in the session scratchpad were deleted to continue (614 MB free).
- 2026-11-25 — Predicate named `closes_ambient_channels` (covers print, seeding and the randomseed wrapper); Task 1's test renamed `only_minimal_withholds_print_and_seeds_random`.
