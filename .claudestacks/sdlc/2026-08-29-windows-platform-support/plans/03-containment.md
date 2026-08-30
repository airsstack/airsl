---
status: done
created: 2026-08-30
depends-on: [02]
---

# Containment Implementation Plan

**Goal:** Close the verbatim containment hole and route all five `canonicalize` sites through one
rule.

**Architecture:** `PathGuard::resolve` stays in `modules/guard.rs` and gains three changes —
reject unrepresentable spellings first, strip the verbatim prefix, return `ResolvedPath`. The four
other sites that canonicalise delegate their comparison to `paths::containment` and apply
`strip_verbatim`. This is the security-critical plan of the chain; it is sequenced before the
cosmetic vocabulary work deliberately.

**Tech Stack:** Rust 1.94, `std::path`, `std::fs::canonicalize`.

**Content authority:** spec §3 in full — §3.1 resolution, §3.2 the hole, §3.3 root resolution,
§3.4 the accepted platform difference, §3.5 the five sites, §3.6 where case matters.

---

## File structure

```
crates/airsl/src/modules/guard.rs        — modify  resolve: reject, strip, return ResolvedPath
crates/airsl/src/sandbox/grants.rs       — modify  resolve_root strips; contains_any delegates
crates/airsl/src/require_loader.rs       — modify  delegate comparison; strip before returning
crates/airsl/src/extension/manifest.rs   — modify  validate_entry delegates comparison
crates/airsl/src/extension/loaded.rs     — modify  recheck_entry delegates + strips before return
```

Every task verifies with `cargo make dod-crate airsl`, and the security tasks additionally need
the Windows CI leg from plan 01 to prove the filesystem-truth half.

### Task 1 — Reject unrepresentable spellings at the guard

**Files:**
- Modify `crates/airsl/src/modules/guard.rs`

**Steps:**

1. Write the failing test first. On Windows,
   `\\?\C:\<root>\a/../../Windows\System32\x` must be refused. Today it is **approved** — the
   `..` hides inside a single `Component::Normal` because `/` is not a separator in a verbatim
   path, so the `ParentDir` arm at `guard.rs:168-173` never fires. This is spec §3.2, and it is
   the reason this plan exists.
2. Add a second test: a device name (`CON`, `NUL`) is refused on Windows with a message naming the
   spelling, not the current misleading "no part of it exists" from the `_ =>` arm at `:176-181`.
3. Call `paths::rules::native::reject_unrepresentable(raw)?` as the **first** statement of
   `resolve`, before `std::path::absolute` at `:146`. Map `Unrepresentable` onto
   `Error::UncheckablePath` with a reason naming the spelling.
4. Verify:
   ```
   $ cargo test -p airsl modules::guard
   ```
   Expected: green on macOS; the Windows-only assertions are `#[cfg(windows)]` and prove out on
   the CI leg.

### Task 2 — Strip verbatim and return `ResolvedPath`

**Files:**
- Modify `crates/airsl/src/modules/guard.rs`

**Steps:**

1. Apply `paths::rules::native::strip_verbatim` to the `canonicalize()` result at
   `guard.rs:155` before the suffix is re-appended.
2. Change `resolve`'s return type to `ResolvedPath`, and `read`/`write` (`:74`, `:87`) with it.
3. Change `deny` (`:105`) to take `&ResolvedPath` rather than `&Path`, so the rendering at `:123`
   is a compile error until it names a form. Use `to_script_string()`.
4. **Leave `:114` alone in this plan.** It renders grant roots off `FsGrant`, never guard-derived,
   and no diagnostic will point at it — plan 04 owns it. A comment marks it so the next reader
   does not assume it was missed.
5. Build and record every resulting compile error across the crate **before changing any
   consumer**. That list is spec §4's mechanism producing the consumer inventory. Write it into
   the handoff section at the foot of this plan; it is plan 04 task 1's input.
