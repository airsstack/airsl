---
status: done
created: 2026-11-26
built-on: af7c0dcc7368
---

# Lockstep Version Implementation Plan

**Goal:** `airsl` and `airsl-cli` take their version from one `[workspace.package] version`.

**Architecture:** The number moves from each member manifest to the root's `[workspace.package]`. Both
published members inherit it with `version.workspace = true`, and `crates/airsl-lua` will inherit
it the same way when plan 04 creates it. The workspace dependency `airsl = { version = "0.2.0",
path = "crates/airsl" }` keeps its explicit version, because crates.io packaging needs it. The
manifest-equality test in `version.rs` leaves, because `package.version` is no longer a string
there. docs-check's fifth guard (plan 07) takes over the lockstep check. `docs-check` keeps its own
`0.0.0`. Deviation D7 records the departure from workspace.md's "members carry their own version".

**Tech Stack:** Cargo workspace manifests (Rust 1.94, cargo 1.94.1), crate `airsl`, cargo-make.
Cargo runs with `CARGO_BUILD_JOBS=2` (`CLAUDE.md`).

**Content authority:** spec §1.2 S2, §2.1 (root `Cargo.toml`, the two member manifests,
`src/version.rs`), §2.4 (the manifest diffs), §2.5, §2.9 D7, §2.10 M11, §9.1 (`version.rs`).

---

## Files that change

```
Cargo.toml                         — modify  [workspace.package] gains version = "0.2.0" and its comment
crates/airsl/Cargo.toml            — modify  version.workspace = true
crates/airsl-cli/Cargo.toml        — modify  version.workspace = true
crates/airsl/src/version.rs        — modify  the manifest-equality test leaves
```

## Order of work

### Task 1 — Move the version into `[workspace.package]`

**Files:**
- Modify `Cargo.toml`: add `version = "0.2.0"` to `[workspace.package]`, with the one-line comment
  that `probes/201-lockstep-manifests/Cargo.toml.diff` carries ("One number for airsl, airsl-cli and
  airsl-lua: the CLI installs the rocks at its own version."), placed as that diff places it. Leave
  the `members` line and the mlua entry unchanged (plans 03 and 04 own them).
- Modify `crates/airsl/Cargo.toml`: replace `version = "0.2.0"` with `version.workspace = true`,
  aligned as in `probes/201-lockstep-manifests/crates_airsl_Cargo.toml.diff`. Do not add the
  `[features]` table; plan 03 owns it.
- Modify `crates/airsl-cli/Cargo.toml`: the same `version` line change, as in
  `probes/201-lockstep-manifests/crates_airsl-cli_Cargo.toml.diff`.
- Modify `crates/airsl/src/version.rs`: delete
  `the_version_is_the_one_the_crate_manifest_declares`. Its `include_str!("../Cargo.toml")` read of
  `manifest["package"]["version"].as_str()` would now unwrap `None` (`version.rs:35-40`). Keep
  `the_version_has_three_numeric_components`. Leave the module's `//!` and the `VERSION` constant
  unchanged.

**Change:** Cargo reports `airsl 0.2.0` and `airsl-cli 0.2.0`, read from one place
(`probes/201-lockstep-manifests.txt`: "airsl 0.2.0", "airsl-cli 0.2.0"). `airsl::VERSION` still comes
from `CARGO_PKG_VERSION`, which is now the workspace number. No behaviour changes.

**Proof:** This task adds no new test, because it changes no behaviour. The deleted test would fail
after the manifest change, which is why it leaves in this same task (M11).
- `the_version_has_three_numeric_components` still passes.
- `cargo metadata --format-version 1 --no-deps` lists `airsl` and `airsl-cli` at `0.2.0` and
  `docs-check` at `0.0.0`.

**Done when:** `cargo test -p airsl version::` passes, `cargo metadata --format-version 1 > /dev/null`
succeeds, `cargo tree -d` reports no duplicate beyond those it reports today, and `cargo make dod`
is green. Commit `chore(workspace): take every published crate's version from the workspace`.

---

## Risks

- The `airsl` workspace dependency (`Cargo.toml:57`) must keep `version = "0.2.0"` beside `path`.
  Dropping it would break `cargo package` for `airsl-cli`. This plan leaves that line alone.
- Nothing enforces lockstep until plan 07 adds docs-check's fifth guard. A member could set its own
  `version` again in the meantime. This is accepted because the plans land together, before any
  release (intent constraint 8).
- `cargo make publish-dry-run` (outside `dod`) reads the inherited version. This plan does not run
  it, because the spec does not ask for it at this stage.

## Proof

- `cargo make dod` is green.
- `cargo metadata --format-version 1 --no-deps` shows both published members at the workspace
  version.

## Review findings

- Reviewer (12-reviewer-p01): SPEC COMPLIANT, blocking none. DoD: `cargo make dod` → exit 0 (fmt, clippy -D warnings, rustdoc -D warnings, 855+101+66+1 tests, 25 doctests).
- 🔵 `Cargo.toml`:10 — the `[workspace.package]` comment names `airsl-lua` and says the CLI installs the rocks at its own version, which was not true at this commit (plan-mandated text) — fixed (2026-11-27) — the comment now says only what is true today: airsl, airsl-cli and airsl-lua share one number, so the crates and the LuaRocks packages built from one commit carry the same version — `cargo make dod` → exit 0 (866+101+66+1 tests, 27 doctests); review 38 re-checked it: "The version comment is true today (rockspecs at 0.2.0)".
- 🟡 `Cargo.toml` / `crates/*/Cargo.toml` — nothing guards the lockstep; a member going back to its own `version` fails no test, lint or DoD step — open — plan 07 adds the docs-check guard; the plan's Risks accept it until then, and neither findings-fix round (2026-11-27) touched it.
- 🔵 `crates/airsl/src/version.rs`:27 (tests module) — removing the `#![expect(clippy::unwrap_used)]` is not in Task 1's text — open — accepted, no code change; recorded under Deviations.

## Deviations

- 2026-11-26: Task 1 named only the test deletion in `crates/airsl/src/version.rs`; the test module's `#![expect(clippy::unwrap_used, …)]` was removed as well, because no unwrap is left for it to cover. Keeping it fails clippy: "error: this lint expectation is unfulfilled --> crates/airsl/src/version.rs:29:9" (`-D unfulfilled-lint-expectations` implied by `-D warnings`; reproduced by the reviewer's probe on cargo 1.94.1, exit 101). With it removed, `cargo make dod` → exit 0.
- 2026-11-26: No commit was made: the execute skill commits only when the instruction to execute says so; the plan's per-task commit messages remain for the author.
