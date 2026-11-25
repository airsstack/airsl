---
status: approved
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
