---
status: approved
created: 2026-11-25
built-on: ae381756f52b
---

# Type File Return Implementation Plan

**Goal:** The rendered LuaLS type file ends by returning the root table.

**Architecture:**
- `TypeFile::render` (`crates/airsl/src/luals/render.rs:84-123`) pushes one final block,
  `return <root>`, after every module's blocks.
- Blocks are joined with a blank line and the text ends with `\n` (`render.rs:117-118`), so the
  file ends `…\n\nreturn <root>\n`. That is the byte tail recorded in
  `probes/airsl-check-typefile-with-return.txt`.
- The header stays `---@meta`, unnamed (spec D5).
- The layout bullet in the module doc (`render.rs:12-13`) and the `render` rustdoc
  (`render.rs:76-79`) both gain the final block.
- `luals/setup.rs` writes whatever `render` returns (`setup.rs:172`), so it is unchanged.

**Tech Stack:** Rust 1.94, crate `airsl`, cargo-make.

**Content authority:** spec §1 (D5), §6, §8 (`luals/render.rs`, `luals/setup.rs`).

---

## Files that change

```
crates/airsl/src/luals/render.rs   — modify  final `return <root>` block; module doc; tests
```

## Order of work

### Task 1 — End the type file with `return <root>`

**Files:**
- Modify `crates/airsl/src/luals/render.rs`.

**Change:** after the module loop, `render` appends the block `return {root_name}`, so it is
joined like every other block. Both descriptions of the layout gain the new block:
- the module doc's layout bullet (`render.rs:12-13`): "…and its functions, then `return <root>`";
- the `render` rustdoc (`render.rs:76-79`): the file ends by returning the root table, which is
  what lets LuaLS type `require("<root>")`.

**Proof:**
- `the_type_file_ends_by_returning_the_root`: the default root's text ends with
  `"\n\nreturn airsstack\n"`. A root of `myapp` ends with `"\n\nreturn myapp\n"`.
- `omits_empty_docs` (`render.rs:362-374`): its expected tail, which ends
  `function airsstack.m.f(x) end\n`, gains `\nreturn airsstack\n` after that line. It still pins
  the whole end of the file, and its `!text.contains('#')` assertion is unchanged.
- The `luals/setup.rs` tests compare with `TypeFile::render` (`setup.rs:507`) and pass unedited.

**Done when:** `cargo make dod-crate airsl` passes and the Definition of Done is green. Commit
`feat(airsl): the type file returns the root table`.

---

## Risks

- A project set up by an earlier build gets a changed type file the next time `airsl lua setup`
  runs. That is intended: the file always matches the airsl that wrote it.
- LuaLS behaviour is pinned to 3.19.2 by the probe (`probes/luals-require.out`). The gate does
  not run LuaLS.

## Proof

- `cargo make dod` is green.
- `cargo test -p airsl luals::` passes.
