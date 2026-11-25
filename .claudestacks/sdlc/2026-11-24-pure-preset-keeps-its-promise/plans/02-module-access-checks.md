---
status: done
created: 2026-11-25
depends-on: [01]
built-on: d8c7bb7f05b4
---

# Host modules check host access Implementation Plan

**Goal:** Make `stdio`, `hook`, `time` and `path` refuse each call whose host-access member the policy does not grant.

**Architecture:**
- A new `modules/access.rs` is the only place the check and refusal shapes A and B are written
  (spec §9, §10.2).
- It is declared `pub(crate) mod access;` in `modules/mod.rs`. Plan 05's `require_loader` sits
  outside `modules/`, and spec §10.3 requires that edge; a private module would fail with E0603.
- Each item lands in the task that first calls it, so no task leaves dead code under
  `-D warnings`.
- Each module captures a clone of `context.grants()` at install, as the guard users do
  (`glob.rs:99`, `hash.rs:73`), and calls `access` before acting (spec §5.1–§5.4).
- Every `Denied.operation` word is the Lua function's own name: `read`, `lines`, `write`, `error`,
  `isatty`, `payload`, `emit`, `context`, `now`, `monotonic`, `parse`, `absolute`. That is what the
  spec §9 texts require. Some existing I/O errors use a shared word instead (`stdio.rs:54`, `:124`;
  `hook.rs:149`), and they stay as they are.
- `ext.granted()` reports the members (spec §5.6).
- Arguments are still converted before the function body runs, so an argument error wins over a
  refusal (spec §5).

**Tech Stack:** Rust 2024, `mlua` 0.12.0, `jiff` 0.2.35, cargo-make.

**Content authority:** spec §5.1–§5.4, §5.6, §9 (shapes and the stdio, hook, time and path rows),
§10.1–§10.3 (modules rows, doc edits, the `require_loader` edge), §11.1 (`modules/access.rs`,
stdio/hook/time/path and `ext.rs` rows), §11.2 (the `time.rs:168` helper).

---

## Files that change

```
crates/airsl/src/modules/access.rs   — create: the host-access check and shapes A and B; tests
crates/airsl/src/modules/mod.rs      — modify: `pub(crate) mod access;`; the time and path doc bullets
crates/airsl/src/modules/stdio.rs    — modify: stdin/stdout/stderr checks; module doc; tests
crates/airsl/src/modules/hook.rs     — modify: stdin/stdout checks; module doc; stale test comment; tests
crates/airsl/src/modules/time.rs     — modify: clock check; zones check in parse; module doc; test helper; tests
crates/airsl/src/modules/path.rs     — modify: cwd check in absolute; module doc; stale test comment; tests
crates/airsl/src/modules/ext.rs      — modify: ext.granted().access; tests
```

## Order of work

### Task 1 — Write shape A and gate `stdio` and `hook` on the stream members

**Files:**
- Create `crates/airsl/src/modules/access.rs`, with:
  - a four-question module doc naming it as the one place a host-access refusal is written (spec
    §10.2);
  - the `#![expect(clippy::redundant_pub_crate, reason = "...")]` header that `guard.rs:14-17`
    carries;
  - tests.
- Modify `crates/airsl/src/modules/mod.rs`: add `pub(crate) mod access;`.
- Modify `crates/airsl/src/modules/stdio.rs`:
  - `install` captures the grants, and each function checks before acting;
  - the module doc drops "Needs no grant" and names the member each function needs;
  - tests.

  Two existing pieces stay as they are. The test helper `eval` (`stdio.rs:169-177`) is on
  `Policy::confined()`, which holds every member after plan 01. `stdio_is_present_even_under_a_pure_policy`
  (`stdio.rs:197-206`) asserts only presence.
- Modify `crates/airsl/src/modules/hook.rs`:
  - `install` captures the grants;
  - `payload`, `emit` and `context` check before acting;
  - the module doc names `stdin` and `stdout`;
  - the comment at `hook.rs:198-199` ("reachable from it without a grant") now says `confined`
    holds the stream members;
  - tests.

