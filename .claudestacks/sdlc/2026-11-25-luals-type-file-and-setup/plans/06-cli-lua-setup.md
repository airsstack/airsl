---
status: done
created: 2026-11-25
depends-on: [05]
built-on: 044432888b01
---

# `airsl lua setup` Implementation Plan

**Goal:** `airsl lua setup [DIR]` sets up a project for LuaLS with the built-in modules under the default root.

**Architecture:**
- `cli.rs` gains a `lua` subcommand group, `Command::Lua(LuaCommand)` with `LuaCommand::Setup { dir: PathBuf }`. This follows how `Ext(ExtCommand)` is declared (`crates/airsl-cli/src/cli.rs:208-210`).
- The new `lua.rs` is a thin caller of `airsl::luals::setup(&dir, &airsl::modules::stdlib()?, &RootTable::default())`.
- Its entry point takes `stdout` and `stderr` as `&mut impl Write` and returns the exit code, the shape of `ext_fire::run` (`crates/airsl-cli/src/ext_fire.rs:91-98`).
- `main.rs` dispatches with the real locked streams, as it does for `Ext` (`crates/airsl-cli/src/main.rs:61-66`).
- The file is `lua.rs`, not `lua_setup.rs`, because the project allows no `<concept>_<qualifier>` files (§2).

**Tech Stack:** Rust 1.94, crate `airsl-cli`, `clap`, cargo-make. The acceptance check uses LuaLS 3.19.2 at `~/.local/share/nvim/mason/bin/lua-language-server`.

**Content authority:** spec §7, §10 (`cli.rs` and `lua.rs` tests, the acceptance check).

---

## Files that change

```
crates/airsl-cli/src/cli.rs    — modify  Command::Lua(LuaCommand), LuaCommand::Setup { dir }
crates/airsl-cli/src/lua.rs    — create  run `lua setup`: call luals::setup, print report or error
crates/airsl-cli/src/main.rs   — modify  mod lua; dispatch Command::Lua
```

## Order of work

### Task 1 — Add `airsl lua setup`

**Files:**
- Modify `crates/airsl-cli/src/cli.rs`:
  - add `Lua(LuaCommand)` to `Command`, with `#[command(subcommand)]` and a doc line;
  - add `pub(crate) enum LuaCommand { Setup { dir: PathBuf } }`, where `dir` has `#[arg(default_value = ".")]` like `Check`'s `path` (`cli.rs:195-199`).
- Create `crates/airsl-cli/src/lua.rs`: `pub(crate) fn run(command: LuaCommand, stdout: &mut impl Write, stderr: &mut impl Write) -> i32`. The module doc follows the crate's shape.
- Modify `crates/airsl-cli/src/main.rs`: add `mod lua;` and dispatch `Command::Lua` to `lua::run` with the locked real stdout and stderr, as the `Ext` arm does (`main.rs:61-66`).

**Change:**
- Parsing, `lua.rs` and dispatch land together, because a new `Command` variant without its `main.rs` arm does not compile.
- The help text says the command writes the LuaLS type file and luarc.
- On success, write the `SetupReport` to stdout followed by `\n`, and return 0.
- On any `airsl::Error` (including a failing `stdlib()`), write `airsl: {error}` and a newline to stderr, and return 1. That prefix is the one `check` and `run` already use (`check.rs:33`, `run.rs:46`).
- If the stdout write fails, write `airsl: {io error}` to stderr and return 1 (§7).

**Proof:**
- `lua_setup_defaults_to_the_working_directory` (in `cli.rs`, using its `run_command` helper at `cli.rs:354`): `["airsl", "lua", "setup"]` parses to `Command::Lua(LuaCommand::Setup { dir })` with `dir == PathBuf::from(".")`.
- `lua_setup_takes_a_directory` (in `cli.rs`): `["airsl", "lua", "setup", "proj"]` gives `dir == PathBuf::from("proj")`.
- `prints_the_report_and_exits_zero` (in `lua.rs`): on a fresh `tempfile::tempdir()`, stdout is `created .airsl/types/airsstack.lua\ncreated .luarc.json\n`, stderr is empty, and the code is 0.
- `prints_the_error_with_the_airsl_prefix_and_exits_one` (in `lua.rs`): on a missing directory, stderr starts with `airsl: open project directory failed on `, stdout is empty, and the code is 1.
- `a_failed_report_write_exits_one` (in `lua.rs`): stdout is a writer whose every write returns `io::Error::other("boom")`. Stderr is exactly `airsl: boom\n`, and the code is 1.

**Done when:**
- `cargo test -p airsl-cli cli::` and `cargo test -p airsl-cli lua::` pass.
- `cargo run -p airsl-cli -- lua setup <tempdir>` prints the two `created` lines.
- `cargo make dod` is green.

Commit `feat(airsl-cli): add airsl lua setup`.

### Task 2 — Run the LuaLS acceptance check

There is no file change of its own. The fixtures live in a scratch directory and are deleted afterwards. This check is not part of the gate, because CI has no LuaLS (spec §10).

**Change:** Run the spec §10 acceptance check twice on this machine:
- **Run A:** a fresh directory with `.airsl/` not ignored.
- **Run B:** a `git init` directory whose `.gitignore` holds `.airsl/`.

For each run:
1. `cargo run -p airsl-cli -- lua setup <dir>`
2. `cd <dir> && ~/.local/share/nvim/mason/bin/lua-language-server --check . --checklevel=Information`

