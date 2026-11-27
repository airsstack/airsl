---
status: done
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

## Review findings

(reviewer verdict: non-compliant, code 0🔴 5🟡 6🔵 · plan 2🟡, DoD green (`cargo make dod-crate docs-check`, 98 tests; `mdbook build book` clean), blocking: F1, P1)
- F1 🟡 `book/src/ch01-05-stock-lua.md:95`: the 5.4 `luarocks make` had no `--lua-version 5.4`, so it works only where luarocks defaults to 5.4 (probe 165: current Homebrew luarocks 3.13 defaults to 5.5); the next command passes the flag — fixed: the command is now `luarocks --lua-version 5.4 make --tree lua54-tree crates/airsl-lua/rockspecs/airsstack-[0-9]*-1.rockspec`, re-run on a fresh tree → rc 0, output repasted (11.45s); `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → 98 passed; `mdbook build book` clean.
- F2 🟡 `book/src/ch01-05-stock-lua.md:128`: the LuaJIT `luarocks make` showed no output although it prints a full transcript, and the "Missing dependencies" note stood for the LuaJIT build only — fixed: `luarocks --lua-version 5.1 make --tree luajit-tree crates/airsl-lua/rockspecs/airsstack-luajit-[0-9]*-1.rockspec` → rc 0, output pasted in full (9.17s), the hello run moved to its own block; the note is now version-neutral ("Some LuaRocks versions end a build with … as this one does"), since on luarocks 3.12.2 only the LuaJIT build prints it; `cargo make dod-crate docs-check` → 98 passed; `mdbook build book` clean.
- F3 🔵 `book/src/ch01-05-stock-lua.md:104`: `~/airsl` replaces the absolute path luarocks prints — open, accepted as nit (recorded in Deviations). — resolved, no change needed — the real path holds the author's home directory and must not be published; the substitution stays, recorded in Deviations.
- F4 🔵 `book/src/ch01-05-stock-lua.md:104`: the make transcript comes from a warm cargo target (one Compiling line); a fresh clone prints far more — open, accepted as nit. — fixed — a sentence says the run had a warm Rust cache (one `Compiling` line) and a first build in a fresh clone prints many more — evidence: `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → 119 passed; `mdbook build book` 0 warnings/errors.
- F5 🔵 `book/src/ch01-05-stock-lua.md:153`: differences.lua runs `sh -c`, so it cannot run as written on Windows — open, accepted as nit. — fixed — differences.lua runs `{ arg[-1], "-e", "io.write(os.getenv('GREETING'))" }`, the interpreter itself, no shell; output blocks unchanged — evidence: lua5.4 and luajit re-run locally, byte-identical to the pasted blocks; on windows-latest (CI rounds 1 and 2, runs 45940029592, 45941260102) Lua 5.4 printed `[2.0,9007199254740992.0]`, `true`, `hello`, `nil`, `hello` and LuaJIT `[2,9007199254740992]`, `false`, `hello`, `nil`, `hello`, as the book shows.
- F6 🟡 `book/src/ch01-05-stock-lua.md:257`: the Neovim cpath line (`~/.luarocks/.../?.so`) is Unix-only; Windows gets no form — open: the Windows form cannot be run on this machine and stays unproven; the plan did not ask for Windows here. — fixed — the snippet takes the cpath from `luarocks --lua-version 5.1 path --lr-cpath` through `vim.fn.system` (asserts `shell_error`, `vim.trim`), with no hard-coded extension or separator; the book says on Windows LuaJIT's `bin` folder must be on `PATH` too — evidence: macOS NVIM 0.11.6 printed `{"editor":"neovim"}` rc 0; windows-latest CI round 2 (run 45941260102), Neovim 0.12.6, following the book (LuaJIT `bin` + luarocks on PATH): `nvim -l nvim-airsstack.lua` printed `{"editor":"neovim"}` exit 0; without LuaJIT `bin` on PATH it failed with `Could not find Lua 5.1 in PATH.`, as the book says.
- F7 🔵 `book/src/ch01-05-stock-lua.md:274`: musl is told to install from source and then that it is not supported — open, accepted as nit. — fixed — "No binary package" now says musl Linux (Alpine) and unlisted platforms have none, the native module is not supported on musl (LuaRocks does not tell musl from glibc, so it can fetch the glibc package), and other platforms build from the recipe — evidence: `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → 119 passed; `mdbook build book` 0 warnings/errors.
- F8 🟡 `book/src/ch01-04-editor-support.md:88`: "the same description fits" stock Lua named only `ext`, but the type file also declares the global `airsstack`, which stock Lua does not set — fixed: ch01-04 now says the global `airsstack` and `ext` exist only inside airsl, and stock-Lua code uses `local airsstack = require("airsstack")`, linking ch01-05; `cargo make dod-crate docs-check` → 98 passed; `mdbook build book` clean.
- F9 🔵 `book/src/ch01-04-editor-support.md:88`: the `--native` fact is restated in ch01-04 and ch01-05 (both link) — open, accepted as nit. — fixed — the `--native` mention is removed from ch01-04; ch01-05 owns it and links Editor Support — evidence: `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → 119 passed; `mdbook build book` 0 warnings/errors.
- F10 🔵 `book/src/ch09-01-three-layers.md:27`: restates the 2.0→2 JSON fact owned by ch01-05; plan Task 2 asks for it — open, accepted as nit. — fixed — the ch09-01 paragraph says airsl always runs 5.4, the native module also runs on LuaJIT, and links ch01-05; the 2.0→2 fact lives only in ch01-05 — evidence: `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → 119 passed; `mdbook build book` 0 warnings/errors.
- F11 🟡 `crates/airsl-cli/src/lua/native.rs` `BOOK_PAGE`: no guard fails if ch01-05 or its SUMMARY entry is renamed; the plan's proof is a one-off grep — open: a docs-check guard lies outside plan 11's files. — fixed — a sixth docs-check check, `crates/docs-check/src/book_links.rs`: every `https://airsstack.github.io/airsl/…` URL in CLAUDE.md, CHANGELOG.md, the READMEs and the `.rs` files outside docs-check must name a chapter SUMMARY lists (with a heading id derived as mdbook 0.4.52 does), the site root, `index.html`, `print.html`, `404.html` or an `install/` file — evidence: with ch01-05 moved away, `every_link_into_the_book_resolves` failed listing README.md:64 and native.rs:58 and :443, green once restored; heading ids pinned against a real mdbook 0.4.52 scratch chapter (`heading_ids_equal_the_ones_mdbook_gave_a_scratch_chapter`, `…_setext_indented_and_fenced_text`); `CARGO_BUILD_JOBS=2 cargo make dod` → exit 0, docs-check 125 passed.
- P1 🟡 plan Task 1 "Building from the repository": "`cargo make rocks` produces the same build" was missing and not listed as a deviation — fixed: one paragraph added (builds both packages into `target/rocks`, packs a `.rock` per package into `target/rocks/dist`, loads each in its own Lua; needs LuaRocks and network), written from a real `cargo make rocks` run → rc 0 (19.52s); `cargo make dod-crate docs-check` → 98 passed; `mdbook build book` clean.
- P2 🟡 plan Task 1 Proof ("pasted unedited"): the omitted LuaJIT output (F2) and the edited path (F3) were not recorded as amendments — resolved: F2 fixed, the path edit and the other departures recorded in Deviations below.