**Change:**
- `access::detail(grants: &GrantSet, member: HostAccess) -> String` builds shape A. It is
  `pub(crate)` because plan 05 uses it. The tail is `{granted}` (spec §9): either "the granted
  access is …" followed by `grants.access()` in `ALL` order, joined by ", ", or "no access is" when
  no member is granted.
- `access::check(grants, module: &'static str, operation: &'static str, member: HostAccess) ->
  Result<()>` returns `Ok(())` when `grants.allows(member)`. Otherwise it returns
  `Error::Denied { module, operation, detail: detail(grants, member) }`.
- Each function checks its member, per spec §5.1:
  - `stdio.read` and `stdio.lines` check `stdin`.
  - `stdio.write` checks `stdout`.
  - `stdio.error` checks `stderr`.
  - `stdio.isatty(s)` checks the member of the stream it names, using the existing stream-name
    mapping (`stdio.rs:140-151`).
  - `hook.payload` checks `stdin`, before calling `read_stdin`.
  - `hook.emit` and `hook.context` check `stdout`, before building the envelope.

  Each function passes its own name as `operation`.

**Proof:** Each test builds an engine on `Policy::pure()`, so nothing is granted. The refusal comes
first, so no test reads stdin or writes stdout.
- `shape_a_with_nothing_granted_says_no_access_is`: `detail(sealed, Stdout)` equals
  `` `stdout` is not granted — no access is ``.
- `shape_a_lists_the_granted_members_in_order`: with `stderr` and `require` granted (added in either
  order), the tail is `the granted access is stderr, require`.
- `check_passes_a_granted_member_and_an_unrestricted_set`: `check` returns `Ok` for `declared()` and
  for `unrestricted()`.
- `stdio_read_is_refused_without_stdin`: the spec §9 `stdio.read` text.
- `stdio_lines_is_refused_without_stdin`: ``stdio.lines denied: `stdin` is not granted — no access
  is``.
- `stdio_write_is_refused_without_stdout`: the spec §9 `stdio.write` text.
- `stdio_error_is_refused_without_stderr`: the spec §9 `stdio.error` text.
- `stdio_isatty_needs_the_member_of_the_stream_it_names`:
  - `isatty("stdin")` names `stdin`, and `isatty("stderr")` names `stderr`;
  - `isatty()` and `isatty("bogus")` name `stdout`;
  - with `stderr` granted, `isatty("stderr")` returns a boolean.
- `hook_payload_is_refused_without_stdin`: ``hook.payload denied: `stdin` is not granted — no
  access is``.
- `hook_emit_is_refused_without_stdout`: the spec §9 `hook.emit` text.
- `hook_context_is_refused_without_stdout`: ``hook.context denied: `stdout` is not granted — no
  access is``.

**Done when:** `cargo make dod-crate airsl` passes, and `cargo make dod` is green. Commit
`feat(airsl): gate stdio and hook on the stream members`.

### Task 2 — Add shape B and gate the clock and named zones in `time`

**Files:**
- Modify `crates/airsl/src/modules/access.rs`: add `Subject`, holding only `NamedZone` in this task;
  add `check_subject`; tests.
- Modify `crates/airsl/src/modules/mod.rs:18`: the `time` bullet drops "needing no authority" and
  names `clock` and `zones`.
- Modify `crates/airsl/src/modules/time.rs`:
  - `install` captures the grants;
  - `now` and `monotonic` check `clock`;
  - `parse` checks `zones`;
  - the module doc drops "Needs no authority" and names `clock` and `zones`;
  - the test helper `eval` (`time.rs:168`, today on `Policy::pure()`) builds on
    `Policy::pure().with_grants(GrantSet::sealed().with_access(HostAccess::Clock))`;
  - tests.

**Change:**
- `access::Subject<'a>` is a `pub(crate)` enum. In this task it has one variant,
  `NamedZone(&'a str)`, which renders as ``the named time zone `{name}` ``.