Each directory holds two scripts:
- `good.lua` calls at least one function of each of the 12 modules with correct arguments. It reads every field of every §3.4 record the way `probes/luals/records-good.lua` does: every `Compiled` field, `Output`, `Stat`, `Granted` and its three nested records. It also uses `json.decode` and `hook.payload` results as a table, a number and a string.
- `nope.lua` reads `airsstack.json.nope`.

**Proof:**
- Each run reports no diagnostic for `good.lua`.
- Each run reports exactly one `undefined-field` for `nope.lua`.
- The command lines and the LuaLS output of both runs are pasted into the `## Review findings` section that `execute` appends to this plan.

**Done when:** both runs meet the proof, and the scratch directories are deleted. No commit. The results go in the plan's `## Review findings` section.

---

## Risks

- **The acceptance check is machine-local.** It depends on the Mason LuaLS 3.19.2 install. A diagnostic here that the unit tests missed means a description's types are wrong (spec §13). The fix belongs in plan 02's module `describe`, and it is recorded as a deviation.

## Proof

- `cargo make dod` is green.
- `cargo run -p airsl-cli -- lua setup <tempdir>` run twice: the second run prints two `unchanged` lines.
- Task 2's two LuaLS runs are recorded.

## Review findings

There was one review of Task 1. Verdict: `blocking: none`, 0🔴 0🟡 2🔵. `cargo make dod-crate airsl-cli` exited 0, with 91 tests passing. Two manual runs printed `created` ×2, then `unchanged` ×2. A missing directory gives `airsl: open project directory failed on …` and exit 1.

- **Fixed** 🔵: the test helper `setup` shared its name with the private `setup`. It is renamed to `setup_command`. `cargo test -q -p airsl-cli lua::` → `test result: ok. 3 passed; 0 failed`.
- **Fixed** 🔵: the `Lua` arm had no test. `main.rs` now has `dispatch(command, stdin, stdout, stderr)`, which `main` calls with the process streams, and the test `the_lua_setup_command_reports_on_stdout_and_leaves_stderr_empty` drives `airsl lua setup <tempdir>` through it. Both the `Ext` and `Lua` arms go through it now.

**Task 2: LuaLS 3.19.2 acceptance check.** The scratch directories were deleted afterwards. `good.lua` calls functions in all 12 modules. It reads every field of `Stat`, `Output`, `Compiled`, `Granted`, `GrantedFs`, `GrantedProc` and `GrantedLimits`, and it uses `json.decode` and `hook.payload` results as a table, a number and a string. `nope.lua` is `print(airsstack.json.nope)`.

Run A: a plain directory.

```
$ cargo run -q -p airsl-cli -- lua setup <scratch>/a
created .airsl/types/airsstack.lua
created .luarc.json
$ cd <scratch>/a && ~/.local/share/nvim/mason/bin/lua-language-server --check . --checklevel=Information
nope.lua:1:22 [Warning] Undefined field `nope`. (undefined-field)
    print(airsstack.json.nope)
                         ^^^^
>>>>>>>>>>>>>>>>>>>> 3/3 [Found 1 problems in 1 files]
Diagnosis complete, 1 problems found
```

Run B: `git init`, with `.gitignore` holding `.airsl/`. `git status --short --ignored` shows `!! .airsl/`.

```
$ cargo run -q -p airsl-cli -- lua setup <scratch>/b
created .airsl/types/airsstack.lua
created .luarc.json
$ cd <scratch>/b && ~/.local/share/nvim/mason/bin/lua-language-server --check . --checklevel=Information
nope.lua:1:22 [Warning] Undefined field `nope`. (undefined-field)
    print(airsstack.json.nope)
                         ^^^^
>>>>>>>>>>>>>>>>>>>> 3/3 [Found 1 problems in 1 files]
Diagnosis complete, 1 problems found
```

LuaLS printed ANSI colour codes, which are removed above.

Both runs report no diagnostic for `good.lua` and exactly one `undefined-field` for `nope.lua`.

As a control in run B, `bad.lua` read `airsstack.proc.run({"x"}).nostatus`. It got `Undefined field `nostatus`. (undefined-field)`. So the record fields are really checked, and `good.lua`'s clean result means something.

## Deviations

- 2026-11-25. There was no task-briefer pass. The coder read Task 1 directly. Task 2 was run by the orchestrator.
- 2026-11-25. The help text says "Lua language server", not "LuaLS". Clippy's `doc_markdown` would require backticks, which clap prints literally.
- 2026-11-25. No commit was made.

## Follow-up round (2026-11-25, at the author's request)

- **Fixed** 🔵: `main.rs` now calls `dispatch(command, stdin, stdout, stderr)`. New test: `the_lua_setup_command_reports_on_stdout_and_leaves_stderr_empty`.
- **Fixed 🔴** (it appeared in this round's review): `dispatch` first received `io::stdin().lock()` for every command. Stdin's lock is not reentrant, so `airsl run` with `stdio.read` or `hook.payload` hung. The unlocked handles are now passed in, which also removes the same hang from the `Ext` arm, where it existed before this chain. Checked by hand on the rebuilt binary: `echo hello | airsl run read.lua` printed `hello` with rc=0, and `echo '{"x":1}' | airsl run hook.lua` printed `1` with rc=0. No automated guard exists, because the crate has no binary-spawning tests.
- The help text now says "description" and "the language server's settings file".