6. Restore compilation. `read`/`write` have 26 consumers (`modules/fs.rs` ×24,
   `modules/hash.rs:97`, `modules/glob.rs:102`), and this plan does not own the question of which
   form each of them wanted — plan 04 does. Give every deferred consumer `.as_path()` and nothing
   else.

   **`.as_path()` here is a deferral, not a fix.** At a rendering site it is precisely the failure
   mode plan 04 task 1 exists to catch: it clears the diagnostic and leaves the native spelling.
   That is tolerable only because the site is written down — the handoff list below, plus a grep
   for `.as_path()`, is what plan 04 triages from.
7. Verify:
   ```
   $ cargo make dod-crate airsl
   ```

### Task 3 — `resolve_root` and `contains_any`

**Files:**
- Modify `crates/airsl/src/sandbox/grants.rs`

**Steps:**

1. Write the failing test: a write root that does **not** exist yet must still match paths
   resolved beneath it. On Windows today it never does — `resolve_root`'s fallback at `:121-125`
   yields `Disk('C')` while checked paths canonicalise to `VerbatimDisk('C')`, and `Prefix`
   derives `PartialEq`, so the grant silently matches nothing. Spec §3.3.
2. Apply `strip_verbatim` to `resolve_root`'s `canonicalize()` branch so both sides are
   `Disk('C')`.
3. Replace the private `contains_any` at `:132-134` with `paths::containment::contains_any`.
4. Verify:
   ```
   $ cargo test -p airsl sandbox::grants
   ```

### Task 4 — The three remaining containment copies

**Files:**
- Modify `crates/airsl/src/require_loader.rs`
- Modify `crates/airsl/src/extension/manifest.rs`
- Modify `crates/airsl/src/extension/loaded.rs`

**Steps:**

1. `require_loader::resolve` (`:202-230`): delegate the `starts_with` at `:217` to
   `paths::containment::is_within`, and apply `strip_verbatim` to the value returned at `:223` —
   it becomes both the module-cache key (`:147`) and the traceback chunk name (`:197`), so an
   unstripped value leaks `\\?\` into a Lua traceback.
2. `manifest::validate_entry` (`:261-296`): delegate the `starts_with` at `:287`. It returns
   `relative`, not a canonicalised path, so no stripping is needed — note that in a comment so the
   asymmetry with the other two is not read as an oversight.
3. `loaded::recheck_entry` (`:135-154`): delegate the `starts_with` at `:147`, and strip before
   returning `resolved` at `:153`.
4. Verify:
   ```
   $ cargo test -p airsl require_loader extension::manifest extension::loaded
   ```

### Task 5 — Pin the accepted platform difference

**Files:**
- Modify `crates/airsl/src/modules/guard.rs`

**Steps:**

1. Document on `resolve` that the `ParentDir` arm is unix-reachable only, because
   `GetFullPathNameW` collapses `..` before the guard sees it, and that soundness holds on both
   platforms by different arguments. Spec §3.4.
2. Add a `#[cfg(windows)]` test asserting the guard's verdict for
   `<root>/link/../secret` matches what the OS actually opens — the check and the open must agree
   even though the answer differs from unix.
3. Give `guard.rs:294-304` and `:254-273` platform-conditional expectations: on Windows those
   errors cannot occur. Do not delete them; the unix assertion is still the one that matters on
   unix.

---

## Verification summary (plan-level)

- The §3.2 verbatim escape is refused, proven by a test that fails before this plan and passes
  after.
- A non-existent write root matches on Windows (§3.3).
- One containment predicate has four callers; no `starts_with`-against-a-root remains outside
  `paths::containment`. Confirm by grep.
- Every `canonicalize()` result crossing a function boundary is verbatim-stripped. Confirm by grep
  for `canonicalize` and checking each hit.
- `cargo make dod` green on all three platforms.

---

## Amendments after approval

