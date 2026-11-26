---
status: approved
created: 2026-11-26
depends-on: [06, 08]
built-on: af7c0dcc7368
---

# Stock Lua Book Chapter Implementation Plan

**Goal:** The airsl Book tells a user how to run, test and debug an airsl script in stock Lua 5.4
or LuaJIT with the native module.

**Architecture:** A new chapter, `book/src/ch01-05-stock-lua.md` ("Running Scripts in Stock Lua"),
is listed after Editor Support. It is the one home of every native-module fact: what the module is,
how to install it, how it differs from airsl, and how to point luarocks at the right Lua. E11's URL
points at it. Other chapters link to it rather than restate it (`CLAUDE.md` § Documentation: "A fact
lives in one chapter").

The chapter is written for a user: plain sentences, no internal vocabulary, no file:line citations,
no history. Every transcript with an output block is run against this checkout before it lands:
- `luarocks make` of the rockspec from a clone;
- `require("airsstack")` and module calls in `lua5.4` and `luajit`.

The `luarocks install --local` and `airsl lua setup --native` sections show their commands with no
output block until the release. Before the release, those commands can only print luarocks' "No
results matching query" (S17). Plan 12 adds the maintainer step that pastes the output.

**Tech Stack:** mdBook (`cargo make book-build`), Markdown, Lua 5.4 / LuaJIT / Neovim / LuaRocks
for the transcripts.

**Content authority:** spec §1.2 S5, S7, S11, S17, §3.2 (module behaviour in stock Lua), §5.4,
§5.5, §6.2 (Homebrew, apt, LuaJIT, musl rows), §6.3, §7, §8.3, §11 (rows for
`ch01-05-stock-lua.md`, `ch01-04-editor-support.md`, `appendix-01-cli.md`, `ch01-01-installation.md`,
`appendix-04-installation.md`, `ch09-01-three-layers.md`, `ch09-03-what-it-promises.md`,
`appendix-03-presets.md` / `ch03-05-presets.md` unchanged).

---

## Files that change

```
book/src/ch01-05-stock-lua.md          — create  "Running Scripts in Stock Lua": the native module, install, differences, per-OS luarocks setup
book/src/SUMMARY.md                    — modify  list the chapter after Editor Support
book/src/ch01-04-editor-support.md     — modify  link to the new chapter; `ext` is typed but exists only in airsl
book/src/appendix-01-cli.md            — modify  lua setup synopsis (:8), command row (:20), exit-code row (:71)
book/src/ch01-01-installation.md       — modify  a pointer to the new chapter
book/src/appendix-04-installation.md   — modify  a pointer to the new chapter
book/src/ch09-01-three-layers.md       — modify  airsl stays Lua 5.4; the native module also builds for LuaJIT, with the JSON difference (:22-25)
book/src/ch09-03-what-it-promises.md   — modify  the limits hold inside airsl; stock Lua has none (:10)
```

## Order of work

### Task 1 — Write "Running Scripts in Stock Lua"

**Files:**
- Create `book/src/ch01-05-stock-lua.md`. It covers, in this order:
  - **What the module is.** `local airsstack = require("airsstack")` in stock Lua 5.4 or LuaJIT
    returns a table of every airsl module except `ext`. It sets no global. It runs with trusted
    authority and no memory or instruction ceiling. `ext` exists only in airsl.
  - **The two packages.** `airsstack` is for Lua 5.4 and `airsstack-luajit` is for LuaJIT. Each Lua
    needs its own package.
  - **Installing.** `airsl lua setup --native [--lua 5.4|luajit]`, and plain
    `luarocks install --local airsstack` / `luarocks install --local airsstack-luajit`. Both sections
    show commands only, with no output block (S17). The section also gives the `luarocks path` hint
    per OS, in the §4 forms: unix `eval "$(luarocks path)"`, and Windows cmd and PowerShell.
  - **Building from the repository.** `luarocks make` of the rockspec from a clone, then `require`
    and a few module calls in `lua5.4` and `luajit`. Run them and paste the output. `cargo make
    rocks` (plan 06) produces the same build.
  - **Differences from airsl.**
    - JSON of an integral float: Lua 5.4 gives `2.0` and LuaJIT gives `2`; `2^53` gives
      `9007199254740992.0` against `9007199254740992`.
    - LuaJIT has no `utf8`.
    - There is no memory or instruction ceiling.
    - `env.set` changes what `env.get` and `proc.run`'s children see, but not Lua's own `os.getenv`.
    - Under LuaJIT with stdout redirected, `stdio.write`/`hook` output can appear before earlier
      `print` lines; Lua 5.4 keeps program order.

    Each is shown from a run (`probes/230-real-module-set-stock-lua.txt` gives the forms).
  - **Pointing LuaRocks at Lua 5.4 or LuaJIT** (the page E11 names):
    - **Homebrew:** `lua@5.4` is keg-only and luarocks defaults to 5.5; use `--lua-dir $(brew
      --prefix lua@5.4)` (`probes/165-y3-brew-macos-arm64.txt`).
    - **Ubuntu apt:** luarocks 3.8 defaults to PUC Lua 5.1, so use a LuaJIT-only directory with
      `--lua-dir` (`probes/164-y2-ubuntu-apt.txt`). A source build there needs the build backend in
      its 5.1 tree first (R5).
    - **Windows:** the luarocks zip, `luarocks.exe` rather than a `.bat` wrapper, and `--lua-dir`.
  - **Neovim.** It needs `airsstack-luajit` and one `package.cpath` line, shown from a run of
    Neovim (`probes/65-neovim-luajit-load.txt`).
  - **No binary rock.** musl Linux and other platforms without a binary rock install from the
    rockspec. That needs git, cargo with Rust 1.94 or newer, a C compiler, Lua headers and the build
    backend. musl is not supported.
  - Which package a Lua needs, stated once. Do not quote the §5.5 load errors (§5.5's last
    paragraph).
- Modify `book/src/SUMMARY.md`: add `- [Running Scripts in Stock Lua](ch01-05-stock-lua.md)` under
  Getting Started, after Editor Support.

**Change:** The book has the chapter E11's message links to, at the path that URL names
(`ch01-05-stock-lua.html`).

**Proof:** No Rust test. Each transcript with output is run against this checkout:
- the `luarocks make`, `lua5.4` and `luajit` runs;
- the Neovim `package.cpath` run.

Their output is pasted unedited. `cargo make book-build` builds with no warning. The rendered page
exists at `book/book/ch01-05-stock-lua.html`.

**Done when:** `cargo make book-build` succeeds and `cargo make dod` is green, so docs-check's
include guard passes. Commit `docs(book): add Running Scripts in Stock Lua`.

### Task 2 — Point the existing chapters at the native module

**Files:**
- Modify `book/src/ch01-04-editor-support.md`: link to the new chapter. Say that the type file still
  declares `ext`, which exists only in airsl (§7).
- Modify `book/src/appendix-01-cli.md`:
  - the synopsis line (`:8`) becomes `airsl lua setup  [--native [--lua 5.4|luajit]] [<DIR>]`;
  - the `lua setup` command row (`:20`) adds that `--native` also installs the native module with
    LuaRocks, linking the new chapter;
  - the `lua setup` exit-code row (`:71`) says `1` also covers luarocks failures, and that an install
    failure leaves the type file written.
- Modify `book/src/ch01-01-installation.md` and `book/src/appendix-04-installation.md`: add one
  pointer each to the new chapter, for running scripts outside airsl.
- Modify `book/src/ch09-01-three-layers.md` (`:22-25`): airsl itself stays Lua 5.4 for the
  integer/float reason. The native module also builds for LuaJIT, where JSON of an integral float
  differs, linking the new chapter.
- Modify `book/src/ch09-03-what-it-promises.md` (`:10`): the memory and time limits hold inside
  airsl. A script run in stock Lua through the native module has none.
- Leave `book/src/appendix-03-presets.md` and `book/src/ch03-05-presets.md` unchanged. They describe
  airsl, which stays Lua 5.4 with `utf8` (§11).

**Change:** Each chapter that touches Lua versions, limits, the CLI or installation links to the one
chapter that holds the facts.

**Proof:**
- `cargo run -p airsl-cli -- lua setup -h` matches the appendix's synopsis and wording.
- `cargo make book-build` succeeds.

**Done when:** `cargo make book-build` succeeds and `cargo make dod` is green. Commit `docs(book):
link the native module from the CLI, installation and design chapters`.

---

## Risks

- The pre-release chapter cannot show real `luarocks install` or `--native` output (S17). Until the
  release step pastes it, those two sections show commands only. This is the accepted state, not a
  gap.
- The apt and Windows instructions are drawn from CI probes and cannot be re-run on this machine.
  They show commands without output blocks.
- Not done here: README, CHANGELOG and `CLAUDE.md` prose, and the values-and-json example correction
  (plan 12).

## Proof

- `cargo make book-build` succeeds.
- `cargo make dod` is green.
- `grep -rn "ch01-05-stock-lua" crates/airsl-cli/src/lua/native.rs book/src/SUMMARY.md` finds the
  E11 URL and the SUMMARY entry naming the same page.