- `access::check_subject(grants, module, operation, member, subject: Subject<'_>) -> Result<()>`
  returns shape B when the member is not allowed:
  `{subject} needs `{member}`, which is not granted — {granted}`.
- `now` and `monotonic` call `check` for `clock` (spec §5.2).
- In `parse`, on the pattern path: after `strtime::parse` succeeds and before `to_zoned`, if
  `iana_time_zone()` is `Some(name)`, call `check_subject` with `zones` and
  `Subject::NamedZone(name)` (spec §5.3).
- The no-pattern path and `format` are unchanged.

**Proof:**
- `shape_b_names_the_zone`: with nothing granted, `check_subject` for `Zones` and
  `NamedZone("America/New_York")` gives the spec §9 `time.parse` text.
- `now_is_refused_without_the_clock`: under `Policy::pure()`, the spec §9 `time.now` text.
- `monotonic_is_refused_without_the_clock`: ``time.monotonic denied: `clock` is not granted — no
  access is``.
- `parse_of_a_named_zone_is_refused_without_zones`: `parse("2024-07-01 12:00 America/New_York",
  "%Y-%m-%d %H:%M %Q")` under `Policy::pure()` gives the spec §9 `time.parse` text.
- `parse_with_an_offset_needs_no_zones`: with `%z` and `-0400`, the result is `1719849600` under
  `Policy::pure()` (E9).
- `parse_with_both_an_offset_and_a_zone_needs_zones`: a `%z %Q` pattern is refused under
  `Policy::pure()` (E16).
- `parse_without_a_pattern_needs_no_zones`: an RFC 3339 text with a `[America/New_York]` annotation
  parses under `Policy::pure()` (E9).
- `parse_of_a_named_zone_is_not_refused_with_zones`: with `zones` granted, the `%Q` case above
  either returns a number or raises an error whose text does not contain ``needs `zones` ``. Every
  `time.parse` failure is already a `Denied` (`time.rs:61-67`), so the word `denied` cannot tell a
  tzdb failure from the refusal. The test does not depend on what any host's tzdb holds.

**Done when:** `cargo make dod-crate airsl` passes, and `cargo make dod` is green. Commit
`feat(airsl): gate the clock and named time zones`.

### Task 3 — Gate `path.absolute` on the cwd

**Files:**
- Modify `crates/airsl/src/modules/access.rs`: `Subject` gains `RelativePath`; a test.
- Modify `crates/airsl/src/modules/mod.rs:23`: the `path` bullet says `absolute` needs `cwd`.
- Modify `crates/airsl/src/modules/path.rs`:
  - `install` captures the grants, and `absolute` checks;
  - the module doc's "single exception is `absolute`" sentence says it needs `cwd`;
  - the comment at `path.rs:629` ("`path` needs no authority …") says every function stays present
    under `pure`;
  - tests.

  Two existing pieces stay as they are. The `eval` helper (`path.rs:302-304`) is on
  `Policy::confined()`. `every_function_is_reachable_under_a_pure_policy_too` (`path.rs:627-640`)
  asserts only presence.

**Change:**
- `Subject::RelativePath(&'a str)` renders as ``the relative path `{raw}` ``.
- When `std::path::Path::is_absolute` is false for the argument, `absolute` calls `check_subject`
  with `cwd` and `Subject::RelativePath(raw)` (spec §5.4).
- An absolute argument goes straight to the existing `fn absolute` (`path.rs:279`).

**Proof:**
- `shape_b_names_the_relative_path`: for `Cwd` and `RelativePath("p.lua")` with `stderr` granted,
  the text is ``the relative path `p.lua` needs `cwd`, which is not granted — the granted access is
  stderr``.
- `absolute_of_a_relative_path_is_refused_without_cwd`: `absolute("x")` under `Policy::pure()`
  gives the spec §9 `path.absolute` text.
