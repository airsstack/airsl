---
status: done
created: 2026-11-25
depends-on: [06]
built-on: 044432888b01
---

# Editor Support Documentation Plan

**Goal:** The airsl Book tells a reader how to get editor completion for airsl scripts and how to describe their own module.

**Architecture:**
- The book is written for users (project `CLAUDE.md`, "Documentation"): plain sentences, one self-contained example per chapter, and every transcript and example run against the checkout before it lands.
- A fact lives in one chapter, and other chapters link to it.
- The new chapter sits under Getting Started.
- The CLI appendix gains one row.
- Chapter 7 is updated for the required `describe` and shows the two `luals` calls a host makes.
- Rustdoc and doctests were already added by plans 01–05.

**Tech Stack:** mdBook (`cargo make book-build`), Markdown, the `airsl` binary from this checkout, LuaLS 3.19.2 for the chapter's transcript.

**Content authority:** spec §11.

---

## Files that change

```
book/src/ch01-04-editor-support.md     — create  "Editor Support": lua setup, what it writes, gitignore note, re-run after upgrade
book/src/SUMMARY.md                    — modify  list ch01-04 under Getting Started
book/src/appendix-01-cli.md            — modify  + `lua setup` in the synopsis, Commands and Exit codes tables
README.md                              — modify  line 57: the airsl-cli row lists `lua`
crates/airsl-cli/README.md             — modify  + `airsl lua setup [DIR]` row in the command table (lines 62-68)
book/src/ch07-00-your-own-module.md    — modify  three methods; describe in every example; verify and setup section
crates/airsl/examples/README.md        — modify  only if line 66's wording became wrong (spec §11)
crates/airsl/examples/custom-module/README.md — modify  line 69's `registry.rs:144` citation follows the moved line
crates/airsl-cli/Cargo.toml            — modify  line 3 description gains `lua setup`
book/src/ch01-00-getting-started.md    — modify  intro sentence gains setting up an editor
CLAUDE.md                              — modify  command list (line 9) gains `lua setup`; the seam bullet (line 81) names `describe`
```

## Order of work

### Task 1 — Write the Editor Support chapter

**Files:**
- Create `book/src/ch01-04-editor-support.md`.
- Modify `book/src/SUMMARY.md`: add `    - [Editor Support](ch01-04-editor-support.md)` after the `ch01-03` line.

- Modify `book/src/ch01-00-getting-started.md`: the intro's first sentence, which lists what the chapter covers, also names setting up an editor for airsl scripts.

**Change:** The chapter covers these things:
1. Running `airsl lua setup` in a project. The transcript of a first run is pasted from a real run.
2. What it writes: `.airsl/types/airsstack.lua` and the `workspace.library` entry in `.luarc.json`, or in `.luarc.jsonc` when only that file exists. Existing settings are kept, a file with comments is refused with the message telling the reader to add the entry by hand, and a second run reports `unchanged`.
3. The note that `.airsl/` may be gitignored, because the luarc entry is what lets the language server find it.
4. Re-running setup after upgrading airsl.
5. A luarc that needs an edit is rewritten with sorted keys, losing its own formatting (D10, L11).

Its example is a two-line Lua script that calls `airsstack.json.encode`. The chapter shows the LuaLS `--check` result before and after setup, pasted from a real run in a scratch directory.

**Proof:** there are no unit tests; this is documentation.
- Every transcript in the chapter is produced by running the command against this checkout's binary (`cargo run -p airsl-cli -- lua setup …`) and LuaLS 3.19.2, then pasted unchanged.
- `cargo make book-build` succeeds.

**Done when:** `cargo make book-build` passes and `cargo make dod` is green. Commit `docs(book): add the editor support chapter`.

### Task 2 — Add `lua setup` to the CLI appendix

**Files:**
- Modify `book/src/appendix-01-cli.md`, in the synopsis block and two tables:
  - In the synopsis block (lines 3–10), after the `check` line: `airsl lua setup [DIR]`, aligned like its neighbours.
  - In the Commands table (lines 14–22), after `check`: `| `lua setup` | writes the LuaLS type file and the `workspace.library` entry into a project; see [Editor Support](ch01-04-editor-support.md) |`.
  - In the Exit codes table (lines 51–57), after `check`: `| `lua setup` | both files are set up or already were | any error; nothing is written when a check fails |`.

