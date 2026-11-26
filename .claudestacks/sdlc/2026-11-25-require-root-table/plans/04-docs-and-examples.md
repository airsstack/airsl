---
status: done
created: 2026-11-25
depends-on: [02, 03]
built-on: ae381756f52b
---

# Docs And Examples Implementation Plan

**Goal:** Every book chapter, example and project note states what `require("<root>")` now does.

**Architecture:**
- The book is written for a reader using `airsl` (`CLAUDE.md`, Documentation). It uses plain short
  sentences, cites no `file:line`, and records no history.
- A fact lives in one chapter. `ch02-00-first-project.md` holds the `require("airsstack")` form,
  and the other chapters link to it.
- Every Lua snippet, CLI transcript and example output is run against the checkout built from
  plans 01–03, and the output is pasted, never retyped. Example outputs come from
  `cargo run -p airsl --example <name>`.
- Wording in every row follows spec §3 and §5. Messages are quoted exactly as `error.rs` prints
  them.

**Tech Stack:** mdBook sources under `book/src/`, the `airsl` examples, `airsl` CLI built from the
checkout, LuaLS 3.19.2 (`~/.local/share/nvim/mason/bin/lua-language-server`).

**Content authority:** spec §9 (every row), §3 and §5 (the wording), §6 (the editor behaviour).

---

## Files that change

```
crates/airsl/examples/multi-file-project/main.rs          — modify  module doc :8-9, :13-14; comment :52-56; FROM_MEMORY :24-34 and comment :77-78
crates/airsl/examples/multi-file-project/which_require.lua — modify  flavour doc and branch :22-30; root=yes|no on the row line :70-77
crates/airsl/examples/multi-file-project/README.md        — modify  prose :5-13, :78-101; output block from a run
crates/airsl/examples/policy-presets/README.md            — modify  prose :66-71; output block if the run changes it
crates/airsl/examples/engine-reuse/README.md              — modify  prose :78-79; output block if the run changes it
book/src/ch02-00-first-project.md                         — modify  new section after the require section; summary list line
book/src/ch03-05-presets.md                               — modify  :54 and the table row :71
book/src/appendix-03-presets.md                           — modify  row :17
book/src/appendix-06-surfaces.md                          — modify  :77-79
book/src/ch06-02-scripts-and-values.md                    — modify  :50-56
book/src/ch07-00-your-own-module.md                       — modify  :59
book/src/ch08-03-hosting.md                               — modify  row :74
book/src/ch01-04-editor-support.md                        — modify  the require form, with a --check transcript
CLAUDE.md                                                 — modify  one Key seams line
```

Line numbers are those on the commit that approves this plan. Plan 01 changes only paths in these
files, so the cited passages keep their place.

## Order of work

### Task 1 — Bring the `require` examples up to date

**Files:**
- Modify `crates/airsl/examples/multi-file-project/main.rs`, `which_require.lua` and `README.md`.
- Modify `crates/airsl/examples/policy-presets/README.md` and
  `crates/airsl/examples/engine-reuse/README.md`.

**Change:**
- `multi-file-project/main.rs`:
  - The module doc (`:8-9`) says `pure`'s `require` refuses every call except the root table's
    name until the member is granted. `:13-14` names the four answers: refusing, confined, Lua's
    own and root-only.
  - The comment at `:52-56` matches.
  - `FROM_MEMORY` (`:24-34`) prints whether `require("airsstack")` is the global, and the message
    `require("lib")` raises. The script catches it with `pcall` and prints `tostring` of the error
    up to its first newline, so no traceback reaches the output. Its doc comment and the comment
    at `:77-78` describe a `require` that answers only the root.
- `which_require.lua`:
  - `refused_outright`'s doc (`:14-16`): the refusal is raised for every name except the root
    table's, so `../secrets` still shows it.
  - `flavour`'s doc (`:22-23`) lists refusing, confined, Lua's own and root-only.
  - `flavour`'s `require == nil` branch is removed, because no disposition leaves the global
    `nil`.
  - `parent_target`'s doc (`:41-43`): the refusing `require` refuses every name but the root
    table's. Its `require == nil` branch (`:45-47`) is removed for the same reason.
  - The row line (`:70-77`) gains `root=yes|no` after `require=…`. `yes` means
    `require("airsstack")` returned the global table.
- `multi-file-project/README.md`: the output block is pasted from
  `cargo run -p airsl --example multi-file-project`. The prose changes:
  - `:5-13`: `pure`'s `require` answers the root name.
  - `:78-101`: the four values with `RootOnly` in place of "the global is cleared". The `memory`
    row now prints a function. The `pure` row still refuses `../secrets` without resolving it,
    and answers only the root. The `require=absent` paragraph is replaced by what the `memory` row
    prints.