| What | Why |
|---|---|
| `manifest::validate_entry` cited as `:280-291`, corrected to `:261-296` | The function starts at 261. The `starts_with` line (`:287`) was right; only the range was wrong. Verified by reading the file. |
| `loaded::recheck_entry` cited as `:140-152`, corrected to `:135-154`; its return cited as `:152`, corrected to `:153` | Same: `:147` was right, the range and the return line were off. `:152` is a closing brace. |
| Task 2 split into "record the inventory" and "restore compilation" (steps 5–7) | As approved, task 2 changed `read`/`write`'s return type and then verified `cargo make dod-crate airsl`. Those cannot both hold: the change breaks 26 consumers, and plan 04 task 1 step 4 verifies by *counting live compile errors* while task 2 step 1 says eighteen sites "compile unchanged" — both of which are only true if this plan ends with the crate not compiling. Ending a plan red contradicts this plan's own gate and the repo's rule that anything short of `cargo make dod` is not a green result. Resolved in favour of a green boundary: the inventory is still compiler-derived, but it is *recorded* rather than left live, and plan 04 was amended to triage from the record. |

## Handoff to plan 04 — deferred consumer sites

Every entry is a site that took a `PathBuf` out of `PathGuard::read` or `PathGuard::write`.
Twenty-six in total: `modules/fs.rs` ×24, `modules/hash.rs:97`, `modules/glob.rs:102`.

Obtained by an exhaustive sweep of the three files rather than by reading diagnostics, because the
diagnostics are transient and this list is not. It is the same set the compiler reports — every
row is a use of a value whose type changes — but it also records *what each use does*, which a
diagnostic does not say and which is the whole input to plan 04 task 1's classification.

> **Basis: the tree as this plan delivers it** (after the `.as_path()` repair), verified by
> grepping `modules/fs.rs`, `modules/hash.rs` and `modules/glob.rs`. An earlier draft of this
> table mixed pre- and post-repair numbers, which is worse than either — plan 04 consumes it as a
> work list and cannot tell which basis a given row used.

**Group 1 — filesystem handoffs. `.as_path()` is correct here permanently, not a deferral.**
The value goes to `std::fs`, `walkdir` or `tempfile`, which want a `&Path`.

| Site | Handoff |
|---|---|
| `fs.rs:86` | `std::fs::read_to_string(target.as_path())` |
| `fs.rs:97` | `std::fs::read_to_string(target.as_path())` |
| `fs.rs:120` | `std::fs::write(target.as_path(), body.as_bytes())` |
| `fs.rs:134` | `.open(target.as_path())` |
| `fs.rs:161` | `.open(target.as_path())` |
| `fs.rs:219` | `std::fs::symlink_metadata(target.as_path())` |
| `fs.rs:256` | `std::fs::read(a.as_path())` |
| `fs.rs:258` | `std::fs::read(b.as_path())` |
| `fs.rs:273` | `std::fs::read_dir(target.as_path())` |
| `fs.rs:298` | `std::fs::create_dir_all(target.as_path())` |
| `fs.rs:309` | `std::fs::remove_file(target.as_path())` |
| `fs.rs:320` | `std::fs::remove_dir_all(target.as_path())` |
| `fs.rs:332` | `std::fs::copy(source.as_path(), target.as_path())` — two values |
| `fs.rs:346` | `std::fs::rename(source.as_path(), target.as_path())` — two values |
| `fs.rs:362` | `.tempdir_in(checked.as_path())` |
| `fs.rs:376` | `.tempfile_in(checked.as_path())` |
| `hash.rs:98` | `std::fs::read(target.as_path())` |
| `glob.rs:106` | `walkdir::WalkDir::new(base.as_path())` |

**Group 1b — chained filesystem queries, which break differently.** These never bind a variable.
`ResolvedPath` exposes no such method, so each needs `.as_path()` before the query. Also
permanent, not a deferral.

| Site | Expression |
|---|---|
| `fs.rs:194` | `g.read("exists", …)?.as_path().exists()` |
| `fs.rs:202` | `g.read("is_file", …)?.as_path().is_file()` |
| `fs.rs:210` | `g.read("is_dir", …)?.as_path().is_dir()` |

