---
status: done
created: 2026-11-25
built-on: ae381756f52b
---

# Require Folder Implementation Plan

**Goal:** `require_loader.rs` becomes the `require/` folder of role files, with no change in behaviour.

**Architecture:**
- `crates/airsl/src/require/mod.rs` is export-only (`mod-rs-export-only.md`). It holds the `//!`
  block, `mod disposition; mod loader;` and the `pub(crate) use` lines for `RequireDisposition` and
  `RequireLoader`. `root.rs` is added by plan 02, not here.
- `disposition.rs` takes `RequireDisposition` and `decide` (`require_loader.rs:40-95`) with their
  tests. `loader.rs` takes `RequireLoader`, `LOADED_KEY`, `REQUIRE`, `load`, `run`, `compile` and
  `resolve` (`require_loader.rs:34-38`, `:97-318`) with their tests.
- Each file opens with the project's module-doc shape: what it is, why it exists as its own
  module, `Responsibilities:`, `Non-responsibilities:`. The current file's `//!` block
  (`require_loader.rs:1-18`) is divided between them, and `mod.rs` says why `require` is a folder:
  a concept with distinct roles (spec §7.1).
- The `#![expect(clippy::redundant_pub_crate, reason = …)]` attribute (`require_loader.rs:19-22`)
  goes into each file whose items keep `pub(crate)`.
- No file is named `require/require.rs` (clippy `module_inception`).

**Tech Stack:** Rust 1.94, crate `airsl`, `mlua` 0.12.0 (via `airsl::mlua`), cargo-make.

**Content authority:** spec §7.1, §7.2, §7.3, §9 (path references), §10 (split risk).

---

## Files that change

```
crates/airsl/src/require_loader.rs                        — delete  split into require/
crates/airsl/src/require/mod.rs                           — create  export-only table of contents
crates/airsl/src/require/disposition.rs                   — create  RequireDisposition + decide, their tests
crates/airsl/src/require/loader.rs                        — create  RequireLoader and the confined resolution, their tests
crates/airsl/src/lib.rs                                   — modify  `mod require_loader;` (:45) → `mod require;`
crates/airsl/src/engine.rs                                — modify  the `use` at :28 → crate::require::{…}
crates/airsl/src/extension/manifest.rs                    — modify  comment at :298 names require::loader::resolve
CLAUDE.md                                                 — modify  :93 `require_loader` → `require::loader`
crates/airsl/examples/multi-file-project/README.md        — modify  paths at :82 (→ src/require/disposition.rs), :120 and :154 (→ src/require/loader.rs)
crates/airsl/examples/policy-presets/README.md            — modify  path at :64 → src/require/disposition.rs
crates/airsl/examples/engine-reuse/README.md              — modify  path at :81 → src/require/loader.rs
```

## Order of work

### Task 1 — Split `require_loader.rs` into `require/`

**Files:**
- Delete `crates/airsl/src/require_loader.rs`.
- Create `crates/airsl/src/require/mod.rs`, `require/disposition.rs` and `require/loader.rs`.
- Modify `crates/airsl/src/lib.rs` and `crates/airsl/src/engine.rs`.

**Change:**
- Move every item and every test verbatim into the file that owns it (Architecture above). The
  only edits are the module path in `use` lines and the divided `//!` blocks.
- Tests about `decide` go to `disposition.rs`. These are
  `a_granted_require_is_confined_on_restricted_and_minimal`,
  `the_full_surface_keeps_lua_s_own_require_whether_or_not_the_script_has_a_root`,
  `a_script_with_no_root_has_nothing_to_resolve_against_and_gets_no_require` and
  `an_ungranted_require_is_refused_on_restricted_and_minimal`. Every other test in the current
  module goes to `loader.rs`, and each new test module keeps only the helpers and imports it uses.
- `Absent` keeps its name here. Plan 02 renames it.

**Proof:** no new test. The proof is that every moved test still passes under its new module
path, and that the count is unchanged:
- `cargo test -p airsl require::` runs as many tests as `cargo test -p airsl require_loader::` did
  on the parent commit (run that command before deleting the file and record the count in the
  commit message).

**Done when:** `cargo make dod-crate airsl` passes and the Definition of Done is green. Commit
`refactor(airsl): split require_loader into require/ role files`.

### Task 2 — Repoint every reference to the old path

**Files:**
- Modify `crates/airsl/src/extension/manifest.rs` (comment at `:298`).
- Modify `CLAUDE.md` (`:93`).
- Modify the three example READMEs at the lines listed above.

**Change:** each `require_loader` or `src/require_loader.rs` mention names the new module or file
that now holds the cited item: `require::loader::resolve`, `src/require/disposition.rs` for
`RequireDisposition::decide`, and `src/require/loader.rs` for `LOADED_KEY`,
`RequireLoader::install` and the containment tests. Nothing else in those passages changes here;
plan 04 rewrites their prose.

**Proof:** `git grep -n require_loader -- ':!.claudestacks'` prints nothing.

**Done when:** that grep prints nothing, `cargo make dod` passes, and the Definition of Done is
green. Commit `docs: point references at the require/ module`.

---

## Risks

- **Riskiest step: the move in Task 1.** It covers 843 lines (`wc -l
  crates/airsl/src/require_loader.rs`, spec §10). A test left behind or silently dropped would
  hide a regression. The test count recorded in the commit message is the guard.
- `pub(crate)` items may now trip `clippy::redundant_pub_crate` in files that lacked the
  `#![expect]`, and an `#[expect]` may go unfulfilled in a file that no longer needs it. The gate
  reports either. Keep the attribute only where it fires.

## Proof

- `cargo make dod` is green.
- `git grep -n require_loader -- ':!.claudestacks'` prints nothing.
- `cargo test -p airsl require::` runs the same number of tests the old module did.

## Review findings

Reviewer, 2026-11-25: 0 🔴, 0 🟡, 7 🔵. Nothing blocking. `cargo make dod` exit 0; `cargo test -p airsl --lib require::` → 33 passed (4 disposition, 29 loader), same names as the 33 under `require_loader::` on the parent commit; `git grep -n require_loader -- ':!.claudestacks'` → no output.

- 🔵 `require/mod.rs:10`: the doc says an absent `require` is "installed". Handed to plan 02, which replaces `Absent`.
- 🔵 `require/loader.rs:3`, `:6`: `//!` lines are 132 and 136 columns. Handed to plan 02 (same file).
- 🔵 `require/disposition.rs:11`: `//!` line is 101 columns. Handed to plan 02.
- 🔵 `CLAUDE.md:93`, `multi-file-project/README.md:82`: repointed lines are past the wrap. Handed to plan 04 (same files).
- 🔵 No gate step guards against a stale `require_loader` path coming back: fixed on 2026-11-25 at the author's request ("fix all of findings"). The unpublished workspace crate `crates/docs-check` fails `cargo make dod` when a cited `.rs` path, a module path or a lone code name in `CLAUDE.md` no longer resolves. Bite proof: changing `require::loader` to `require_loader` in CLAUDE.md fails with "cites require_loader: appears in no .rs or Cargo.toml file of the workspace".

## Deviations

- 2026-11-25: The task-briefer ledger/brief step was skipped. The coder read the plan directly, and Tasks 1 and 2 ran in one coder.
- 2026-11-25: `require/mod.rs` also carries `#![expect(clippy::redundant_pub_crate, …)]`. Without it, the `pub(crate) use` lines in a private module fail clippy. `paths/mod.rs` does the same.
- 2026-11-25: Not committed. Execute leaves the tree uncommitted until the author asks; the commit message for Task 1 must record the count 33.