- 🟡 (fix-pass review R3) the Neovim section's "This works on every platform" was never run on Windows; whether nvim.exe resolves the module's `lua51.dll` was unknown — fixed — run on windows-latest and the book's Windows wording corrected: LuaJIT's `bin` folder (the `--lua-dir` folder plus `\bin`) must be on `PATH`, or LuaRocks reports it could not find Lua 5.1 in `PATH` — evidence: CI round 2 (run 45941260102): with it, `{"editor":"neovim"}` exit 0 (Neovim 0.12.6 ships `lua51.dll` beside `nvim.exe`); without it, `Could not find Lua 5.1 in PATH.`, exit 1.
- 🟢 (fix-pass review nit 10) the differences.lua bullet said it "runs the same on every platform", unrun on Windows — fixed — the words are dropped — evidence: the Windows run matched the book anyway (runs 45940029592, 45941260102).
- 🟢 (fix-pass review nits 3–8, `book_links`, owned here through F11) "root and `index.html` always resolve" overstated; an unreadable scanned file or chapter was silently skipped or misreported; CHANGELOG.md unscanned and `print.html`/`404.html` would be false positives; `heading_ids` missed setext headings, read 4-space-indented `#` lines and toggled fences on any fence line; `use super::*`; over-long and short-wrapped lines — fixed — the docs and test name say an anchor on the root is checked against the first chapter; an unreadable file is reported ("could not be read") and an unreadable chapter says so; CHANGELOG.md scanned, `print.html`/`404.html` resolve; `heading_ids` returns ordered ids, handles single-line setext, skips 4+-column indents, tracks the fence by character and length, and strips inline HTML tags; tests import by name; lines rewrapped — evidence: the two mdbook-pinned tests were red before the rule change and green after; `CARGO_BUILD_JOBS=2 cargo make dod` → exit 0 (docs-check 124, then 125 with the restored plan-07 test).
- 2026-11-27 findings-fix pass: CI round 3 (run 45942172171) all 10 ci.yml jobs green on ubuntu/macos/windows; local `CARGO_BUILD_JOBS=2 cargo make dod` exit 0.