**Group 2 — handoffs to a helper that renders below.** `.as_path()` compiles and leaves the
`display()` inside the helper natively spelled. **This is the deferral, and it is the failure mode
plan 04 task 1 exists to catch.** Plan 04 changes each helper's parameter to `&ResolvedPath`, at
which point these sites drop their `.as_path()` and are correct.

| Helper | Renders at | Called from |
|---|---|---|
| `io` (`fs.rs:53-60`) | `:54` `path.display().to_string()` | `fs.rs:87`, `:98`, `:121`, `:135`, `:137`, `:166`, `:173`, `:220`, `:256`, `:258`, `:275`, `:299`, `:310`, `:321`, `:333`, `:347`, `:363`, `:377` |
| `walk` | `root.display().to_string()`; `entry.path().strip_prefix(root)` | `fs.rs:289` |
| `atomic_write` | `target.display().to_string()`; also passes to `io` | `fs.rs:147` |

`atomic_write` additionally passes `directory`, derived from `target.parent()`. It is **not**
guard-derived and cannot honestly become a `ResolvedPath`; plan 04 task 2 gives it a separate
`io_at` helper rather than pretending otherwise.

**Group 3 — direct renderings, no helper to blame.** Each converts to `to_script_string()` in
plan 04.

| Site | Expression | Where it goes |
|---|---|---|
| `fs.rs:244` | `target.as_path().to_string_lossy()` | **returned to Lua** by `fs.canonicalize` — the reachable consequence that makes the verbatim hole more than theoretical |
| `fs.rs:380` | `checked.as_path().display().to_string()` | inline `Error::Io` in `fs.tempfile`, not routed through `io` |
| `hash.rs:100` | `target.as_path().display().to_string()` | inline `Error::Io { path, … }` |
| `glob.rs:109` | `base.as_path().display().to_string()` | inline `Error::Io { path, … }` |
| `glob.rs:112` | `entry.path().strip_prefix(base.as_path())` | the relative path each match is rendered from |

The `fs.rs:380` row was missed on this table's first pass and found by the compiler: it is a direct
rendering sitting between two `io(...)` calls, so a sweep keyed on the helper misses it.

---

## Amendments after approval

| What | Why |
|---|---|
| `manifest::validate_entry` cited as `:280-291`, corrected to `:261-296` | The function starts at 261. The `starts_with` line (`:287`) was right; only the range was wrong. Verified by reading the file. |
| `loaded::recheck_entry` cited as `:140-152`, corrected to `:135-154`; its return cited as `:152`, corrected to `:153` | Same: `:147` was right, the range and the return line were off. `:152` is a closing brace. |
| Task 2 split into "record the inventory" and "restore compilation" (steps 5–7) | As approved, task 2 changed `read`/`write`'s return type and then verified `cargo make dod-crate airsl`. Those cannot both hold: the change breaks 26 consumers, and plan 04 task 1 step 4 verifies by *counting live compile errors* while task 2 step 1 says eighteen sites "compile unchanged" — both of which are only true if this plan ends with the crate not compiling. Ending a plan red contradicts this plan's own gate and the repo's rule that anything short of `cargo make dod` is not a green result. Resolved in favour of a green boundary: the inventory is still compiler-derived, but it is *recorded* rather than left live, and plan 04 was amended to triage from the record. |

## Handoff to plan 04 — deferred consumer sites

Every entry is a site that took a `PathBuf` out of `PathGuard::read` or `PathGuard::write`.
Twenty-six in total: `modules/fs.rs` ×24, `modules/hash.rs:97`, `modules/glob.rs:102`.

Obtained by an exhaustive sweep of the three files rather than by reading diagnostics, because the
diagnostics are transient and this list is not. It is the same set the compiler reports — every
row is a use of a value whose type changes — but it also records *what each use does*, which a
diagnostic does not say and which is the whole input to plan 04 task 1's classification.

**Group 1 — filesystem handoffs. `.as_path()` is correct here permanently, not a deferral.**
The value goes to `std::fs`, `walkdir` or `tempfile`, which want a `&Path`.

