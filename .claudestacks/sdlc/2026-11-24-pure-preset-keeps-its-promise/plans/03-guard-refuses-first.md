---
status: done
created: 2026-11-25
depends-on: [02]
built-on: d8c7bb7f05b4
---

# Filesystem guard refuses before resolving Implementation Plan

**Goal:** Make `PathGuard` refuse, before any filesystem access, a path that no root could allow or that needs the ungranted cwd.

**Architecture:** All changes are in `PathGuard::read` and `PathGuard::write` (`guard.rs:77`,
`:90`), the single funnel for `fs.*`, `hash.hash_file` and `glob.walk` (spec §5.5). Each check runs
in this order:

1. today's lexical `reject_unrepresentable` (`guard.rs:165-176`), unchanged and first;
2. rule 2 (the axis has no roots);
3. rule 1 (a relative path without `cwd`), built with `access::check_subject` from plan 02;
4. today's `resolve`, followed by the grant check.

An unrestricted grant set skips rules 2 and 1, as it skips the grant check today.

**Tech Stack:** Rust 2024, cargo-make.

**Content authority:** spec §5.5, §9 (guard rule rows), §11.1 (`guard.rs` row), §11.2 (last
paragraph), §14 (first two rows).

---

## Files that change

```
crates/airsl/src/modules/guard.rs   — modify: lexical step split out of resolve; rules 2 and 1; raw-path refusal; docs; tests
```

## Order of work

### Task 1 — Refuse a no-roots axis before touching the filesystem

**Files:**
- Modify `crates/airsl/src/modules/guard.rs`:
  - the lexical step becomes callable on its own, ahead of the rules;
  - `read` and `write` apply rule 2;
  - a refusal path that renders the raw path;
  - the module doc and the `resolve` doc state the new order;
  - tests.

**Change:**
- `read` and `write` first run the lexical check that `resolve` starts with today. It is moved into
  a private function that `resolve` also calls, so the order inside `resolve` is unchanged.
- Then, when the grant set is not unrestricted and the axis's root list is empty
  (`fs().read_roots()` or `fs().write_roots()`), they return `Error::Denied`. The detail is
  `` `{raw}` is outside the granted {read|write} roots: none are granted ``, with `raw` exactly as
  the script passed it.
- `deny` (`guard.rs:108-129`) keeps its wording for the resolved case. A sibling builds the
  raw-path refusal with the same sentence, so the two cannot drift; both render through `Access`'s
  word.

**Proof:**
- `no_roots_refuses_before_touching_the_filesystem`:
  - with no read roots, `read` of a symlink created in a temp dir, pointing at a second temp dir,
    returns a refusal naming the symlink path exactly as passed, not the target;
  - `read("/nonexistent/x")` names `/nonexistent/x`;
  - the error is `Error::Denied`, not `UncheckablePath`.
- `no_roots_refuses_writes_with_the_write_wording`: with no write roots, `write` of an absolute
  path names `write roots: none are granted`.
- `a_nul_path_is_still_uncheckable_before_either_rule`: with no roots, `read("a\0b")` is
  `Error::UncheckablePath`, and the message comes from `reject_unrepresentable`.
- The existing `a_refusal_with_no_roots_at_all_says_so` (`guard.rs:431-439`) still passes
  unchanged.

**Done when:** `cargo make dod-crate airsl` passes, and `cargo make dod` is green. Commit
`fix(airsl): refuse a path no root could allow before resolving it`.

### Task 2 — Refuse a relative path when the cwd is not granted

**Files:**
- Modify `crates/airsl/src/modules/guard.rs`: rule 1 in `read` and `write`, after rule 2; tests.

**Change:** When the grant set is not unrestricted, `cwd` is not granted, and `raw` is not absolute
by `std::path::Path::is_absolute`, `read` and `write` return
`access::check_subject(grants, module, operation, HostAccess::Cwd, Subject::RelativePath(raw))`'s
refusal before `resolve`. With `cwd` granted, a relative path resolves as today.

