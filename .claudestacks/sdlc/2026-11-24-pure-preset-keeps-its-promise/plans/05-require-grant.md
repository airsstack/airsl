---
status: done
created: 2026-11-25
depends-on: [02]
built-on: d8c7bb7f05b4
---

# require follows the require grant Implementation Plan

**Goal:** Make the `require` a script gets below `Full` depend on the `require` host-access member instead of on the surface alone.

**Architecture:**
- `RequireDisposition::decide` (`require_loader.rs:71-77`) also takes the grant set, and gains a
  fourth disposition, `Refused` (spec §6).
- `RequireLoader` gains an installer for a refusing stub. The stub returns the new
  `Error::RequireDenied`, whose detail is `access::detail(grants, HostAccess::Require)` from plan 02 (shape A).
- `Engine::set_require` (`engine.rs:280-289`) passes `self.policy.grants()` and handles the new arm.
- Containment, caching and cycle detection are unchanged.
- The one new dependency edge is `require_loader` → `modules::access` (spec §10.3).

**Tech Stack:** Rust 2024, `mlua` 0.12.0, `tempfile` in tests, cargo-make.

**Content authority:** spec §6, §9 (`require` row), §10.1–§10.3 (`require_loader.rs`,
`engine.rs`, `error.rs` rows), §11.1 (`require_loader.rs` row).

---

## Files that change

```
crates/airsl/src/error.rs            — modify: Error::RequireDenied; test
crates/airsl/src/require_loader.rs   — modify: decide(surface, root, grants); Refused; refusing stub; docs; tests
crates/airsl/src/engine.rs           — modify: set_require passes the grants and installs the stub; tests
```

## Order of work

### Task 1 — Add `Error::RequireDenied`

**Files:**
- Modify `crates/airsl/src/error.rs`:
  - a documented variant beside `RequireEscape` and `RequireCycle` (`error.rs:216-232`), with
    `#[error("require denied: {detail}")]` and one field, `detail: String`;
  - a test in `mod tests`.

**Change:** The new variant only. Nothing raises it yet. It is public, and `Error` is already
`#[non_exhaustive]`.

**Proof:**
- `require_denied_names_require_and_the_detail`: `Error::RequireDenied { detail:
  "`require` is not granted — no access is".into() }.to_string()` equals
  ``require denied: `require` is not granted — no access is``.

**Done when:** `cargo make dod-crate airsl` passes, and `cargo make dod` is green. Commit
`feat(airsl): add Error::RequireDenied`.

### Task 2 — Decide `require` from the grant and install the refusing stub

**Files:**
- Modify `crates/airsl/src/require_loader.rs`:
  - `RequireDisposition` gains `Refused`;
  - `decide` takes `&GrantSet`;
  - its doc (`:57-70`) and the enum doc (`:38-45`) are rewritten to the four cases;
  - `RequireLoader` gains a function that installs the refusing stub;
  - the module doc's Responsibilities list mentions it;
  - tests.
- Modify `crates/airsl/src/engine.rs`:
  - `set_require` calls `decide(self.policy.language(), script.root(), self.policy.grants())`;
  - its doc (`engine.rs:271-275`) now says the `require` follows the surface, the `require` grant
    and the root, and that an ungranted one refuses;
  - on `Refused`, it installs the stub with the shape-A detail for `require`;
  - tests.

**Change:** `decide` stays an exhaustive match, with no wildcard, and stays a `const fn` (`allows`
is `const` after plan 01). It returns:

| Surface | `require` granted | Script has a root | Disposition |
|---|---|---|---|
| `Full` | either | either | `Native` |
| `Restricted` / `Minimal` | no | either | `Refused` |
| `Restricted` / `Minimal` | yes | yes | `Confined(root)` |
| `Restricted` / `Minimal` | yes | no | `Absent` |

The stub is created with `create_function` taking `mlua::MultiValue`, so no argument conversion can
fail first. It returns `Error::RequireDenied` with the detail fixed when it was installed. The
module cache in the registry is not touched (spec §6).

**Proof:**
- Existing tests in `require_loader.rs` change:
  - `only_the_restricted_surface_gets_a_confined_require` (`:307`) becomes
    `a_granted_require_is_confined_on_restricted_and_minimal`. With `declared()`, both surfaces
    with a root give `Confined`.
  - `a_script_with_no_root_has_nothing_to_resolve_against_and_gets_no_require` (`:337`) passes
    `declared()`, and still expects `Absent`.
  - `the_full_surface_keeps_lua_s_own_require_whether_or_not_the_script_has_a_root` (`:324`)
    passes both `sealed()` and `declared()`, and expects `Native` for each.