| Site | Handoff |
|---|---|
| `fs.rs:86` | `std::fs::read_to_string(&target)` |
| `fs.rs:97` | `std::fs::read_to_string(&target)` |
| `fs.rs:119` | `std::fs::write(&target, body.as_bytes())` |
| `fs.rs:133` | `.open(&target)` |
| `fs.rs:160` | `.open(&target)` |
| `fs.rs:216` | `std::fs::symlink_metadata(&target)` |
| `fs.rs:251` | `std::fs::read(&a)` |
| `fs.rs:252` | `std::fs::read(&b)` |
| `fs.rs:266` | `std::fs::read_dir(&target)` |
| `fs.rs:290` | `std::fs::create_dir_all(&target)` |
| `fs.rs:301` | `std::fs::remove_file(&target)` |
| `fs.rs:312` | `std::fs::remove_dir_all(&target)` |
| `fs.rs:324` | `std::fs::copy(&source, &target)` |
| `fs.rs:337` | `std::fs::rename(&source, &target)` |
| `fs.rs:353` | `.tempdir_in(&checked)` |
| `fs.rs:367` | `.tempfile_in(&checked)` |
| `hash.rs:98` | `std::fs::read(&target)` |
| `glob.rs:106` | `walkdir::WalkDir::new(&base)` |

**Group 1b — chained filesystem queries, which break differently.** These never bind a variable:
`g.read(...)?.exists()`. `ResolvedPath` exposes no such method, so each needs `.as_path()` before
the query. Also permanent, not a deferral.

| Site | Expression |
|---|---|
| `fs.rs:191` | `g.read("exists", …)?.exists()` |
| `fs.rs:199` | `g.read("is_file", …)?.is_file()` |
| `fs.rs:207` | `g.read("is_dir", …)?.is_dir()` |

**Group 2 — handoffs to a helper that renders below.** `.as_path()` compiles and leaves the
`display()` inside the helper natively spelled. **This is the deferral, and it is the failure mode
plan 04 task 1 exists to catch.** Plan 04 changes the helper's parameter to `&ResolvedPath`, at
which point each of these drops its `.as_path()` and is correct.

| Helper | Renders at | Called from |
|---|---|---|
| `io` (`fs.rs:53-60`) | `:54` `path.display().to_string()` | `fs.rs:87`, `:97`, `:120`, `:136`, `:165`, `:172`, `:216`, `:251`, `:252`, `:267`, `:291`, `:302`, `:313`, `:338`, `:354`, `:368` |
| `walk` (`fs.rs:397-413`) | `:402` `root.display().to_string()`; `:408` `entry.path().strip_prefix(root)` | `fs.rs:281` |
| `atomic_write` (`fs.rs:419-434`) | `:430` `target.display().to_string()`; `:424`/`:426`/`:427` pass to `io` | `fs.rs:146` |

Note `atomic_write:424` passes `directory`, derived from `target.parent()`. It is **not**
guard-derived and cannot honestly become a `ResolvedPath`; plan 04 task 2 gives it a separate
`io_at` helper rather than pretending otherwise.

**Group 3 — direct renderings, no helper to blame.** Each converts to `to_script_string()` in
plan 04.

| Site | Expression | Where it goes |
|---|---|---|
| `fs.rs:240` | `target.to_string_lossy()` | **returned to Lua** by `fs.canonicalize` — this is the reachable consequence spec §3.2 names as the reason the verbatim hole is not merely theoretical |
| `hash.rs:100` | `target.display().to_string()` | inline `Error::Io { path, … }` |
| `glob.rs:109` | `base.display().to_string()` | inline `Error::Io { path, … }` |
| `glob.rs:112` | `entry.path().strip_prefix(&base)` | the relative path each match is rendered from |
| `fs.rs:380` (post-repair numbering) | `checked.display().to_string()` | inline `Error::Io` in `fs.tempfile`, not routed through `io` |

The `fs.tempfile` row was missed on the first pass of this table and added after the compiler
found it — it is a direct rendering that happens to sit next to two `io(...)` calls, so a sweep
that keys on the helper misses it. Plan 04 task 4 step 3 already owned it; only this table was
short.