- Modify `README.md:57`: the `airsl-cli` row's command list becomes `run`, `test`, `check`, `doctor`, `ext`, `lua`.
- Modify `crates/airsl-cli/README.md`: add a row `| `airsl lua setup [DIR]` | write the LuaLS type file and luarc entry so editors complete airsl scripts |` to the command table (lines 62–68), after the `check` row.

**Change:** One synopsis line and four table rows, matching §7 (exit 0 on success, 1 on any error) and §6 (steps 1–4 write nothing). The READMEs carry no Rust example to compile.

**Proof:** `cargo run -p airsl-cli -- lua setup --help` shows the command, and the row's wording matches it.

**Done when:** `cargo make book-build` passes. Commit `docs: list lua setup in the CLI appendix and READMEs`.

### Task 3 — Update "Your Own Module" for `describe`

**Files:**
- Modify `book/src/ch07-00-your-own-module.md`.
- Modify `crates/airsl/examples/README.md` only if needed.

**Change:**
- The sentence "The trait has two methods" (line 18) becomes three methods, naming `describe` and saying in one sentence what it is for.
- The Summary bullet at line 160 ("A host module implements `HostModule`: a `name`, and an `install` that fills a Lua table.") also names `describe`.
- Both `HostModule` impls gain a `describe`: the visible `Notes` impl (lines 21–51), and the hidden `#` copy inside the second example (lines 65–113). The `rust,ignore` block at lines 139–144 is a lone `install` fragment, not an impl, so it is unchanged.
- A new short section, placed before "Summary", shows `airsl::luals::verify` in a host's own test and `airsl::luals::setup` for a host's own root (`RootTable::new("…")`).
- `crates/airsl/examples/custom-module/README.md:69` cites `src/modules/registry.rs:144` for `ModuleSet::insert`. Update the number to `insert`'s line after plan 02, found with `grep -n "pub fn insert" crates/airsl/src/modules/registry.rs`.
- `crates/airsl/examples/README.md:66` reads "Implementing `HostModule`, reading authority from `InstallContext`, and naming your own root table". Edit it only if it is no longer accurate.

**Proof:**
- Each changed Rust block becomes `src/main.rs` of a throwaway crate in the scratchpad, with `airsl = { path = "<checkout>/crates/airsl" }`, `Cargo.lock` copied in, and `cargo +1.94 run -q`. The crate is deleted after each run.
  - The first block (lines 21–51) has no `main`, so the check appends `fn main() {}` to it.
  - The second block (lines 65–113) is compiled with its hidden lines' leading `# ` removed, as rustdoc would. It is already a full program with `fn main`.
  - The new `verify`/`setup` section's block is written as a complete program, or uses the same hidden-line convention, and is checked the same way.
- Output shown in the chapter is pasted from those runs.

**Done when:** every changed block compiles and runs as above, and `cargo make book-build` passes. Commit `docs(book): describe modules in the your-own-module chapter`.

### Task 4 — Update the crate description and project guide

**Files:**
- Modify `crates/airsl-cli/Cargo.toml`: line 3's description, "Command-line runner for airsl Lua scripts: run, test, check and doctor", gains `lua setup`.
- Modify `CLAUDE.md`:
  - line 9's command list (`run`, `test`, `check`, `doctor`, `ext doctor`/`ext fire`) gains `lua setup`;
  - the extension-seam bullet at line 81 names `describe` beside `install`, in one clause saying it describes the module's Lua surface for editors and the drift check.

**Change:** Two text edits (spec §11).

**Proof:** `cargo make dod` is green, which shows the edited manifest still parses.

**Done when:** `cargo make dod` is green. Commit `docs: name lua setup and describe in the crate description and project guide`.

---

## Risks

