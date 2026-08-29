---
status: approved
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
5. Build and record every resulting compile error across the crate. That list is spec §4's
   mechanism producing the consumer inventory; hand it to plan 04.
6. Verify:
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
2. `manifest::validate_entry` (`:280-291`): delegate the `starts_with` at `:287`. It returns
   `relative`, not a canonicalised path, so no stripping is needed — note that in a comment so the
   asymmetry with the other two is not read as an oversight.
3. `loaded::recheck_entry` (`:140-152`): delegate the `starts_with` at `:147`, and strip before
   returning `resolved` at `:152`.
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
