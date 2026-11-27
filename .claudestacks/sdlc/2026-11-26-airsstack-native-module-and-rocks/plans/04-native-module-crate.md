---
status: done
created: 2026-11-26
depends-on: [01, 02, 03]
built-on: af7c0dcc7368
---

# Native Module Crate Implementation Plan

**Goal:** A new `airsl-lua` crate builds `airsstack` as a native Lua module that the gate loads into
a real interpreter of each supported ABI.

**Architecture:** A fourth member, `crates/airsl-lua`, is a `cdylib` with `[lib] name = "airsstack"`.
It is `publish = false` and dist ignores it. Its features `lua54` and `luajit` have no default. It
depends on `airsl` by path with `default-features = false, features = ["module"]` (§2.4, D8).

Its source is two files:
- `src/lib.rs`: crate docs and `mod entry;` only;
- `src/entry.rs`: the one `#[mlua::lua_module] fn airsstack`, whose body is the one expression
  §2.6 fixes: `standalone()` opened with the default root table (§2.6, §3.2, F9, F14).

`build.rs` adds `-undefined dynamic_lookup` on macOS only (F10). A module-mode crate cannot link a
Lua-touching Rust test (F4), so the crate has no Rust tests (D3, D4). Instead `lua/check.lua` runs
in a real `lua5.4` and a real `luajit` (§9.2), driven by a new `native-load` task. `test` becomes a
dependency-only task over `test-cargo` and `native-load`.

Every `--workspace` command excludes `airsl-lua`, because a workspace build unifies its `module`
with `vendored`/`send` (§2.3). Root `default-members` leaves it out, so bare `cargo
build`/`test`/`check` keep working (F15). clippy and doc run once more per ABI (§9.3), and the clippy
run over `airsl-lua` also lints `airsl` in module mode (M6). `deny.toml` takes the §9.5 diff
(D10). dist takes `precise-builds = true`, because without it dist builds `--workspace` and the
member breaks every release build (§10.1).

**Tech Stack:** Rust 1.94, `mlua` 0.12.0 `module` mode, cargo-make 0.37 duckscript, Lua 5.4
(`lua5.4`) and LuaJIT 2.1 (`luajit`) interpreters, cargo-deny 0.20.2, dist 0.33.1. Cargo runs with
`CARGO_BUILD_JOBS=2` (`CLAUDE.md`).

**Content authority:** spec §1.1 F4, F5, F9–F15, §2.1 (`Cargo.toml` members/default-members,
`Cargo.lock`, `deny.toml`, `dist-workspace.toml` precise-builds, `Makefile.toml`, `crates/airsl-lua/`
except rockspecs/doc/ci), §2.2, §2.3, §2.4 (`airsl-lua` manifest), §2.7, §2.8, §2.9 D3–D5, D8–D11,
§2.10 M6, M13, §3.2, §5.6.1, §5.6.2, §6.3 (module rows `check.lua` asserts), §9.2, §9.3 (clippy, doc,
test, test-doc, clippy-fix, dod-crate, native-load), §9.5, §10.1 (`precise-builds`), §11 (the
`Cargo.toml` `:3` comment row).

---

## Files that change

```
crates/airsl-lua/Cargo.toml            — create  the §2.4 manifest (cdylib "airsstack", lua54/luajit, airsl path dep, dist = false)
crates/airsl-lua/build.rs              — create  macOS -undefined dynamic_lookup; //! names unit-test exemption category 5
crates/airsl-lua/src/lib.rs            — create  crate docs + `mod entry;`; //! names unit-test exemption category 1
crates/airsl-lua/src/entry.rs          — create  the #[mlua::lua_module] fn airsstack; //! states D4
crates/airsl-lua/lua/check.lua         — create  the §9.2 interpreter check with the §5.6.2 messages
Cargo.toml                             — modify  members += crates/airsl-lua; default-members; members comment (:3)
Cargo.lock                             — modify  + airsl-lua
Makefile.toml                          — modify  --exclude airsl-lua, per-ABI clippy/doc, test-cargo + native-load under test, dod-crate airsl-lua branch, D5/D11 comments
deny.toml                              — modify  allow-wildcard-paths = true; skip syn@2 (§9.5 diff)
dist-workspace.toml                    — modify  precise-builds = true with its comment
.github/workflows/airsl-cli-release.yml — expected unchanged (probe 190); committed only if `dist generate` rewrites it (Task 4)
```

## Order of work

### Task 1 — Add `airsl-lua` to the workspace and lint and document it per ABI

