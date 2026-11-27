---
status: done
created: 2026-11-26
depends-on: [04]
built-on: af7c0dcc7368
---

# Rockspecs Implementation Plan

**Goal:** The `airsstack` and `airsstack-luajit` LuaRocks packages build, pack and load from this
repository with `cargo make rocks`.

**Architecture:** Two rockspecs are checked in under `crates/airsl-lua/rockspecs/`, named
`<package>-<version>-1.rockspec` at the workspace version `0.2.0`. Each uses `build.type =
"rust-mlua"` with `luarocks-build-rust-mlua == 0.2.7`, and builds `--package airsl-lua --locked`
from the workspace root. The `lua` dependency routes each package to its luarocks.org manifest
(§8.2):
- `airsstack` depends on `lua == 5.4`;
- `airsstack-luajit` depends on `lua == 5.1` and `luajit >= 2.1`.

`copy_directories = { "crates/airsl-lua/doc" }` makes the rocks carry only
`crates/airsl-lua/doc/LICENSE`, a copy of the root `LICENSE`, so `CLAUDE.md` and the other root docs
stay out (S13). There is no source rock (S12).

A `rocks` cargo-make task sits outside `dod`, like `deny` and `examples`, because it needs luarocks
and the network. Before it builds anything it looks up its three tools and each package's one
rockspec, refusing with the §5.6.5 messages (S19). It then runs `luarocks make`, `luarocks pack` and
`check.lua` (plan 04) for both packages into `target/rocks/`, moving between the workspace root and
`target/rocks/dist` with checked `cd`s (S26).

**Tech Stack:** LuaRocks 3.12.2 (`rockspec_format = "3.0"`), luarocks-build-rust-mlua 0.2.7-1,
cargo-make 0.37.24 duckscript (`which`, `glob_array`, `array_length`, `basename`, `length`/`calc`/`substring`,
`pwd`, `cd`, `mkdir`, `exec --fail-on-error`, as probes 233, 237 and 247 ran them), Lua 5.4 and
LuaJIT interpreters.

**Content authority:** spec §1.2 S8, S12, S13, S19, S26, §2.1 (`doc/LICENSE`, `rockspecs/*`,
`Makefile.toml` `rocks`), §2.2 (rows for `rockspecs/*.rockspec` and `doc/`), §2.10 M7, M12, §5.6.5,
§5.6.7 (the rocks rows), §6.4 (the two `cargo make rocks` rows), §8.1, §8.2, §8.3, §9.3 (`rocks`
row), §11 (the S17 transcripts that need `luarocks make`), §12 R14.

---

## Files that change

```
crates/airsl-lua/doc/LICENSE                             — create  a byte copy of the root LICENSE, the rocks' only doc file
crates/airsl-lua/rockspecs/airsstack-0.2.0-1.rockspec    — create  the §8.1 Lua 5.4 rockspec, byte for byte
crates/airsl-lua/rockspecs/airsstack-luajit-0.2.0-1.rockspec — create  the §8.1 LuaJIT rockspec, byte for byte
Makefile.toml                                            — modify  + task rocks (outside dod)
```

## Order of work

### Task 1 — Check in both rockspecs and the licence directory they copy

**Files:**
- Create `crates/airsl-lua/doc/LICENSE`: a byte copy of the root `LICENSE`. `crates/airsl/LICENSE`
  is identical to it. The rocks ship it as their only doc file.
- Create `crates/airsl-lua/rockspecs/airsstack-0.2.0-1.rockspec` and
  `crates/airsl-lua/rockspecs/airsstack-luajit-0.2.0-1.rockspec`, byte for byte from §8.1 (the files
  in `probes/235-rockspec-only-install-and-docs/src/crates/airsl-lua/rockspecs/`):
  - `source.url = "git+https://github.com/airsstack/airsl"` and `source.tag = "airsl-cli-v0.2.0"`;
  - the `description` table;
  - the `dependencies` and `build_dependencies`;
  - `build.type = "rust-mlua"`, `cargo_extra_args`, `modules = { airsstack = "airsstack" }` and
    `copy_directories`.

  The keys sit on the lines §5.6.3 names: `package` on line 2, `version` on line 3, `tag` on line 6.
  Plan 07's guard reports those line numbers. The files carry no chain or probe reference
  (doc-comment-discipline.md, leakage rule).