- `absolute_of_an_empty_path_is_refused_without_cwd`: `""` is refused the same way (E18).
- `absolute_of_an_absolute_path_needs_no_cwd`: under `Policy::pure()`, `absolute` of a path built
  by `crate::test_support::abs` returns that path.
- `absolute_of_a_relative_path_works_with_cwd`: under `confined`, the existing result is unchanged.

**Done when:** `cargo make dod-crate airsl` passes, and `cargo make dod` is green. Commit
`feat(airsl): gate path.absolute on the working directory`.

### Task 4 — Report host access in `ext.granted()`

**Files:**
- Modify `crates/airsl/src/modules/ext.rs`: `granted_table` (`ext.rs:154-205`) gains the `access`
  sequence; tests.

**Change:** `granted_table` sets `access` to a sequence of `HostAccess::as_str` values from
`grants.access()`, in `ALL` order (spec §5.6). The other keys are untouched.

**Proof:**
- `granted_lists_access_members_in_order`: `table.concat(airsstack.ext.granted().access, ",")` is
  `stdin,stdout,stderr,clock,cwd,require,zones` under `confined` and under `trusted`, and is empty
  under `pure`.

**Done when:** `cargo make dod-crate airsl` passes, and `cargo make dod` is green. Commit
`feat(airsl): report host access in ext.granted()`.

---

## Risks

- **A wrong operation word.** The word must come from the Lua function's name, not from the older
  I/O errors. A test that checks only the member would not notice the mix-up, so each function's
  test pins its full text.
- **An under-granted helper.** Only the `time.rs:168` helper moves off bare `Policy::pure()`, and it
  grants just `clock`. A `time` test that silently relied on another member fails loudly.
- **CI's `cargo make examples`.** It is not part of `dod`, and it is red from this plan until plan
  09, because `text-toolkit` calls `time.monotonic` under `pure`. Do not push an intermediate
  state; the set lands together.

## Proof

- `cargo make dod` is green.
- The CLI check under `airsl run --policy pure` lives in plan 08, because until plan 07 the CLI
  folds its flags onto `declared()`.

## Review findings

- 🟡 registry.rs:104 — `HostModule::install` doc named `path` as needing no authority. Fixed: names `json`. `cargo make dod` exit 0 (2026-11-25, after all fixes): airsl 643 lib tests + 16 doctests, airsl-cli 86 tests; fmt, clippy -D warnings, rustdoc -D warnings green.
- 🟡 no test pinned "argument error wins over a refusal". Fixed: `an_argument_error_wins_over_a_refusal` (stdio.rs).
- 🔵 access.rs:10 "first shape" spec vocabulary — fixed; hook.rs `context` shadowing — fixed (`context_fn`); stdio.rs module doc logic + no-arg `isatty` — fixed; time.rs doc "pattern names a zone" → "text names a zone" — fixed; path.rs grants now in `Arc` — fixed.
- 🔵 duplicated `refusal` test helper (hook.rs, stdio.rs) — open, left duplicated (differing docs, no natural shared spot).
- 🔵 mod.rs stdio/hook bullets rewritten beyond the plan's list — accepted.
- Reviewer mutation runs: removing the `path.absolute` check, making it unconditional, removing the `time.parse` zone check, and mapping every `isatty` to stdout each fail a named test.

## Deviations

- 2026-11-25 — Ledger taken with `grep -n '^### Task'` instead of a `task-briefer` ledger spawn; briefs were spawned per task as specified.
- 2026-11-25 — Plans 01–06 ran in one execute run on one working tree, with file-disjoint tasks from different plans coded concurrently; each plan was reviewed from a frozen patch snapshot of its own files.
- 2026-11-25 — Not committed: the execute instruction did not ask for commits, so the per-task commit messages the plan names were not created. All changes sit in the working tree.
- 2026-11-25 — Disk at 100% (242 MB free) mid-run; regenerable probe `target/` dirs in the session scratchpad were deleted to continue (614 MB free).
- 2026-11-25 — Task 3 coder wrote tests and implementation before the first run (no observed red); the reviewer's mutation runs confirmed each new test fails with the check removed.