- `policy-presets/README.md` `:66-71`: `pure`'s `require` refuses every call except
  `require("airsstack")`, and a script from source gets a `require` that answers only the root.
  Run `cargo run -p airsl --example policy-presets`. If the output changed, paste the new block.
- `engine-reuse/README.md` `:78-79`: a confined script from source gets a `require` that answers
  only the root. Run `cargo run -p airsl --example engine-reuse`. If the output changed, paste the
  new block.

**Proof:** each example's README output block is byte-identical to its `cargo run` output.
`cargo test -p airsl --all-targets` compiles the examples.

**Done when:** each of the three `cargo run -p airsl --example …` runs matches its README, and
`cargo make dod` is green. Commit `docs(examples): require answers the root table`.

### Task 2 — Teach the `require("airsstack")` form in the book

**Files:**
- Modify `book/src/ch02-00-first-project.md`, `ch03-05-presets.md`, `appendix-03-presets.md`,
  `appendix-06-surfaces.md`, `ch06-02-scripts-and-values.md`, `ch07-00-your-own-module.md` and
  `ch08-03-hosting.md`.

**Change:**
- `ch02-00-first-project.md`: after the `require` section (`:120-170`), add a short section. It
  says `local airsstack = require("airsstack")` works under every preset and returns the same
  modules as the global. A file named `airsstack.lua` beside the script is never loaded by that
  name. It carries one small example, run with `airsl run` and its output pasted. The summary
  list (`:241`) gains one line.
- `ch03-05-presets.md` `:54` and table row `:71`, `appendix-03-presets.md` row `:17`, and
  `appendix-06-surfaces.md` `:77-79`: `pure`'s `require` refuses every call except
  `require("airsstack")` until the `require` member is granted. Each links to the ch02 section
  rather than restating it.
- `ch06-02-scripts-and-values.md` `:50-56`: a script built with `Script::from_source` has a
  `require` that answers only `require("airsstack")`. Any other name fails with
  ``module `lib` cannot be loaded: the script has no directory to load modules from``.
  `.with_root(dir)` gives it a folder.
- `ch07-00-your-own-module.md` `:59`: with the root renamed, scripts also write
  `require("myapp")`.
- `ch08-03-hosting.md` row `:74`: the root name is also what `require` returns.

**Proof:** every new or changed Lua snippet and transcript is run against the checkout, and its
output is pasted. `cargo make book-build` builds the book without errors.

**Done when:** every snippet's pasted output matches a fresh run, the book builds, and
`cargo make dod` is green. Commit `docs(book): require("airsstack") returns the modules`.

### Task 3 — Show the `require` form in the editor and record the seam

**Files:**
- Modify `book/src/ch01-04-editor-support.md` and `CLAUDE.md`.

**Change:**
- `ch01-04-editor-support.md`: after the misspelled-field transcript (`:49-61`), add a short
  passage. A script that writes `local airsstack = require("airsstack")` is checked the same way.
  It shows a `lua-language-server --check . --checklevel=Information` transcript in which a
  misspelled field on that local is reported. Run it in a fresh folder after `airsl lua setup`
  from the checkout, and paste the output. One sentence says completion follows from the same
  type. There is no completion transcript, because `--check` cannot produce one.
- `CLAUDE.md` Key seams gains one line: `require("<root>")` is answered by `require::root` before
  any other resolution, and through `package.loaded` on `Full`.

**Proof:** the transcript is pasted from a LuaLS 3.19.2 run against a type file written by the
checkout's `airsl lua setup`.

**Done when:** the transcript matches a fresh run and `cargo make dod` is green. Commit
`docs: type the require form in editors and record the require seam`.

---

## Risks

- **Riskiest step: the example output blocks.** They depend on plan 02's exact messages, so they
  are pasted only after plans 02 and 03 are in the tree (`depends-on`).
- Line numbers drift if an earlier task edits the same file. Plan 01 changes only paths there, so
  the cited passages stay where they are.
- The book is published on push to `main` before a release carries this behaviour. That is
  accepted for the roadmap: no release per milestone.

## Proof

- `git grep -n "require=absent\|no \`require\` at all\|refuses every call until\|refuses every call with\|before the name is looked at\|there is no require to call\|fails before it looks at the name\|refuses every call\.\|refuses every$" -- book crates/airsl/examples`
  prints nothing. On the commit that approves this plan it prints ten lines:
  `appendix-06-surfaces.md:78`, `ch03-05-presets.md:54`, `engine-reuse/README.md:79`,
  `multi-file-project/README.md:70`, `:91` and `:98`, `multi-file-project/main.rs:8`,
  `which_require.lua:15` and `:46`, and `policy-presets/README.md:69`. Every hit is a passage
  Task 1 or Task 2 rewrites. New wording must say "refuses every call except …", which matches
  none of these phrases.