**Files:**
- Create `crates/airsl-lua/Cargo.toml` with the §2.4 text, byte for byte
  (`probes/201-lockstep-manifests/crates_airsl-lua_Cargo.toml`).
- Create `crates/airsl-lua/build.rs`, the shape of
  `probes/230-real-module-set-stock-lua/crates/airsl-lua/build.rs`. When the target OS is macOS it
  prints the two `cargo:rustc-cdylib-link-arg` lines, `-undefined` and `dynamic_lookup`. Elsewhere it
  prints nothing. Its `//!` follows the crate's doc shape and carries a line naming unit-test
  exemption category 5 (build script) and why: it only prints link arguments
  (unit-test-mandate.md:25).
- Create `crates/airsl-lua/src/lib.rs`. The crate `//!` uses the house module-doc shape (what, why
  its own crate, `Responsibilities:`, `Non-responsibilities:`) and says:
  - it is the airsstack host modules as a loadable Lua library, installed through LuaRocks;
  - it runs with trusted authority, with no memory or instruction ceiling;
  - `ext` is absent;
  - no global is set;
  - process-wide state (the `env` overlay, `time`'s origin) is shared between every Lua state in
    the process;
  - it has no Rust tests, because a module-mode build cannot link one, and the interpreter check
    tests it instead.

  It also carries a line naming unit-test exemption category 1. The body is `mod entry;` only
  (mod-rs-export-only.md).
- Create `crates/airsl-lua/src/entry.rs`, the shape of
  `probes/230-real-module-set-stock-lua/crates/airsl-lua/src/entry.rs`:
  - it imports `airsl::mlua::{self, Lua, Result, Table}`, because the macro names `mlua` unqualified
    (`probes/05-feature-split-workspace.txt` (k1)/(k2)), and `RootTable`;
  - it holds one `#[mlua::lua_module] fn airsstack(lua: &Lua) -> Result<Table>` with the §2.6 body;
  - its `//!` names the exported symbol `luaopen_airsstack`, and states D4: there are no inline tests
    because a module-mode test cannot be built (F4), and the one delegating expression's logic is
    unit-tested in `airsl`;
  - it has no `unwrap`/`expect` (M-PANIC-IS-STOP).
- Modify `Cargo.toml`:
  - add `"crates/airsl-lua"` to `members`;
  - add the `default-members` line with its comment, byte for byte from §2.4
    (`probes/232-default-members.txt`);
  - reword the `members` comment (`Cargo.toml:3`): `docs-check` is unpublished and test-only,
    `airsl-lua` ships through LuaRocks rather than crates.io, and the other two are the crates.io
    releases.
- Modify `Cargo.lock`: commit what cargo writes for the new member.
- Modify `Makefile.toml`:
  - `clippy`: three commands, each must pass:
    - `cargo clippy --workspace --exclude airsl-lua --all-targets -- -D warnings`;
    - `cargo clippy -p airsl-lua --features lua54 --all-targets -- -D warnings`;
    - the same with `--features luajit`.
  - `doc`, with `RUSTDOCFLAGS="-D warnings"`: three commands:
    - `cargo doc --workspace --exclude airsl-lua --no-deps`;
    - `cargo doc -p airsl-lua --no-deps --features lua54`;
    - the same with `--features luajit`.
  - `test` becomes `dependencies = ["test-cargo"]` with no command of its own, the mechanism `dod`
    uses (`Makefile.toml:43-46`). The new task `test-cargo` keeps today's `test` comment and runs
    `cargo test --workspace --exclude airsl-lua --all-targets`.
  - `test-doc`: `cargo test --workspace --exclude airsl-lua --doc`.
  - `clippy-fix`: the three clippy commands with `--fix --allow-dirty` in place of `-- -D warnings`.

  Multi-command tasks use `script_runner = "@duckscript"` with `exec --fail-on-error` per command,
  the form `dod-crate` already uses (`Makefile.toml:393-415`). `doc` keeps `RUSTDOCFLAGS` scoped to
  the doc commands. `dod` keeps its five steps in their order.

**Change:**
- A bare `cargo build`, `cargo test` or `cargo check` at the root builds the other three members
  only and passes.
- `cargo make dod` lints and documents `airsl-lua` under both ABIs, and `airsl` in module mode with
  them.
- `-p airsl-lua` without a Lua feature fails with mlua's "No Lua feature enabled" (§2.4 table, last
  row).

**Proof:** There are no Rust tests (D3, D4). The interpreter check comes in Task 2. Checks:
- `cargo build` at the root exits 0, and `cargo build --workspace` fails with "`vendored`,
  `external` and `module` features are mutually exclusive". Both are F15, and both are documented.
