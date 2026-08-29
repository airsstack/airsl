---
status: approved
created: 2026-08-30
---

# Windows Build Spike Implementation Plan

**Goal:** Establish what actually happens when this workspace builds and tests on
`x86_64-pc-windows-msvc`, and record the failure inventory every later plan works from.

**Architecture:** No design work. The `compile_error!` stays; only its `cfg` predicate widens, so
unix and Windows both build and genuinely exotic targets still get an actionable sentence. A
Windows CI leg is added, and the build plus the test suite run. The output is data: does vendored
Lua compile, and which of the 567 tests fail and how. Spec §10's tasks 1 and 2.

**Tech Stack:** Rust 1.94 (`x86_64-pc-windows-msvc`), `cargo-make`, GitHub Actions
`windows-latest`, `mlua` with `vendored`.

**Content authority:** spec §10, and the discovery mechanisms of §4, §9 and §11.

**Amendments after approval.** This plan was approved, then edited during execution. Recorded here
because the chain directory is untracked, so git carries no history a reader could diff:

| What | Why |
| --- | --- |
| Task 3 step 1 rewritten | The original said "push and let the Windows leg run". `ci.yml:3-7` triggers only on push-to-`main`, `pull_request` and `workflow_dispatch`, so a push to this branch runs nothing. Replaced with the real commands, and the local cross-check evidence folded in. |
| Task 3 steps renumbered 3-5 | Consequence of the above. |
| Task 5 step 1 rewritten | The original said "run the full suite on Windows with nothing fixed". Impossible — the crate does not compile on Windows, so `cargo test` reaches no test. Replaced with the throwaway measurement spike, its rationale and its blind spot. |
| Task 5 steps renumbered 2-4 | Consequence of the above. |
| Architecture and file-structure lines | Said "temporary escape hatch" and "warning gate"; neither was ever built. Corrected to describe the widened `cfg` predicate actually delivered. |
| Final verification bullet | Scoped to the delivery branch, since the spike branch necessarily carries more. |

Task content that is *not* listed here is as approved.

---

## File structure

```
crates/airsl/src/lib.rs                — modify  widen the compile_error! cfg to admit windows
.github/workflows/ci.yml               — modify  add windows-latest to the matrix
.claudestacks/sdlc/2026-08-29-windows-platform-support/plans/INVENTORY.md
                                       — create  the recorded failure list (this plan's product)
```

This plan is expected to leave the tree **red on Windows**. That is its purpose. It must not fix
anything it discovers; fixes belong to plans 02–08.

### Task 1 — Relax the platform gate

**Files:**
- Modify `crates/airsl/src/lib.rs`

**Steps:**

1. Replace the `#[cfg(not(unix))] compile_error!` at `crates/airsl/src/lib.rs:39-43` with a
   `#[cfg(not(any(unix, windows)))]` form, so unix and Windows both build and genuinely exotic
   targets still get the message. Leave the explanatory comment at `:33-38` in place for now —
   plan 08 rewrites it.
2. Verify locally that unix is unaffected:
   ```
   $ cargo make dod
   ```
   Expected: green on macOS, exactly as before — 567 tests, zero warnings.

### Task 2 — Add the Windows CI leg

**Files:**
- Modify `.github/workflows/ci.yml`

**Steps:**

1. Add `windows-latest` to the matrix at `.github/workflows/ci.yml:37`.
2. Add `shell: bash` to the multi-line diagnostic step at `:46-51`.
3. Add a step, before the gate, enabling Developer Mode so symlink tests can run:
   ```
   reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock" /t REG_DWORD /f ^
     /v AllowDevelopmentWithoutDevLicense /d 1
   ```
4. Leave the comment at `:31-33` untouched; plan 08 rewrites it.

### Task 3 — Answer the build question

**Steps:**

1. Get the Windows leg to actually run. A plain branch push does **not** do this: `ci.yml:3-7`
   triggers on push to `main`, on `pull_request`, and on `workflow_dispatch` only, and this work
   sits on `worktree-windows-support`. Either open a PR against `main`, or dispatch it directly:
   ```
   $ git push -u origin worktree-windows-support
   $ gh workflow run ci.yml --ref worktree-windows-support
   ```