- `an_ungranted_require_is_refused_on_restricted_and_minimal`: with `sealed()`, both surfaces give
  `Refused`, with a root and without one.
- `the_refusing_require_ignores_its_argument`: under `Policy::pure()`, `require()`, `require({})` and
  `require("x")` all raise ``require denied: `require` is not granted — no access is``.
- `a_pure_script_with_require_granted_loads_its_siblings`: a temp-dir script under
  `Policy::pure().with_grants(GrantSet::sealed().with_access(HostAccess::Require))` requires a
  sibling returning `42` and gets `42`.
- `a_pure_script_with_require_granted_cannot_load_outside_its_folder`: under the same policy, a
  file script running `return require('../secrets')` fails with an error containing
  `require target`. That is the same input and assertion as the existing `confined` test
  `a_require_that_escapes_the_root_is_refused` (`engine.rs:776-784`).
- `a_pure_script_with_require_granted_cannot_follow_a_symlink_out_of_its_folder`: unix only, mirroring
  `a_symlink_out_of_the_root_is_refused` (`require_loader.rs:376`) under the same `pure` +
  `require` policy. It raises `RequireEscape`, the containment spec §6 and §11.1 name.
- In `engine.rs`, `a_pure_policy_gives_no_require_even_to_a_script_on_disk` (`:811`) becomes
  `a_pure_policy_gives_a_refusing_require_to_a_script_on_disk`. `type(require)` is `function`, and
  calling it raises `RequireDenied`.
- `a_confined_policy_with_sealed_grants_refuses_require`:
  `Policy::confined().with_grants(GrantSet::sealed())` with a file script gets the refusal.
- The existing `confined` tests (`engine.rs:692-809`) pass unchanged.

**Done when:** `cargo make dod-crate airsl` passes, and `cargo make dod` is green. Commit
`feat(airsl): decide require from the require grant`.

---

## Risks

- **`decide` staying `const`.** This depends on `GrantSet::allows` being `const fn` (plan 01,
  Task 2). If it could not be, `decide` drops `const`. It is crate-private, so nothing public
  changes.

## Proof

- `cargo make dod` is green.
- The CLI run of the same check is in plan 08, because until plan 07 the CLI folds its flags onto
  `declared()`, and `probes/require.lua` uses `print`, which plan 04 withholds.

## Review findings

- 🟡 require_loader.rs:43-48 — `RequireDisposition` doc stale ("Minimal is entitled to none"). Fixed: documents Full→Native; below Full: ungranted→Refused, granted+root→Confined, granted+no root→Absent. `cargo make dod` exit 0 (2026-11-25, after all fixes): airsl 643 lib tests + 16 doctests, airsl-cli 86 tests; fmt, clippy -D warnings, rustdoc -D warnings green.
- 🟡 require_loader.rs:369-371 — stale test comment. Fixed.
- 🔵 engine.rs:277 wording — fixed; duplicated `raised()` test helper — fixed (moved to `test_support`).
- CLI check under `--policy pure` deferred to plan 08 as the plan states: until plan 07 the CLI folds flags onto `declared()`, so `airsl run --policy pure` still gets a confined `require` (observed: "module `x` not found under `<dir>`").

## Deviations

- 2026-11-25 — Ledger taken with `grep -n '^### Task'` instead of a `task-briefer` ledger spawn; briefs were spawned per task as specified.
- 2026-11-25 — Plans 01–06 ran in one execute run on one working tree, with file-disjoint tasks from different plans coded concurrently; each plan was reviewed from a frozen patch snapshot of its own files.
- 2026-11-25 — Not committed: the execute instruction did not ask for commits, so the per-task commit messages the plan names were not created. All changes sit in the working tree.
- 2026-11-25 — Disk at 100% (242 MB free) mid-run; regenerable probe `target/` dirs in the session scratchpad were deleted to continue (614 MB free).
- 2026-11-25 — A refusal reaches the caller as `Error::Lua` wrapping `Error::RequireDenied` / `Error::RequireEscape` (as every host refusal does); tests assert by downcasting the source.
- 2026-11-25 — `crates/airsl/examples/multi-file-project/which_require.lua:13` comment ("three possible globals") may be stale now `pure` has a refusing `require`; left for plan 09.