- `cargo build -p airsl-lua --features lua54` exits 0, and on macOS
  `nm -gU target/debug/libairsstack.dylib` lists `T _luaopen_airsstack` (F9).
- `cargo clippy -p airsl-lua --features luajit --all-targets -- -D warnings` exits 0
  (`probes/143-luajit-stdlib-cfg.txt`).
- `cargo metadata --format-version 1 --no-deps` lists `airsl-lua 0.2.0`.

**Done when:** those checks give those results, `cargo tree -p airsl-lua --features lua54 -d --depth 0`
lists `syn v2.0.119` and `syn v3.0.3` and no other crate (the command and output of
`probes/201-lockstep-manifests.txt`; a bare `cargo tree -d` resolves only `default-members`, which
leave `airsl-lua` out), and `cargo make dod` is green.
Commit `feat(airsl-lua): build the airsstack modules as a native Lua module`.

### Task 2 — Load the module into real Lua 5.4 and LuaJIT inside `test`

**Files:**
- Create `crates/airsl-lua/lua/check.lua`. It takes the library directory as `arg[1]` and the ABI,
  `lua54` or `luajit`, as `arg[2]`. It raises each failure with `error("check.lua: …")`, using the
  exact §5.6.2 texts, in this order:
  - **usage:** a missing or unknown argument gives the usage message.
  - **interpreter:** checked before any `require`.
    - `lua54` requires `_VERSION == "Lua 5.4"` and no `jit` table.
    - `luajit` requires `rawget(_G, "jit")` to be a table.
  - **library path:** prepend `<dir>/?.so` and `<dir>/?.dll` to `package.cpath`.
  - **require:**
    - `require("airsstack")` succeeds, and the error is wrapped as `require("airsstack") failed: {error}`;
    - it returns a table;
    - the global `airsstack` is not set;
    - `package.loaded.airsstack` is that table;
    - `ext` is nil.
  - **modules:** each of `json`, `path`, `fs`, `env`, `proc`, `regex`, `hash`, `time`, `glob`,
    `stdio` and `hook` is a table.
  - **trusted authority:** `fs.read(arg[0])` succeeds.
  - **JSON:** `json.encode({2, 2.0})` is `[2,2.0]` under Lua 5.4 and `[2,2]` under LuaJIT.
  - **utf8:** under LuaJIT, `utf8` is nil.

  On success it prints `check.lua: ok (Lua 5.4)` or `check.lua: ok (Lua 5.1, {jit.version})`. Its
  name matches neither `*_test.lua` nor `test_*.lua`, so `airsl test` never collects it.
- Modify `Makefile.toml`:
  - add `native-load` with `script_runner = "@duckscript"`, the form proven in
    `probes/233-native-load-task-form/Makefile.toml`. For `lua54`, then `luajit`, it:
    1. resolves the interpreter from `AIRSL_LUA54` (default `lua5.4`) or `AIRSL_LUAJIT` (default
       `luajit`) with `which`;
    2. on an empty answer, prints the §5.6.1 message and fails, never skipping;
    3. runs `exec --fail-on-error cargo build -p airsl-lua --features <abi> --target-dir
       target/native-build`;
    4. copies `target/native-build/debug/libairsstack.dylib` (macOS), `libairsstack.so` (Linux) or
       `airsstack.dll` (Windows) to `target/native/<abi>/airsstack.so`, or `airsstack.dll` on
       Windows;
    5. runs `exec --fail-on-error <interpreter> crates/airsl-lua/lua/check.lua target/native/<abi>
       <abi>`.

    The explicit `--target-dir` fixes the library path whatever `CARGO_TARGET_DIR` says. A comment
    says why.
  - set `test`'s dependencies to `["test-cargo", "native-load"]`.
  - rewrite the comments that say the gate is exactly the guideline's five commands (D5, D11). Those
    are the header (`Makefile.toml:11-13`), the `test` comment (`:85-87`), the `examples` comment
    (`:107-108`) and the `deny` comment (`:291-293`). Each now says that the five step names and
    their order stay, that a step can run more than one cargo command, and that `test` also loads
    the native module into two real interpreters, so `dod` needs `lua5.4` and `luajit` (or
    `AIRSL_LUA54`/`AIRSL_LUAJIT`).

**Change:** `cargo make dod` fails, never skips, when either interpreter is missing. It also fails
when the module does not load or breaks a §9.2 assertion. A green `dod` means the real eleven-module
set loaded under both ABIs with trusted authority, no global and no `ext` (F14).

**Proof:** `check.lua` is the test. Each failure path is seen failing before the success path is
accepted:
- `cargo make native-load` prints `check.lua: ok (Lua 5.4)` and `check.lua: ok (Lua 5.1, LuaJIT
  2.1.…)`, and exits 0.