2. The question the leg answers is whether `mlua`'s `vendored` feature compiles Lua 5.4 from C
   under MSVC. Nobody in this project has ever observed it. A local cross-check on 2026-08-30
   narrowed it without settling it (all run timestamps below are UTC; the CI runs land on
   2026-08-29 UTC, which is 2026-08-30 in the author's local time):
   `cargo check --target x86_64-pc-windows-msvc -p airsl` got
   every pure-Rust dependency (globset, walkdir, sha2, jiff, tempfile, regex) through clean and
   stopped only in `mlua-sys`' build script, where `lua-src` invoked `cc` with `-DLUA_USE_WINDOWS`
   and Apple's `cc` failed on `#include <errno.h>` for want of a Windows SDK. So `lua-src` has a
   deliberate Windows branch and the failure was the host's missing headers, not the Lua source.
   That is evidence, not proof — only the MSVC runner settles it.
3. Record the answer in `INVENTORY.md` under `## Build`.
4. **If the build fails**, stop here and report. Every later plan assumes it succeeds; a failure
   changes the shape of the work rather than its size, and needs a decision before continuing.
5. Verify:
   ```
   $ cargo build --workspace --all-features    # on the Windows runner
   ```
   Expected: links a static Lua 5.4, no `pkg-config`, no system Lua.

### Task 4 — Record the compiler inventory

**Steps:**

1. From the same run, capture every compile error and warning on Windows. Under `-D warnings`
   these are failures, and they are the first half of spec §4's mechanism.
2. Record verbatim in `INVENTORY.md` under `## Compile`, grouped by file.

### Task 5 — Record the test inventory

**Steps:**

1. **The suite cannot be run "with nothing fixed" — this step as originally written is
   impossible, and the run on 2026-08-29 proved it.** `airsl` does not compile on Windows (ten
   errors, recorded under `## Compile`), so `cargo test` never reaches a test. No ordering of this
   plan's tasks produces a test inventory on an unmodified tree.

   Unblock it with a throwaway measurement spike on its own branch, kept off
   `worktree-windows-support`:
   - `#[cfg(unix)]` on the eight `#[test]` functions that call `std::os::unix::fs::symlink`
     (gate the whole function, not the statement — a gated statement leaves the rest of the body
     asserting against setup that no longer happened, which pollutes the very inventory being
     collected).
   - A stub `#[cfg(windows)]` arm for `proc::is_executable`, `meta.is_file()` and nothing more.

   The spike is a measuring instrument, not an implementation. Nothing from it is kept and the
   branch is **never merged**. It is retained rather than deleted, because it is the sole evidence
   behind every number in `INVENTORY.md`'s Tests section and its commit is cited there; delete it
   once those numbers are superseded by a real Windows-green run. Its cost is a known blind spot:
   it *skips*
   the symlink tests rather than porting them, so the inventory it yields is silent on symlink
   containment — the question plan 03 exists to answer. Record that limitation next to the results.

2. On the spike branch, run the full suite on Windows:
   ```
   $ cargo test --workspace --all-targets --all-features
   $ cargo test --workspace --all-features --doc
   ```
3. Record every failure verbatim in `INVENTORY.md` under `## Tests`, with the assertion text.
4. Triage each into spec §9's three classes — escape-processing string literals, errors that
   cannot occur on Windows, natively-rendered paths — plus a fourth bucket, `unclassified`, for
   anything the spec did not anticipate. The unclassified bucket is the one that matters most:
   it is what four rounds of reading the code failed to predict.

---

## Verification summary (plan-level)

- `cargo make dod` still green on Linux and macOS — this plan changes no behaviour.
- The Windows leg runs to completion and produces a build verdict.
- `INVENTORY.md` exists with four populated sections: Build, Compile, Tests, unclassified.
- No production code outside `lib.rs`'s cfg predicate has been modified **on the delivery
  branch**. The measurement spike of task 5 lives on its own branch and is never merged; if any of
  it reaches `worktree-windows-support`, this plan has failed its own purpose. Verify with
  `git diff <base>..HEAD --stat` (two files) and `grep -rn "cfg(unix)\|cfg(windows)" crates/`
  (no hits) on the delivery branch.