- **Book Rust blocks are not compiled by the gate.** The Makefile's `dod` runs no `mdbook test`. Task 3's throwaway-crate compile is the only check, so skipping it would let a stale example ship.
- **The transcript in Task 1 depends on LuaLS output wording** on 3.19.2. A reader with another version may see different text. The chapter shows the command and the result, not version-specific internals.
- **Not done here:** README Rust examples. Spec §11 found no README with a `HostModule` implementation (`git grep -n "HostModule for" -- '*README*'` finds nothing).

## Proof

- `cargo make book-build` succeeds.
- `cargo make dod` is green.
- Every transcript and Rust example in the changed chapters came from a run on this checkout.

## Review findings

There was one review of Tasks 1–4: code 2🔴 1🟡 6🔵 and spec 1🔴, all from the same cause. The setup transcripts and the refusal message matched real runs. The three ch07 Rust blocks compiled and ran on 1.94. `registry.rs:170` is `pub fn insert`. Report: `.airsstack/cc/plugins/claudestacks/handoff/20261125-164553-edd2/10-reviewer-plan07.md`.

- **Fixed (🔴 ×2, plan drift):** the chapter's script had a `-- hello.lua` first line, so the LuaLS transcripts' `1:14` and `1:29` did not match a real run, which reports `2:14` and `2:29`. I dropped the comment, which also makes the script two lines as the plan says, and re-ran LuaLS 3.19.2 in a scratch dir (since deleted). The results now match the chapter: `hello.lua:1:14 … Undefined global `airsstack``, `Diagnosis completed, no problems found` after setup, and `typo.lua:1:29 … Undefined field `encod``.
- **Fixed 🔵:** "keys in alphabetical order" now reads "keys sorted". serde_json sorts by byte order.
- **Fixed 🔵:** the doctor chapter's closing "In the next chapter…" now links to A First Project and points to Editor Support first. `cargo make book-build` builds.
- **Fixed 🔵:** the ch07 `cargo test --quiet` transcript now carries cargo's blank line before `running 1 test`, pasted from a real run of the example's test.
- **Fixed 🔵:** the appendix, the CLI README and ch07 now say "description" and "the language server's settings file", as the Editor Support chapter does. ch07 says once, where it first names the file `setup` writes, that it is the description file the editor reads.
- **Fixed 🔵:** the synopsis line is `airsl lua setup  [<DIR>]`, placed beside the `ext` lines and aligned to their argument column, with `<DIR>` spelled as they spell it.
- **Fixed 🔵:** the custom-module README cites `ModuleSet::insert` and `stdlib()` by name.
- **Fixed 🟡 (Rust blocks):** the three ch07 blocks are now `crates/airsl/examples/notes-module/{notes,main,setup}.rs`, included with `rustdoc_include` anchors. `cargo make dod` compiles them, and runs the `verify` test through an `[[example]]` with `test = true`. The LuaLS transcripts stay ungated, because CI has no LuaLS.

## Deviations

- 2026-11-25. One coder did all four tasks in order, with no task-briefer pass. The tasks are text edits, and Tasks 1 and 3 share the transcript workflow.
- 2026-11-25. The Getting Started closing paragraph in `ch01-03-airsl-doctor.md`, a file not in the plan's list, was edited so that it still points the right way once the new chapter sits between it and chapter 2.
- 2026-11-25. No commit was made. The plan's per-task commits are left to the author.

## Follow-up round (2026-11-25, at the author's request)

- **Fixed:**
  - The notes-module hidden lines are reworded for readers.
  - `use std::path::Path;` now sits inside the visible anchor.
  - `examples/README.md` names `notes-module-setup` as the one example that writes into the working directory.
  - Every `file:line` citation in `crates/airsl/examples/**/README.md`, about 136 across 15 READMEs, is now a citation by name. About twenty had moved when `describe` was added, and many others, including `builder.rs:83` and `error.rs:217`, were already wrong. `grep -rnE '\.rs:[0-9]+' crates/airsl/examples --include=README.md` now finds nothing.
- **Declined** 🔵 "book-build is not in dod": CI's `book` job (`.github/workflows/ci.yml`, `mdbook build book`) already builds the book on every PR. Adding mdbook to `dod` would make every local gate need mdbook installed.