- `AIRSL_LUA54=nope-lua cargo make native-load` prints `` native-load: `nope-lua` was not found;
  install Lua 5.4 or set AIRSL_LUA54 to its interpreter `` and exits non-zero. `AIRSL_LUAJIT=/no/such/luajit`
  prints the LuaJIT message the same way (`probes/233-native-load-task-form.txt`).
- `lua5.4 crates/airsl-lua/lua/check.lua` with no arguments raises the usage message and exits 1.
- `luajit crates/airsl-lua/lua/check.lua target/native/lua54 lua54` raises `this check is for lua54,
  but the interpreter is Lua 5.1`.
- `lua5.4 crates/airsl-lua/lua/check.lua target/native/luajit luajit` raises `this check is for
  luajit, but the interpreter is Lua 5.4 without jit`.
- `lua5.4 crates/airsl-lua/lua/check.lua target/native/luajit lua54` raises `require("airsstack")
  failed: …`, because the LuaJIT build does not load into 5.4 (F5).

**Done when:** those runs give those results and `cargo make dod` is green. Commit
`test(airsl-lua): load the native module into real Lua 5.4 and LuaJIT in the gate`.

### Task 3 — Scope `dod-crate` to `airsl-lua`

**Files:**
- Modify `Makefile.toml`, the `dod-crate` script.
  - For `airsl-lua` it runs, as §9.3's `dod-crate` row lists and nothing more:
    - clippy `--all-targets -- -D warnings` under `--features lua54`, then `--features luajit`;
    - `cargo doc --no-deps` with `RUSTDOCFLAGS="-D warnings"` under each ABI;
    - no `cargo test` and no doctests (D3);
    - native-load.
  - Every other crate keeps the five commands, which plan 03 left without `--all-features`.
  - The task's comment states the `airsl-lua` branch and why: a module-mode crate has no runnable
    Rust tests and rustdoc cannot test a cdylib (F4).

**Change:** `cargo make dod-crate airsl-lua` is the scoped gate for the native module.
`cargo make dod-crate airsl` and `dod-crate airsl-cli` behave as before.

**Proof:**
- `cargo make dod-crate airsl-lua` runs the steps above, ends with both `check.lua: ok` lines, and
  exits 0.
- `cargo make dod-crate airsl` still runs its doctests and exits 0.

**Done when:** both commands exit 0 and `cargo make dod` is green. Commit `build(airsl-lua): scope
dod-crate to the native module`.

### Task 4 — Clear cargo-deny and dist for the new member

**Files:**
- Modify `deny.toml`: apply the §9.5 diff byte for byte
  (`probes/144-cargo-deny-features/deny.d.diff`). That is `allow-wildcard-paths = true`, and the
  `skip` entry for `syn@2` with its reason. `[graph] all-features = true` stays (D1).
- Modify `dist-workspace.toml`: add `precise-builds = true` after `pr-run-mode`, with a comment in
  the file's style. It says dist would otherwise build `--workspace`, where the native module's
  `module` feature cannot share a build. Change nothing else; plans 09 and 10 add the jobs.
- `.github/workflows/airsl-cli-release.yml`: expected unchanged.
  `probes/190-dist-rocks-global-job/airsl-cli-release.yml.diff` holds only the `custom-build-rocks`
  and `custom-publish-rocks` jobs and the `host`/`announce` `if` lines (plans 09 and 10), no line
  from `precise-builds`. If `dist generate --check` nonetheless reports drift, commit what
  `dist generate` writes, never a hand edit.

**Change:**
- `cargo deny check` passes with `airsl-lua` in the graph, so its licences and advisories are still
  checked.
- dist builds only the package it ships (`airsl-cli`), and `[package.metadata.dist] dist = false`
  (Task 1) keeps the cdylib out.

**Proof:**
- `cargo make deny` prints "advisories ok, bans ok, licenses ok, sources ok" and exits 0. Before the
  `deny.toml` edit it fails with `error[wildcard]` and `error[duplicate]`
  (`probes/144-cargo-deny-features.txt` (a) then (d)).
- `dist plan` exits 0 (`probes/44-dist-toy-member.txt` F: "--package=airsl-cli", exit 0).
- `dist generate --check` exits 0.

**Done when:** those three commands exit 0 and `cargo make dod` is green. Commit `build(workspace):
keep cargo-deny and dist clean with the native module`.

---

## Risks

- Task 2 is the riskiest step. Before this plan, `dod` needed only cargo. Now it needs two
  interpreters on every machine (D5, R8). Plan 05 installs them in CI, and plan 12 tells
  contributors.
