---
status: done
created: 2026-11-23
depends-on: [01]
built-on: a4e85e09b81c
---

# Book: a module's own name and path Implementation Plan

**Goal:** Teach a reader of The airsl Book how a module uses its own name and path to require its siblings.

**Architecture:** One new section in `book/src/ch02-00-first-project.md`, the chapter where
`require` is introduced, so the fact lives in one chapter (project rule). The example and its
expected output are fixed by spec §7. The transcript comes from a real run on the checkout that
has plan 01, which is why this plan depends on it.

**Tech Stack:** mdBook (`cargo make book-build`), the `airsl` binary built from this checkout.

**Content authority:** spec §7, first bullet.

---

## Files that change

```
book/src/ch02-00-first-project.md   — modify: new section on what a module receives in `...`
```

## Order of work

### Task 1 — Add the section and its run transcript

**Files:**
- Modify `book/src/ch02-00-first-project.md`: a new section placed right after the paragraph
  that ends "`require("lib.markdown")` loads `lib/markdown.lua`.", before "## Testing it".

**Change:**
- **Content.** The section tells the reader two things, as spec §7 asks:
  - a module receives its own name and its path in `...`;
  - it can use its name to require a sibling.

  The section's own words do not mention `require`'s return values. `require` still returns only
  the module (spec §2.2). Nothing beyond spec §7 is added: no new `## Summary` bullet and no
  claims the transcript does not show.
- **Example.**
  - The two Lua files, `lib/toc.lua` and `toc.lua`, are exactly as spec §7 gives them.
  - The example is presented as **its own small folder** holding `toc.lua`, `lib/toc.lua` and
    `lib/markdown.lua`, a copy of the chapter's `markdown.lua` under `lib/`.
  - The chapter's running project (`headings.lua`, `markdown.lua` at the root, and
    `markdown_test.lua` in "## Testing it") is left unchanged, so the next section still works
    for a reader following along.
- **Transcript.**
  - Build the example folder in a scratch location.
  - From the repository root, run `cargo run -p airsl-cli -- run <absolute path to toc.lua>`.
    The output prints only the module file's basename, so the scratch location does not show.
  - Paste the console block as `$ airsl run toc.lua` followed by that run's output.
  - The output must be exactly these three lines (spec §7):
    - `-- lib.toc (toc.lua)`
    - `# Shopping`
    - `## Hardware`
  - Different output means plan 01 is wrong. Stop and report it; do not edit the text to match.
- **Voice.** Plain, short sentences for a reader using `airsl`. No issue numbers, no history, no
  internal vocabulary, no `file:line` (project rule for the book).

**Proof:**
- **Transcript check.** The pasted transcript is the real output of the run described above,
  and it matches the three lines.
- **Compile check.** From the repository root,
  `cargo run -p airsl-cli -- check <absolute path to the scratch folder>` reports every example
  file compiled, 0 failed.
- **Book build.** `cargo make book-build` builds the book without errors.

**Done when:** `cargo make book-build` succeeds and `cargo make dod` is green. Commit
`docs(book): show how a module requires its siblings through its own name`.

---

## Risks

- A transcript typed by hand instead of pasted from a run would hide a defect in plan 01. The
  task forbids it, and the expected lines are fixed in advance.
- The example's `path:match("[^/]*$")` relies on `/` separators. The spec guarantees `/` on every
  platform (spec §2.1), so the same text holds for a Windows reader.

## Proof

- `cargo make book-build` green.
- The transcript in the chapter equals the three lines fixed by spec §7.

## Review findings

- 🔵 No DoD step checks the chapter's example — declined: book examples are run by hand by project convention; plan 01's `a_required_module_receives_its_name_and_path_as_its_arguments` and `a_module_reaches_its_sibling_through_its_own_name_from_either_root` guard the behaviour.
- 🔵 "The name is the one the caller wrote" inexact for a file reached under two names — declined: the chapter never introduces `init.lua`; the sentence is exact for every case a reader of the chapter meets.
- 🔵 "The path always uses `/`" not shown by the transcript — declined: the sentence is what the example's `path:match("[^/]*$")` relies on, and the plan's Risks rest on it.
- 🔵 "from the last section" ambiguous — fixed: "from the previous section".
- 🔵 copied `lib/markdown.lua` keeps its `-- markdown.lua` header — declined: the plan asks for a copy.
- Verified: `cargo run -q -p airsl-cli -- run <scratch>/book-toc/toc.lua` → `-- lib.toc (toc.lua)` / `# Shopping` / `## Hardware`; `check <scratch>/book-toc` → `3 file(s) compiled, 0 failed`; `cargo make book-build` exit 0; `cargo make dod` green.

## Deviations

- 2026-11-23 — No commit made (the execute instruction did not say to commit).
- 2026-11-23 — Run in parallel with plan 01's final review (disjoint files; plan 01's code was complete and its DoD green).