## Deviations

- 2026-11-27 — The pasted `luarocks make` transcripts replace the absolute worktree path with `~/airsl`; nothing else in them was edited.
- 2026-11-27 — The make commands use the globs `airsstack-[0-9]*-1.rockspec` and `airsstack-luajit-[0-9]*-1.rockspec`, so no release version sits in a command; the transcripts still print 0.2.0 and the backend 0.2.7, and will date.
- 2026-11-27 — Commands shown without output blocks: `airsl lua setup --native`, `luarocks install --local …`, the Windows `luarocks path` forms (copied from `hint()` in `native.rs`, not run), and the Homebrew, Ubuntu apt and Windows `--lua-dir` sections (drawn from CI probes 164, 165 and 167, not re-run).
- 2026-11-27 — On luarocks 3.12.2 only the LuaJIT build prints the trailing "Missing dependencies" message (probe 165 shows it after the 5.4 build on luarocks 3.13), so the chapter's wording is version-neutral.
- 2026-11-27 — Execution process: one coder ran tasks 1–2; no briefer spawns; workspace `cargo make dod` ran green once during this plan (exit 0).
- 2026-11-27 — A sixth docs-check check, `crates/docs-check/src/book_links.rs`, closes F11 beyond the spec's five; `CLAUDE.md`'s docs-check paragraph now says "six things" and describes it.
- 2026-11-27 — The Neovim snippet takes `package.cpath` from `luarocks --lua-version 5.1 path --lr-cpath` instead of a hard-coded `~/.luarocks/…/?.so`, so one form serves every platform.
- 2026-11-27 — The Neovim section says that on Windows LuaJIT's `bin` folder must be on `PATH` as well, or `luarocks path` fails with "Could not find Lua 5.1 in PATH" (CI round 2).
- 2026-11-27 — `differences.lua` runs its child through `arg[-1]` (the running interpreter) instead of `sh -c`, so it runs on Windows; its printed output is unchanged.
- 2026-11-27 — The "No binary package" section says the native module is not supported on musl and LuaRocks may fetch the glibc package there, replacing the install-from-source wording.
- 2026-11-27 — ch09-01 no longer restates the 2.0→2 JSON fact plan Task 2 asked for; it links ch01-05, which owns it (F10). ch01-04 no longer mentions `--native` (F9).
