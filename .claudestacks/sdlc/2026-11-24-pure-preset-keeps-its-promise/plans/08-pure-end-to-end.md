---
status: done
created: 2026-11-25
depends-on: [02, 03, 04, 05, 07]
built-on: d8c7bb7f05b4
---

# pure end-to-end proof Implementation Plan

**Goal:** Prove with one script per preset that `pure` closes every route while `confined` keeps every host-access member.

**Architecture:** Two tests in `sandbox/policy.rs`'s `mod tests`, each running a Lua script through
`Engine` (spec §11.1, the `policy.rs` row). Today's tests there check `pure` field by field, and the
intent records that this is why the gaps went unnoticed. These tests exercise behaviour. No
production code changes.

**Tech Stack:** Rust 2024, `mlua` 0.12.0, `tempfile`, cargo-make.

**Content authority:** spec §2 (the "after" column), §9, §11.1 (the `policy.rs` row).

---

## Files that change

```
crates/airsl/src/sandbox/policy.rs   — modify: two end-to-end tests
```

## Order of work

### Task 1 — Assert every route is closed under `pure` and open under `confined`

**Files:**
- Modify `crates/airsl/src/sandbox/policy.rs`, `mod tests`: two tests that run a script, written to
  a temp dir so it has a root, through `Engine::builder().policy(..)`.

**Change:** Tests only.

**Proof:**
- `pure_reaches_nothing_a_script_could_observe` runs one script under `Policy::pure()`. With
  `pcall`, it collects the refusal text of each call below, then asserts each equals its spec §9
  row (the `{granted}` tail is `no access is`):
  - `print` is `nil`;
  - `stdio.read`, `stdio.lines`, `stdio.write`, `stdio.error`, `stdio.isatty()`;
  - `hook.payload`, `hook.emit`, `hook.context`;
  - `time.now`, `time.monotonic`, and `time.parse` with `%Q`;
  - `path.absolute("x")`;
  - `fs.read("p.lua")` and `fs.read` of an absolute path, both giving the rule-2 text;
  - `hash.hash_file("p.lua")` and `glob.walk(".", "*.lua")`, which pass valid arguments (E5 shows an
    argument error winning otherwise), each giving the rule-2 text under its own module name;
  - `require("x")`;
  - `math.randomseed()`.

  A second fresh `Policy::pure()` engine returns the same `math.random(1, 1000000)` as the first.
- `confined_keeps_every_host_access_member` runs under `Policy::confined()`, with a sibling module
  in the temp dir. It asserts:
  - `ext.granted().access` lists all seven members;
  - these calls do not raise: `stdio.isatty` for `"stdin"`, `"stdout"` and `"stderr"`, `time.now`,
    `time.monotonic`, `path.absolute("x")`, and `require` of the sibling;
  - `time.parse` with `%Q` is not refused: it either returns a number or raises an error whose text
    does not contain ``needs `zones` `` (every `time.parse` failure is a `Denied`, `time.rs:61-67`).
    The test does not depend on what a host's tzdb holds.

  It calls nothing that reads stdin or writes stdout or stderr (spec §11.1).

**Done when:** `cargo make dod` passes. Commit
`test(airsl): prove pure closes every route end to end`.

---

## Risks

- **Expected texts.** The expected strings are copied from spec §9 rows, not from the
  implementation. A wording drift in plans 02–05 fails here.

## Proof

- `cargo make dod` is green.
- The CLI checks that plans 02, 04 and 05 defer here, run with `cargo run -p airsl-cli -- run` on
  scripts in a temp dir:
  - `--policy pure`, on a script calling `airsstack.stdio.write`: exits non-zero with the spec §9
    `stdio.write` text.
  - `--policy pure --allow-access stdout`, on a script that writes `type(require)` and the result of
    `pcall(require, sibling)` through `stdio.write`: prints `function`, then the `require denied:`
    text.
  - The same with `--allow-access stdout --allow-access require`: prints `function`, then `true 42`
    (the confined result in E1).
  - `--policy pure --allow-access stdout`, on a script writing `math.random(1, 1000000)`, run three
    times: prints `275394` each time.

## Review findings

`0🔴 1🟡 4🔵`, blocking none. `cargo test -p airsl sandbox::policy` → 8 passed after the fixes below.

- 🟡 `policy.rs` comment used the spec's "shape B" vocabulary — fixed: "only the refusal's own words tell it apart".
- 🔵 `TRY` doc called a multi-statement prelude "one-line helper" — fixed: it names `try`, `s` and `out`.
- 🔵 the plan lists `stdio.isatty()` with no argument; the test passed `'stdout'` — fixed: called with no argument.
- 🔵 refusals asserted with `starts_with`, which would accept trailing text — fixed: the first line of `tostring(e)` must equal the expected text.
- 🔵 if the stdin refusals were reverted, `stdio.read`/`lines`/`hook.payload` would block on a terminal instead of failing — fixed 2026-11-25: the evaluation runs on a thread awaited with `recv_timeout` (10 s); proved with a scratch test granting stdin under `sleep 30 | cargo test`: failed at 10.01 s, "a stdin-reading call was not refused: the evaluation was still running after 10s".

## Deviations

- 2026-11-25 — Started while plan 07 Task 2 was in flight; the files are disjoint and it uses only Task 1's `--allow-access` for the CLI proof.
- 2026-11-25 — CLI Proof, real output (built binary, scripts in scratch): `run --policy pure w.lua` → exit 1, ``stdio.write denied: `stdout` is not granted — no access is``; `run --policy pure --allow-access stdout r.lua` → `function`, then ``false require denied: `require` is not granted — the granted access is stdout``; `run --allow-access stdout --allow-access require r.lua` → `function`, `true 42`; `run --policy pure --allow-access stdout m.lua` ×3 → `275394` each time.