**Proof:**
- `a_relative_path_without_cwd_is_refused_before_resolution`: with a read root granted (a temp
  dir), `stderr` granted and `cwd` not granted, `read("p.lua")` returns exactly
  ``read denied: the relative path `p.lua` needs `cwd`, which is not granted — the granted access is
  stderr``, prefixed by the guard's module name, as in the spec §9 rule-1 row.
- `no_roots_wins_over_the_relative_rule`: with no roots and no `cwd`, `read("p.lua")` gives the
  rule-2 text `` `p.lua` is outside the granted read roots: none are granted ``.
- `a_relative_path_with_cwd_resolves_as_before`: with `cwd` granted and the temp dir as a read root,
  a relative path that resolves inside it is allowed, exactly as before.
- `an_unrestricted_set_skips_both_rules`: under `GrantSet::unrestricted()`, a relative path that
  exists resolves and is allowed.

**Done when:** `cargo make dod-crate airsl` passes, and `cargo make dod` is green. Commit
`fix(airsl): refuse a relative path when the working directory is not granted`.

---

## Risks

- **The variant change in §14.** Under no roots, inputs that used to fail inside `resolve` with
  `UncheckablePath` (cwd unreadable, climbs through a missing directory, nothing exists) now get
  `Error::Denied`. Existing tests that assert those variants call `PathGuard::resolve` directly
  (for example `guard.rs:442-447`), and that function is unchanged. Any test that reaches them
  through `read`/`write` with no roots will show in the `dod` run, and is changed to the §14
  behaviour.
- **Tests that build relative paths.** Tests under `confined` that build their guard on
  `declared()` hold `cwd` after plan 01, so their relative paths resolve as before.

## Proof

- `cargo make dod` is green.
- Under `cargo run -p airsl-cli -- run --policy pure`, `fs.read("/etc/hosts")` gives
  `` `/etc/hosts` is outside the granted read roots: none are granted ``, where 0.1.6 printed
  `/private/etc/hosts` (E5).

## Review findings

- 🔴 guard.rs:661-668 — `a_relative_path_with_cwd_resolves_as_before` compared a verbatim `\\?\` canonical cwd with `resolve`'s stripped path, failing on windows-latest. Fixed: uses `test_support::canonical`. Not run on Windows here. `cargo make dod` exit 0 (2026-11-25, after all fixes): airsl 643 lib tests + 16 doctests, airsl-cli 86 tests; fmt, clippy -D warnings, rustdoc -D warnings green.
- 🟡 guard.rs:570 — lexical-before-rule-1 unguarded when roots are granted. Fixed: `a_nul_relative_path_is_uncheckable_even_with_roots_and_no_cwd`; fails when `reject_lexically` runs after the cwd refusal (coder mutation run).
- 🔵 order hidden in `refuse_if_no_roots` — fixed: `refuse_before_resolving` runs lexical → rule 2 → rule 1, then `resolve`.
- 🔵 doc lines >100 cols, ambiguous `read` lead, history wording in `resolve` doc — fixed; six pre-existing >100-col lines later rewrapped too.
- Plan proof: `airsl run --policy pure` with `airsstack.fs.read("/etc/hosts")` → "fs.read denied: `/etc/hosts` is outside the granted read roots: none are granted" (rc=1).

## Deviations

- 2026-11-25 — Ledger taken with `grep -n '^### Task'` instead of a `task-briefer` ledger spawn; briefs were spawned per task as specified.
- 2026-11-25 — Plans 01–06 ran in one execute run on one working tree, with file-disjoint tasks from different plans coded concurrently; each plan was reviewed from a frozen patch snapshot of its own files.
- 2026-11-25 — Not committed: the execute instruction did not ask for commits, so the per-task commit messages the plan names were not created. All changes sit in the working tree.
- 2026-11-25 — Disk at 100% (242 MB free) mid-run; regenerable probe `target/` dirs in the session scratchpad were deleted to continue (614 MB free).
- 2026-11-25 — `a_relative_path_with_cwd_resolves_as_before` uses the canonical process cwd as the read root, not a temp dir: the process cwd is shared by all tests and cannot be changed safely. The temp-dir case is covered by `a_relative_path_with_cwd_is_still_checked_against_the_roots`.