**Change:** `luarocks make` of either rockspec, from the workspace root, builds the native module
with cargo and installs it. `airsstack` is listed for Lua 5.4 only, and `airsstack-luajit` for 5.1
only.

**Proof:**
- `luarocks lint crates/airsl-lua/rockspecs/airsstack-0.2.0-1.rockspec` and the same for
  `airsstack-luajit-0.2.0-1.rockspec` both exit 0 (`probes/235-rockspec-only-install-and-docs.txt`:
  "both-lint-clean").
- `cmp LICENSE crates/airsl-lua/doc/LICENSE` reports no difference.

**Done when:** both lints exit 0 and `cargo make dod` is green. Commit `feat(airsl-lua): add the
airsstack and airsstack-luajit rockspecs`.

### Task 2 — Add the `rocks` task

**Files:**
- Modify `Makefile.toml`: add `rocks`, with `category = "Release"` and `script_runner =
  "@duckscript"`. Its comment says it is deliberately outside `dod`, as `deny` and `examples` are,
  because it needs luarocks and the network (luarocks fetches the build backend), and names the
  variables it reads: `AIRSL_LUAROCKS`, `AIRSL_LUA54`, `AIRSL_LUAJIT`, `AIRSL_LUA54_DIR` and
  `AIRSL_LUAJIT_DIR`. The task's body is the §9.3 `rocks` row, in this order:
  1. **Root.** The first line records `root` with `pwd`: `<root>` is the directory the task starts
     in, the workspace root when `cargo make rocks` runs there (probe 247 (a)).
  2. **Lookups, before any build** (§5.6.5, in this order; the first failure prints its message with
     `echo`, then `exit 1`):
     - luarocks: `AIRSL_LUAROCKS`, default `luarocks`, looked up with `which`; an empty answer gives
       `` rocks: `{program}` was not found; install LuaRocks or set AIRSL_LUAROCKS to its path ``;
     - Lua 5.4: `AIRSL_LUA54`, default `lua5.4`, looked up as native-load does; an empty answer gives
       `` rocks: `{program}` was not found; install Lua 5.4 or set AIRSL_LUA54 to its interpreter ``;
     - LuaJIT: `AIRSL_LUAJIT`, default `luajit`, the same way; an empty answer gives
       `` rocks: `{program}` was not found; install LuaJIT or set AIRSL_LUAJIT to its interpreter ``;
     - for `airsstack`, then `airsstack-luajit`: the files matching the anchored pattern
       `crates/airsl-lua/rockspecs/<package>-[0-9]*-1.rockspec`. No match gives
       `` rocks: no `{package}-<version>-1.rockspec` in `crates/airsl-lua/rockspecs` ``; more than one
       gives `` rocks: {n} files match `{package}-<version>-1.rockspec` in `crates/airsl-lua/rockspecs`
       ({paths, comma-separated, as the glob returns them: `\`-separated on Windows}); docs-check
       allows one per package `` (§5.6.5, probe 243).
       The one match is the package's rockspec (a relative path), and `<version>` is its base name
       without the leading `<package>-` and the trailing `-1.rockspec`, cut with `length`, `calc` and
       `substring` (probe 237).

     `{program}` is the variable's value or the default, as typed. The task does not compare
     `<version>` with the workspace version; docs-check owns that (§5.6.3).
  3. **Output directory.** `mkdir` `<root>/target/rocks/dist` once.
  4. **Per package**, `airsstack` with `--lua-version 5.4` and ABI `lua54`, then `airsstack-luajit`
     with `--lua-version 5.1` and ABI `luajit`; `--lua-dir` is added from `AIRSL_LUA54_DIR` /
     `AIRSL_LUAJIT_DIR` when the variable is set; every command runs through `exec --fail-on-error`
     with the looked-up luarocks or interpreter:
     1. from `<root>`: `<luarocks> <lua flags> make --tree <root>/target/rocks/<abi> <rockspec>`;
     2. `cd <root>/target/rocks/dist`, because `pack` writes into the working directory, then
        `<luarocks> <lua flags> --tree <root>/target/rocks/<abi> pack <package> <version>-1`;
     3. `cd <root>`, then `<interpreter> crates/airsl-lua/lua/check.lua
        <root>/target/rocks/<abi>/lib/lua/5.4 lua54` (or `…/lib/lua/5.1 luajit`).
  5. **Each `cd` is checked.** duckscript's `cd` answers empty and stays put when it cannot change
     directory (probe 247 (d)), so the task tests each `cd`'s answer and, when it is empty, prints
     `` rocks: could not change into `{dir}` ``, `{dir}` being the absolute directory it tried
     (`<root>/target/rocks/dist` or `<root>`), and exits 1 (probe 247 (e)). The return to `<root>`
     after `pack` is what makes the relative `check.lua` path and the second package's relative
     rockspec resolve (probe 247 (b), control (c)).

**Change:** `cargo make rocks` refuses, before any build, when luarocks or either interpreter is
missing or a package has no rockspec or more than one; otherwise it leaves two binary rocks in
`target/rocks/dist/`, `airsstack-0.2.0-1.<arch>.rock` and `airsstack-luajit-0.2.0-1.<arch>.rock`,
and proves each installed tree loads in its interpreter. This is how the book's pre-release
transcripts (plan 11) and a maintainer build the rocks locally.

**Proof:** The task is the test. Each refusal is seen before the success run is accepted, and each
refusal exits non-zero before any `Compiling` line (the forms of `probes/237-rocks-task-lookup.txt`):
- `AIRSL_LUAROCKS=nope-luarocks cargo make rocks` prints
  `` rocks: `nope-luarocks` was not found; install LuaRocks or set AIRSL_LUAROCKS to its path ``.
- `AIRSL_LUA54=nope-lua cargo make rocks` prints the Lua 5.4 line, and
  `AIRSL_LUAJIT=/no/such/luajit cargo make rocks` prints the LuaJIT line, each with the value as
  typed.
- With a temporary copy of the `airsstack` rockspec named `airsstack-0.1.0-1.rockspec` beside it,
  `cargo make rocks` prints `rocks: 2 files match `airsstack-<version>-1.rockspec` in
  `crates/airsl-lua/rockspecs` (…airsstack-0.1.0-1.rockspec, …airsstack-0.2.0-1.rockspec); docs-check
  allows one per package`. The copy is deleted after the run.
- With the `airsstack` rockspec moved out of the directory, `cargo make rocks` prints
  `` rocks: no `airsstack-<version>-1.rockspec` in `crates/airsl-lua/rockspecs` ``: the anchored
  pattern does not take `airsstack-luajit-0.2.0-1.rockspec` for it, which the unanchored
  `airsstack-*-1.rockspec` would (probe 237's control). The file is moved back after the run.
- The `cd` check is read against probe 247 (e)'s form; the real tree gives no `cd` that fails, so
  no run provokes it.
- `cargo make rocks` with every tool present prints "Packed: …airsstack-0.2.0-1.<arch>.rock" and
  "…airsstack-luajit-0.2.0-1.<arch>.rock", then `check.lua: ok (Lua 5.4)` and `check.lua: ok (Lua
  5.1, LuaJIT 2.1.…)`, and exits 0 (the forms of `probes/146-rockspecs-make-server.txt`: make from the
  root, pack with `--tree` from another directory). Both rocks are in `target/rocks/dist/`.
- `unzip -l target/rocks/dist/airsstack-0.2.0-1.<arch>.rock` lists the rockspec,
  `lib/airsstack.so`, `rock_manifest` and `crates/airsl-lua/doc/LICENSE`, and no `CLAUDE.md`
  (§8.3; `probes/235-rockspec-only-install-and-docs.txt`).
- `git status --short crates/airsl-lua/rockspecs` shows nothing after the runs.

**Done when:** every run above gives its result, `cargo make rocks` exits 0 with that output, and
`cargo make dod` is green. Commit `build(airsl-lua): add a rocks task that builds, packs and loads
both rocks`.

---

## Risks

- `source.tag = "airsl-cli-v0.2.0"` names a tag that has no `airsl-lua`. Nothing fetches it before
  the roadmap's release bumps the version (§2.5, intent constraint 8). `luarocks make` builds the
  local tree and never clones.
- `copy_directories` with a nested `/`-separated path was run on macOS only (R13, accepted). Its
  first Linux and Windows runs are the `build-rocks.yml` legs (plan 09).
- With Homebrew's luarocks defaulting to Lua 5.5, `rocks` relies on its explicit `--lua-version`. A
  machine whose luarocks cannot find the interpreter needs `AIRSL_LUA54_DIR`/`AIRSL_LUAJIT_DIR`.
  This plan adds no detection beyond that (S5).
- R14 (spec): luarocks' `make` and `pack` and `check.lua` were never run as one task; the moves
  between `<root>` and `target/rocks/dist` ran with stand-in commands (probes 247, 252). The first
  combined run is Task 2's success run.
- On Windows `pwd` answers a `\`-separated `<root>`, so the `cd` message shows both separators
  (probe 252). The spec keeps that text as written.
- Not done here: the guard that keeps rockspec names and contents on the workspace version (plan 07).

## Proof

- Both rockspecs lint clean.
- `cargo make rocks` packs and loads both rocks.
- `cargo make dod` is green.

## Review findings

- Reviewer (35-reviewer-p05-p06, shared with plan 05): plan 06 SPEC COMPLIANT; its one blocker was in plan 05's `ci.yml`. DoD: `cargo make dod` → exit 0, test step ends with both `check.lua: ok` lines; `luarocks lint` on both rockspecs → exit 0 each, both byte-identical to the probe 235 sources (`cmp`); `cmp LICENSE crates/airsl-lua/doc/LICENSE` → identical; `cargo make rocks` → exit 0, packing airsstack-0.2.0-1.macosx-aarch64.rock + "check.lua: ok (Lua 5.4)" and airsstack-luajit-0.2.0-1.macosx-aarch64.rock + "check.lua: ok (Lua 5.1, LuaJIT 2.1.1770848219)"; the 3 tool refusals → exit 1 with the plan's exact text and no `Compiling` line.
- 🔵 `Makefile.toml`:403 — `root = pwd` prints the root as a stray output line — open — the plan prescribes `pwd`; cosmetic.
- 🔵 `Makefile.toml`:463 — the comment says every argument is copied to a name first, but `${6}` is read with `get_env` — fixed — the comment says `${6}` is read with `get_env` before the first `enter`; `cargo make rocks` → exit 0 (both `check.lua: ok` lines); `cargo make dod` → exit 0.
- 🔵 `Makefile.toml`:459-470 — duckscript has one global namespace, so `root = set ${9}` rebinds the caller's `root`; the comment named only the `${1}` hazard — fixed — the comment names it (same value today); `cargo make rocks` → exit 0; `cargo make dod` → exit 0.
- 🟡 `Makefile.toml` (`rocks`) — no CI or DoD step runs `cargo make rocks`; reverting the `cd` check is caught by nothing, the anchored glob and the named-variable fix only by a manual success run — open — the plan accepts it (the task is the test; R12).

## Deviations

- 2026-11-26: duckscript function arguments share one namespace: a nested call (`enter ${dir}`) overwrote the caller's `${1}`, and the first success run failed in `pack` with "Error: cannot find package .../target/rocks/dist 0.2.0-1". `build_package` therefore copies its arguments to named variables before any nested call and takes `<root>` as `${9}` (a comment says why). The reviewer's scratch duckscript probe confirmed the clobbering. `cargo make rocks` → exit 0.
- 2026-11-26: `root = pwd` prints the directory (as in probe 247's raw output) and is not suppressed. The coder also reported `cd` echoing; the reviewer found that only `pwd` prints and an assigned `cd` prints nothing.
- 2026-11-26: the `cd`-failure branch ("rocks: could not change into ...") was not provoked in the tree, as the plan says. The reviewer's scratch duckscript probe confirmed it prints that message and exits 1.
- 2026-11-26: the Linux and Windows branches of `rocks` are unrun (macOS arm64 only; R12).
- 2026-11-26: `luarocks make` fetched `luarocks-build-rust-mlua` 0.2.7-1 from luarocks.org into each tree during `cargo make rocks`.
- 2026-11-26: Task 1's `cargo make dod` was deferred to the batch review (one cargo run at a time); the reviewer's run → exit 0.
- 2026-11-26: No commit was made: the execute skill commits only when the instruction to execute says so; the plan's per-task commit messages remain for the author.
