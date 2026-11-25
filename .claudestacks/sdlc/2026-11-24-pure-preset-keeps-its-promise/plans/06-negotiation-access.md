---
status: done
created: 2026-11-25
depends-on: [01]
built-on: d8c7bb7f05b4
---

# Negotiation takes host access from the ceiling Implementation Plan

**Goal:** Make a negotiated extension policy hold exactly the host-access members its ceiling holds.

**Architecture:** `negotiate` (`negotiate.rs:275`) builds its work grant set from
`GrantSet::sealed()`, then adds every member `ceiling.policy().grants().access()` yields. Before
this plan it built from `GrantSet::declared()` (`:279`). The fs, env and proc intersection, and the
surface and limits taken from the ceiling, are unchanged (spec §7). Manifests gain nothing.

**Tech Stack:** Rust 2024, cargo-make.

**Content authority:** spec §7, §11.1 (`negotiate.rs` row).

---

## Files that change

```
crates/airsl/src/extension/negotiate.rs   — modify: the base grant set; the `negotiate` doc; tests
```

## Order of work

### Task 1 — Build the negotiated grants on the ceiling's access

**Files:**
- Modify `crates/airsl/src/extension/negotiate.rs`:
  - the `Intersection`'s starting `grants` (`:279`);
  - the `negotiate` rustdoc gains a sentence saying host access comes from the ceiling;
  - tests.

**Change:** The starting grant set is `GrantSet::sealed()` with each member of
`ceiling.policy().grants().access()` added through `with_access`. Nothing else in `negotiate`
changes.

**Proof:**
- `a_pure_ceiling_negotiates_no_access`: `negotiate` against `Ceiling::new(Policy::pure()).unwrap()`
  (`Ceiling::new` returns `Result`, `extension/ceiling.rs:27`) gives a policy whose
  `grants().access()` is empty.
- `a_confined_ceiling_negotiates_every_member`: against `Ceiling::new(Policy::confined()).unwrap()`,
  all seven.
- `a_ceiling_with_stderr_and_require_passes_exactly_those`: against
  `Ceiling::new(Policy::pure().with_grants(GrantSet::sealed().with_access(HostAccess::Stderr).with_access(HostAccess::Require))).unwrap()`,
  exactly `stderr` and `require`.
- The presence-assertion test ending at `:591`, as plan 01 left it, still passes.
- The `ext_doctor` expectations set in plan 01 (`:351`, `:356`, `:509`, `:514`) still pass, because
  their ceiling is confined.

**Done when:** `cargo make dod` passes. Commit
`feat(airsl): negotiate host access from the ceiling`.

---

## Risks

- Under a confined ceiling, every extension output is identical before and after this plan. The
  only observable change is under a `pure`-based ceiling, which is what the tests pin.

## Proof

- `cargo make dod` is green.

## Review findings

- 🔵 negotiate.rs:274 — doc listed the HostAccess members by hand. Fixed: links [`HostAccess`]. `cargo make dod` exit 0 (2026-11-25, after all fixes): airsl 643 lib tests + 16 doctests, airsl-cli 86 tests; fmt, clippy -D warnings, rustdoc -D warnings green.
- 🔵 negotiate.rs:278 — local `access` held a GrantSet. Fixed: renamed `base`.
- Reviewer guard check: reverting to `declared()` fails the pure and stderr+require tests; bare `sealed()` fails the confined test. A trusted (unrestricted) ceiling cannot reach `negotiate` (`Ceiling::new` refuses it with `Error::CeilingUnbounded`).

## Deviations

- 2026-11-25 — Ledger taken with `grep -n '^### Task'` instead of a `task-briefer` ledger spawn; briefs were spawned per task as specified.
- 2026-11-25 — Plans 01–06 ran in one execute run on one working tree, with file-disjoint tasks from different plans coded concurrently; each plan was reviewed from a frozen patch snapshot of its own files.
- 2026-11-25 — Not committed: the execute instruction did not ask for commits, so the per-task commit messages the plan names were not created. All changes sit in the working tree.
- 2026-11-25 — Disk at 100% (242 MB free) mid-run; regenerable probe `target/` dirs in the session scratchpad were deleted to continue (614 MB free).