- The duckscript form of native-load was run on macOS only (R12, accepted). Its first Linux and
  Windows runs are the pull request's `dod` job.
- An editor whose check passes `--workspace` meets the mlua feature error (§2.3). Plan 12 says so in
  `CLAUDE.md`. This plan cannot prevent it.
- Not done here: rockspecs and the `rocks` task (plan 06), CI interpreters (plan 05), and the rock
  workflows (plans 09, 10).

## Proof

- `cargo make dod` is green and its `test` step ends with both `check.lua: ok` lines.
- A bare `cargo build` at the root exits 0.
- `cargo make deny`, `dist plan` and `dist generate --check` exit 0.

## Review findings

- Reviewer (31-reviewer-p04): SPEC COMPLIANT-WITH-AMENDMENTS, blocking none; code 2🟡 10🔵, spec 2🔵 2❓. DoD: `cargo make dod` → exit 0 (865+101+66 tests, 27 doctests; test step ends with both `check.lua: ok` lines); bare `cargo build` → exit 0; `cargo make deny` → exit 0 "advisories ok, bans ok, licenses ok, sources ok"; `dist plan` → exit 0 (airsl-cli 0.2.0 only); `dist generate --check` → exit 0. Line numbers are as the reviewer cited them.
- 🔵 `Makefile.toml`:94 — `test` description omitted the native-module load — fixed — description names it; `cargo make native-load` → exit 0; `cargo make dod-crate airsl-lua` → exit 0; `cargo make dod` → exit 0 ending "check.lua: ok (Lua 5.4)" and "check.lua: ok (Lua 5.1, LuaJIT 2.1.1770848219)".
- 🔵 `Makefile.toml`:102-104 — `test-cargo` comment claimed it runs `native-load` (it is `test` that does); line 104 was 175 columns — fixed — rewritten: `test` is `test-cargo` plus `native-load`; `cargo make native-load` → exit 0; `cargo make dod-crate airsl-lua` → exit 0; `cargo make dod` → exit 0 ending "check.lua: ok (Lua 5.4)" and "check.lua: ok (Lua 5.1, LuaJIT 2.1.1770848219)".
- 🔵 `Makefile.toml`:128 — a set `build.target`/`CARGO_BUILD_TARGET` moves the library to `native-build/<triple>/debug` — fixed — the comment says so and that the `cp` then fails with "Path does not exist"; `cargo make native-load` → exit 0; `cargo make dod-crate airsl-lua` → exit 0; `cargo make dod` → exit 0 ending "check.lua: ok (Lua 5.4)" and "check.lua: ok (Lua 5.1, LuaJIT 2.1.1770848219)".
- 🔵 `Makefile.toml`:145 — duckscript `echo` adds a trailing space, one byte off the plan's Proof text (matches probe 233) — open — accepted, the message text is frozen; recorded under Deviations.
- 🔵 `Makefile.toml`:149-150 — OS detected from two sources (`is_windows` and `CARGO_MAKE_RUST_TARGET_OS`) — fixed — read once from `CARGO_MAKE_RUST_TARGET_OS` (`eq ${target_os} windows`); `cargo make native-load` → exit 0; `cargo make dod-crate airsl-lua` → exit 0; `cargo make dod` → exit 0 ending "check.lua: ok (Lua 5.4)" and "check.lua: ok (Lua 5.1, LuaJIT 2.1.1770848219)".
- 🔵 `Makefile.toml`:167-168 — interpreters resolved per ABI pass, so a missing `luajit` is reported only after the whole lua54 build and check — fixed (2026-11-27) — `find_interpreter` resolves both interpreters before either build; the refusal texts are byte-identical (review 38: `${3}`/`${1}` take the place of `${4}`/`${2}`), and native-refusals guards the order (see Deviations) — luajit lookup moved back after the lua54 build → native-refusals fails "native-refusals: native-load without LuaJIT ran cargo before refusing" (fix-round-1 coder; review 38's own revert in a scratch Makefile → exit 1, same text); `cargo make dod` → exit 0.
- 🔵 `Makefile.toml`:481 — `dod-crate airsl-lua` calls `exec cargo make native-load` — open — accepted (runs, exit 0; duckscript cannot call a task, and `exec cargo make native-refusals` now follows it the same way); recorded under Deviations.
- 🟡 `Cargo.toml`:6 — `default-members` is unguarded: no dod step, CI job or task runs a bare cargo command, so reverting it passes everything — fixed (2026-11-27) — `test-cargo` runs `cargo test --workspace --exclude airsl-lua --all-targets` and then a bare `cargo test --all-targets --no-run`, which resolves `default-members` (round 1 made test-cargo/test-doc bare; review 38 asked for the spec form back; round 2 restored it and kept the bare no-run beside it; the `default-members` comment in `Cargo.toml` names it) — default-members line deleted → `cargo make test-cargo` exit 1, "`vendored`, `external` and `module` features are mutually exclusive"; restored → bare no-run on warm artifacts 0 Compiling, 0.08s; `cargo make dod` → exit 0.
- 🔵 `crates/airsl-lua/build.rs`:9 — comment said `check.lua` fails if the link args are wrong; in fact the macOS link fails first ("Undefined symbols for architecture arm64") — fixed — comment says the cdylib link fails outright; `cargo make native-load` → exit 0; `cargo make dod-crate airsl-lua` → exit 0; `cargo make dod` → exit 0 ending "check.lua: ok (Lua 5.4)" and "check.lua: ok (Lua 5.1, LuaJIT 2.1.1770848219)".
- 🔵 `crates/airsl-lua/src/entry.rs`:9 — `ModuleSet::open` was plain code — fixed — intra-doc link to `airsl::modules::ModuleSet::open`; `cargo doc` under `-D warnings` green, `cargo make native-load` → exit 0; `cargo make dod-crate airsl-lua` → exit 0; `cargo make dod` → exit 0 ending "check.lua: ok (Lua 5.4)" and "check.lua: ok (Lua 5.1, LuaJIT 2.1.1770848219)".
- 🔵 `crates/airsl-lua/lua/check.lua`:12,24,46 — lines over 100 columns — fixed — rewrapped (`MODULES` now a local table), messages byte-identical (usage and both mismatch paths re-run by hand); `cargo make native-load` → exit 0; `cargo make dod-crate airsl-lua` → exit 0; `cargo make dod` → exit 0 ending "check.lua: ok (Lua 5.4)" and "check.lua: ok (Lua 5.1, LuaJIT 2.1.1770848219)".
- 🔵 `crates/airsl-lua/lua/check.lua`:27 (now :29) — prepending keeps the default cpath, so a hand run with no `<dir>/airsstack.so` could load an installed rock — accepted — spec §9.2 prepends; native-load's copy guarantees the file (fix round 1 made check.lua replace the cpath; round 2 reverted it after review 38: `git diff HEAD -- crates/airsl-lua/lua/check.lua | wc -l` → 0).
- 🟡 `Makefile.toml`:136-168 + `crates/airsl-lua/lua/check.lua`:15-25 — never-skip for a missing interpreter and check.lua's usage/mismatch branches are proven by hand runs only; reverting one to a skip or pass keeps dod green — fixed (2026-11-27) — new `native-refusals` task (a dependency of `test` after `native-load`, and run by `dod-crate airsl-lua`) provokes each refusal and requires a non-zero exit, the exact message and no cargo output — each revert fails it: missing-interpreter `exit 1` → `exit 0`: "native-load without Lua 5.4 exited 0, but it must refuse"; the `exit 1` removed: "native-load without Lua 5.4 ran cargo before refusing"; check.lua usage error → `os.exit(0)`: "check.lua with no arguments exited 0, but it must refuse"; lua54 mismatch check removed: "did not print: check.lua: this check is for lua54, but the interpreter is Lua 5.1"; luajit mismatch check removed: "did not print: ... luajit, but the interpreter is Lua 5.4 without jit"; round 2 mutations of the usage text, " or set " and the Lua 5.1 text each fail naming the missing text; `cargo make dod` → exit 0; CI run https://github.com/airsstack/airsl/actions/runs/45927429681 (snapshot fb2aa8d) ran native-refusals green on ubuntu-latest, macos-latest and windows-latest.
- 🔵 spec (plan Task 2, `Makefile.toml`:179, :363) — the four comments did not each carry the plan's three statements — fixed — the header, `test-cargo`, `examples` and `deny` comments each carry all three; `cargo make native-load` → exit 0; `cargo make dod-crate airsl-lua` → exit 0; `cargo make dod` → exit 0 ending "check.lua: ok (Lua 5.4)" and "check.lua: ok (Lua 5.1, LuaJIT 2.1.1770848219)".
- ❓ spec (plan Task 3, `Makefile.toml`:473) — `dod-crate airsl-lua` runs `cargo fmt -p airsl-lua -- --check`, which the plan's "nothing more" list omits — fixed (2026-11-27) — settled, kept: spec §9.3 excludes only `cargo test` and doctests for `airsl-lua` (D3), and fmt is the gate's first step for every crate; the `dod-crate` comment now says so — `cargo make dod-crate airsl-lua` → exit 0; review 38: "fmt runs first for every crate, matching the script".
- ❓ spec (plan Task 2 / spec §5.6.2) — check.lua message texts the plan does not quote were not compared with §5.6.2 — fixed (2026-11-27) — settled, no change: all 11 texts and both wrong-interpreter conditions match §5.6.2 byte for byte (fix-round-1 coder; review 38 re-checked every row and both ok lines).
- 🔵 spec (amendment hygiene) — deviations 1–3 were not recorded in the plan — fixed — recorded under Deviations below.
- Reviewer (38-reviewer-findings-fix, review of findings-fix round 1 for plans 01–06; recorded here, outcomes from fix round 2, 2026-11-27): SPEC COMPLIANT-WITH-AMENDMENTS; code 0🔴 2🟡 8🔵, spec 0🔴 3🟡 5🔵; blocking: test-cargo/test-doc bare selection, rocks root, spec §9.2 cpath. DoD re-run (CARGO_BUILD_JOBS=2, cargo-make 0.37.24): `cargo make dod` → exit 0 (866+1+101+66 tests, 27 doctests; "check.lua: ok (Lua 5.4)", "check.lua: ok (Lua 5.1, LuaJIT 2.1.1770848219)", then native-refusals silent); `cargo make rocks` → exit 0; `git diff --check HEAD` and `--cached --check` → rc 0. Round 2's verification: `cargo make dod` → exit 0 (both `check.lua: ok` lines, native-refusals ran); `cargo make rocks` → exit 0; `cargo build` → exit 0; `git diff --check HEAD` → exit 0.
- 🔵 `Makefile.toml` `[config]` — the `default_to_workspace` comment said every gate command carries `--workspace`, false once test-cargo/test-doc went bare — fixed (2026-11-27) — the comment says each command selects its packages itself (`--workspace`, `-p`, or a bare command that resolves `default-members`) — `cargo make dod` → exit 0.
- 🟡 `Makefile.toml` test-cargo/test-doc — with bare `cargo test`, what dod tests depends on `default-members`, so a member left out of it goes untested without error — fixed (2026-11-27) — back to `--workspace --exclude airsl-lua` (`--all-targets`, `--doc`); test-cargo adds a bare `cargo test --all-targets --no-run` as the default-members guard (see Deviations) — default-members line deleted → `cargo make test-cargo` exit 1, "`vendored`, `external` and `module` features are mutually exclusive"; restored → bare no-run on warm artifacts 0 Compiling, 0.08s; `cargo make dod` → exit 0.
- 🟡 `Makefile.toml` rocks `root = get_env CARGO_MAKE_WORKING_DIRECTORY` — unproven on Windows, while `pwd` is proven on all three runners (probe 252) — fixed (2026-11-27) — reverted to `root = pwd` (plan 06's form; its stray-line finding stays accepted there) — `cargo make rocks` → exit 0 (both rocks Packed, both `check.lua: ok` lines, the root line printed again).
- 🔵 `Makefile.toml` native-refusals — a third hand-written copy of the `AIRSL_LUA54`→`lua5.4` / `AIRSL_LUAJIT`→`luajit` defaults, which can drift — open — accepted: duckscript functions cannot cross tasks.
- 🔵 `Makefile.toml` native-refusals comment "runs after native-load" — true only inside `test` and `dod-crate` — fixed (2026-11-27) — the comment says it runs after `native-load` inside `test` and `dod-crate airsl-lua`, and run alone assumes the same — `cargo make dod` → exit 0.
- 🔵 `Makefile.toml` native-refusals — code lines of 118–191 columns — fixed (2026-11-27) — wrapped (`concat`, `expect_missing_interpreter`, `output`/`message` variables); only the task's `description` string is over 100 columns now — `cargo make dod` → exit 0.
- 🔵 `.gitattributes` — no comment says why, and nothing in the gate catches its removal (`git diff --check` is not a dod step) — open — the why-comment was added (2026-11-27, "cmd.exe labels and `goto` only work on CRLF line ends, so these files stay CRLF in every checkout."); no gate step guards the file, accepted.
- 🔵 `crates/airsl-lua/lua/check.lua` `package.cpath` — nothing fails if the replace form is reverted to prepend — fixed (2026-11-27) — moot: round 2 reverted check.lua to HEAD's prepend form — `git diff HEAD -- crates/airsl-lua/lua/check.lua | wc -l` → 0.
- 🔵 `crates/airsl/src/error.rs` `Error::root_table_install` doc — "engine's install and ModuleSet::open" is imprecise — fixed (2026-11-27) — the doc names `ModuleSet::install` (shared by the engine and `open`) and the builder's global set, and says the modules word their own `ModuleInstall` — `cargo make dod` → exit 0.
- 🔵 `crates/airsl/src/error.rs` test `a_root_table_install_failure_names_the_table_and_the_cause` — tests only the constructor; inlining the closure again at either call site passes every test — open — accepted nit, the fix is structural (plan 02).
- 🟡 spec §9.2 "prepends" — amendment missing for check.lua replacing the cpath — fixed (2026-11-27) — check.lua reverted to the §9.2 prepend form, so no amendment is needed (plan 04 finding on the default cpath is now accepted) — `git diff HEAD -- crates/airsl-lua/lua/check.lua | wc -l` → 0.
- 🟡 spec §9.3 rocks row / S26 `root = pwd` — amendment missing — fixed (2026-11-27) — reverted to `root = pwd`, the spec form — `cargo make rocks` → exit 0.
- 🟡 spec §9.3 test-cargo/test-doc rows — amendment missing, and "F15 does not apply (`--workspace` is explicit)" was no longer true — fixed (2026-11-27) — both are back to the spec's `--workspace --exclude airsl-lua` commands; the extra bare no-run is recorded under Deviations — `cargo make dod` → exit 0.
- 🔵 spec §9.3 test row and dod-crate airsl-lua row — the added `native-refusals` is a departure not recorded — open — kept; recorded under Deviations (2026-11-27); spec §9.3 rows lag and the spec is not edited.
- 🔵 spec §9.3 native-load row / this plan's per-ABI order — both interpreters are looked up before either build — open — kept; recorded under Deviations (2026-11-27); messages byte-identical, order guarded by native-refusals.
- 🔵 plan 12 Task 4 — "native-load runs inside `test`" will undersell CLAUDE.md, since native-refusals runs there too — open — for plan 12 when it executes.
- 🔵 plan 05 Deviations — still described the removed `rev-parse HEAD` compare — fixed (2026-11-27) — plan 05's entry corrected.
- 🔵 plans 01–06 Review findings — fixed items still marked open — fixed (2026-11-27) — statuses updated in this bookkeeping pass.

## Deviations

- 2026-11-26: the missing-interpreter message carries a trailing space after "interpreter", because duckscript's `echo` emits one (probe 233's recorded output has the same). Observed: `AIRSL_LUA54=nope-lua cargo make native-load` → exit 1, "native-load: `nope-lua` was not found; install Lua 5.4 or set AIRSL_LUA54 to its interpreter".
- 2026-11-26: `native-load` picks the library file name from cargo-make's `CARGO_MAKE_RUST_TARGET_OS` (macos → .dylib, windows → .dll, else .so), a variable the plan did not name; after review it is the single OS source (`is_windows` dropped). macOS resolves correctly (`cargo make native-load` → exit 0); the Linux and Windows branches are untested (accepted risk R12).
- 2026-11-26: `dod-crate airsl-lua` calls `cargo make native-load` through `exec`, because duckscript cannot invoke another task. `cargo make dod-crate airsl-lua` → exit 0, ending with both `check.lua: ok` lines.
- 2026-11-26: `LuaRocks` is backticked in the `crates/airsl-lua/src/lib.rs` docs, because clippy's `doc_markdown` lint requires it under `-D warnings`.
- 2026-11-26: No commit was made: the execute skill commits only when the instruction to execute says so; the plan's per-task commit messages remain for the author.
- 2026-11-27: a `native-refusals` task was added. It is a dependency of `test` (`["test-cargo", "native-load", "native-refusals"]`) and `dod-crate airsl-lua` runs it after `native-load`; spec §9.3's `test` and `dod-crate airsl-lua` rows lag (they name only `native-load`). `native-load` now resolves both interpreters before either build (`find_interpreter`), where this plan resolved them per ABI pass; the refusal message text is unchanged. `cargo make dod` → exit 0; CI run https://github.com/airsstack/airsl/actions/runs/45927429681 (snapshot fb2aa8d) green on three OSes with native-refusals passing.
- 2026-11-27: `test-cargo` is now a duckscript of two commands: `cargo test --workspace --exclude airsl-lua --all-targets`, then a bare `cargo test --all-targets --no-run`, which resolves `default-members` and so fails the step if `airsl-lua` is put back into it. Spec §9.3's commands are otherwise unchanged (`test-doc` stays `cargo test --workspace --exclude airsl-lua --doc`). Default-members line deleted → `cargo make test-cargo` exit 1; restored → `cargo make dod` exit 0.