- `cargo make dod` is green, and `cargo make book-build` builds.

## Review findings

Reviewer, 2026-11-25: 0 🔴, 3 🟡, 6 🔵 code + 1 🔵 plan. Blocking: ch06-02:51 and ch06-02:55-58. All fixed in one round. Final: `cargo make dod` exit 0; `cargo make book-build` exit 0; `cmp` of fresh `cargo run -q -p airsl --example <name>` against the README Output block identical for multi-file-project, policy-presets and engine-reuse; proof grep over `book` and `crates/airsl/examples` → no output.

- 🟡 ch06-02:51, the `pure` sentence still said every `require` is refused: fixed, now "refused, except `require("airsstack")`".
- 🟡 ch06-02:55-58, the from-source claim had no policy condition: fixed, qualified by policy and linked to ch02 (see Deviations).
- 🟡 multi-file-project heading "one name, three functions": fixed, now "four functions"; README block re-pasted, `cmp` identical.
- 🔵 ch01-04:83, the link pointed at the whole chapter: fixed, now `#requiring-the-modules-by-name`.
- 🔵 ch02-00:146-147, uneven reflow: fixed.
- 🔵 ch02-00:215, "including `pure`" read as covering the snippet's `print`: fixed, the claim is now about the call.
- 🔵 ch07-00:61: fixed, "can also write".
- 🔵 multi-file-project/main.rs:28-29, omitted the `require` grant: fixed.
- 🔵 policy-presets/README.md:73-75, the from-source script runs `require("lib")`: fixed, `cmp` identical.
- 🔵 appendix-03:17, the row had no link to ch02: fixed, linked.
- 🔵 No automated guard against docs drift: fixed on 2026-11-25 at the author's request. `crates/docs-check` fails the gate when:
  - an example's output differs from its README `## Output` block;
  - a cited source path, module path or `:line` citation is stale or present;
  - a book `{{#include}}`, `{{#rustdoc_include}}` or `{{#playground}}` target is missing;
  - an example's `no-output` or `unix-only` marker file is not named in its expected list in `crates/docs-check/src/examples.rs`;
  - the `examples` task of `Makefile.toml` compares output with anything but the book appendix.

  Bite proof: changing "doubles to 6" to "doubles to 7" in the engine-reuse README fails with "README drift: example engine-reuse: its output differs…".

## Deviations

- 2026-11-25: The task-briefer step was skipped. Tasks 1–3 ran in three parallel coders (disjoint files), each running the gate once at the end.
- 2026-11-25: Task 2's wording for ch06-02 ("a script built with `Script::from_source` has a `require` that answers only `require("airsstack")`") is unconditional, and the code shows it holds only below `Full` with `require` granted. A probe crate against the checkout evaluated `pcall(require,"lib")` from source: trusted → `module 'lib' not found:` (Lua's own require); confined → ``module `lib` cannot be loaded: the script has no directory to load modules from``; pure → ``require denied: `require` is not granted — no access is granted``. The chapter was written qualified by policy. This is plan wording, not a spec claim.
- 2026-11-25: `which_require.lua`'s `flavour` doc lists four kinds of `require` but notes that root-only never reaches a file-backed script; the code cannot tell it from confined.
- 2026-11-25: The policy-presets README now counts four kinds of answer (it said three); the `memory` row is the fourth.
- 2026-11-25: The ch01-04 LuaLS transcript has its ANSI escapes and `\r` progress redraws stripped, matching the chapter's existing transcripts.
- 2026-11-25: Not committed; the commit waits for the author.
- 2026-11-25: Scope addition, signed off by the author ("fix all of findings"; then chose "unpublished member crate" and "deviation in plan 04"). A new workspace member `crates/docs-check` (`publish = false`, tests only) joins the `cargo make dod` gate. It checks example output against README `## Output` blocks, cited `.rs` paths, `CLAUDE.md` module paths and lone code names, refuses `:line` citations, and checks book include, rustdoc_include and playground targets, the marker files of the examples against their expected lists, and that the `examples` task compares output with the appendix only. It came from three reversion-guard findings this chain had first declined (plans 01, 02, 04). Side effects: 11 `CLAUDE.md` citations now carry `crates/airsl/src/…`; `denials-are-data/README.md` cites symbols instead of line numbers; the duckscript `examples` task no longer repeats the output check. Reviewed in two rounds; findings fixed.
