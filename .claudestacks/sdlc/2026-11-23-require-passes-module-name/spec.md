---
status: approved
created: 2026-11-23
built-on: 3302a1fa5305
---

# Spec: A required module receives its own name and path

The confined `require` (the one every surface below `Full` gets when the script has a root) runs a
module's chunk with no arguments. This spec makes it call the chunk with `(name, path)`, the two
arguments Lua 5.4's own `require` passes. A module can then require its siblings by a name
relative to its own, which lets one module load both through an entry script and under
`airsl test`. `require` itself keeps returning one value. A module that returns nothing now yields
`true` on its first `require`, not only on later ones. §2.4 lists the only changes a script can
observe. Ships as `airsl` 0.1.5 and `airsl-cli` 0.1.5. Tracked as airsstack/airsl#17.

## 1. Evidence

Every behaviour claim below cites one of these files in `probes/`. Each was run on 2026-11-23
against this checkout at `1a926b3` (`airsl 0.1.4`, built with `cargo build -p airsl-cli`) or against
the exact `mlua` version in `Cargo.lock`.

| File | What it is |
|---|---|
| `01-airsl-0.1.4-behaviour.sh` / `.txt` | Fixture script and raw output of the 0.1.4 binary: sections P1–P9 |
| `02-mlua-0.12.0-approaches.rs` / `.Cargo.toml` / `.txt` | Throwaway crate on `mlua = "=0.12.0"` (`lua54, vendored, serialize, send`, the workspace's features), comparing four ways to run a chunk. Approaches A and B call the chunk with `("pkg.mod", "/abs/m.lua")`; the issue's fix calls it with `"pkg.mod"`. Its `Cargo.lock` resolves `mlua` 0.12.0 (last two lines of the `.txt`) |
| `03-lua-5.4-manual-require.html.txt` | Lua 5.4 manual §6.3 `require`, lines 8701–8721 of `https://www.lua.org/manual/5.4/manual.html`, fetched 2026-11-23 |
| `04-mlua-0.12.0-chunk-eval.txt` | `mlua-0.12.0/src/chunk.rs:585-602` (`Chunk::eval`) and `:725` (`fn to_expression`, not `pub`) |

Workspace pins: `Cargo.toml:20` declares `mlua = { version = "0.12", features = ["lua54",
"vendored", "serialize", "send"] }`. `Cargo.lock:472-473` resolves `mlua` `0.12.0`.

## 2. The behaviour

### 2.1 What a module receives

The chunk is called with exactly two arguments:

1. **The name**: the `require` target exactly as the caller wrote it (`RequireTarget::as_str()`),
   with no normalisation. `require("lib")` and `require("lib.init")` pass different names even
   though both resolve to `lib/init.lua` (`crates/airsl/src/types/require_target.rs:77-83`,
   `candidates`).
2. **The path**: the resolved file's path as a script sees it. This is the same string the
   chunk name carries after its `@`: the canonical, absolute path that `resolve` returns
   (`require_loader.rs:207`, canonicalised and verbatim-stripped). It is rendered by
   `rules::native::to_script_string` (`crates/airsl/src/paths/rules.rs:289-291`), which applies
   the host flavour to `to_script_string` (`rules.rs:196-202`): unchanged on unix, `/` separators
   and no `\\?\` prefix on Windows. A path with non-UTF-8 bytes is rendered lossily
   (`to_string_lossy`, `rules.rs:197`), exactly as the chunk name already is.

Why: Lua 5.4 "calls the loader with two arguments: `modname` and an extra value, a *loader
data* … if the loader came from a file, this extra value is the file path" (`03`, lines 1–11).
Stock Lua under `--policy trusted` gives a module two arguments (`01` P4: `module argc=2`).

On 0.1.4 the module receives none (`01` P3: `count=0`). That is the defect: the issue's idiom
indexes a nil value (`01` P1: `core/hooks/enforce.lua:1: attempt to index a nil value`).

### 2.2 What `require` returns

Always exactly one value. The path is never a second return value.

| Call | 0.1.4 (`01` P4, confined) | This spec |
|---|---|---|
| module returns a value `v`, first load or cache hit | `v` (n=1) | `v` (n=1) |
| module returns nothing, first load | `nil` | **`true`** |
| module returns nothing, later load | `true` | `true` |
| module returns `false` | `false` | `false` |
| module returns several values | the first (`01` P8: `n=1 type=table`) | the first |

- **Why one value (author's decision, after review):** stock Lua also returns the path as a
  second value (`03`, lines 19–20; `01` P4 stock: `first n=2`). In a multi-value position that
  second value spills over. In stock Lua, `{ require(ma), require(mb) }` gives `parts=3`, and
  `table.insert({}, require(multi))` fails with ``bad argument #2 to 'table.insert' (number
  expected, got table)`` (`01` P8 stock). The same code on 0.1.4's confined `require` gives
  `parts=2` and `ok=true` (`01` P8 confined). Returning the path would break such scripts. A
  module that needs its own path reads it from `...` instead.
- **Nil fix:** 0.1.4 returns the module's raw value on the first load and the recorded `true`
  after (`require_loader.rs:175-184`: `Ok(value)` is returned while `recorded` is stored). This
  spec returns the recorded value on the first load too. That matches stock Lua (`01` P4 stock:
  `nil module: first=true second=true`) and airsl's own cache hit.
- **Several return values:** only the first is kept, unchanged. In `02` (row "two return values"),
  `return { v = 1 }, 'extra'` gives `table v=1` under 0.1.4's `eval` and under approach A, so
  converting the call's result to one `Value` drops the rest.

### 2.3 How the chunk is evaluated: expression first

The chunk is compiled the way 0.1.4 compiles it, expression first, then called with §2.1's
arguments:

1. Compile `"return " .. source` under the chunk name. If it compiles, that function is the
   module.
2. Otherwise compile `source` as written. If that fails, its error (not step 1's) is the error.
3. Call the function with `(name, path)`, converting the result to one `Value`.

This is `Chunk::eval`'s order (`04`: "first try interpreting the lua as an expression by adding
`return`, then as a statement"), which 0.1.4 uses (`require_loader.rs:202`, `.eval::<Value>()`).
mlua's own prefix is exactly `b"return "` (`mlua-0.12.0/src/chunk.rs:765`,
`expression_source`). The order is rebuilt here, not called, because `eval` takes no arguments
and `to_expression` is private (`04`, line `fn to_expression`).

Checked examples. All seven rows come from `02` (`eval (0.1.4)` vs `A expression-first`):

| Module source | 0.1.4 | This spec | Why this row matters |
|---|---|---|---|
| `return (...)` | `Nil` | `"pkg.mod"` | the fix; parentheses keep only the name |
| `return select('#', ...)` | `0` | `2` | argument count; see §2.4 |
| `...` | `Nil` | `"pkg.mod"` | a bare-expression module can read its name too |
| `{ v = 'bare' }` | `v = "bare"` | `v = "bare"` | must not regress; the issue's `.call()` gives `syntax error … near '{'` |
| `tostring(1)` | `"1"` | `"1"` | both an expression and a statement; statement-first would give `Nil` |
| `local x = \n` | `syntax error: m.lua:2: unexpected symbol near <eof>` | same text | the error is the statement parse's; statement-first would report `m.lua:1: … near 'local'` |
| `return { v = 1 }, 'extra'` | `table v=1` | `table v=1` | several values: the first is kept |

Binary chunks cannot reach this step. The module is read with `read_to_string`
(`require_loader.rs:194-199`), and a Lua 5.4 bytecode header is not valid UTF-8 (`02`:
`first 12 bytes: [1b, 4c, 75, 61, 54, 00, 19, 93, …]`,
`String::from_utf8: Err("invalid utf-8 sequence of 1 bytes from index 7")`). So the read fails with
`Error::ScriptRead` before compilation, as on 0.1.4. A module holding byte `0x93` fails that way today
(`01` P7: ``cannot read script `…/fx/notutf8.lua`: stream did not contain valid UTF-8``).

### 2.4 Compatibility

A script can observe exactly two changes. Both are compatible under the CHANGELOG's own rule
(`CHANGELOG.md:3-6`), so this ships as a `z` bump.

1. **Inside a module, `...` is no longer empty.** `select('#', ...)` goes from `0` to `2` (`02`).
   On 0.1.4, `...` in a module was always empty (`01` P3: `count=0`), so this changes only what a
   module reads from `...`. That is the fix itself, and it is what stock Lua already passes
   (`01` P4: `module argc=2`).
2. **A module that returns nothing yields `true` on its first `require`** (0.1.4: `nil`). §2.2.

Everything else is unchanged:

- **`require`'s result count:** one value (§2.2), so a `require` in a multi-value position
  behaves as on 0.1.4 (`01` P8 confined).
- **Resolution and containment:** `resolve` is untouched. It searches `<stem>.lua`, then
  `<stem>/init.lua` under the root, and refuses an escape.
- **Caching:** one load per canonical path, kept across evaluations of a reused engine
  (`require_loader.rs:145-165`). One file under two names runs once, and the module sees the
  first name it was required under (`01` P4: `runs=1 same=true` for `lib` then `lib.init`). A
  `false`-returning module runs once (`01` P4 confined: `runs=1`), unlike stock Lua (`runs=2`).
- **Cycle detection** and the in-progress marker (`require_loader.rs:156-172`).
- **Which surfaces get the confined loader:** `RequireDisposition::decide` is untouched. `pure`
  still has no `require` (`01` P6: `pure: nil`), and `trusted` keeps Lua's own (`01` P6).
- **The main script** still receives no `...`. It is evaluated by `Engine::eval_to` with
  `.eval::<T>()` and no arguments (`engine.rs:246-249`). `airsl run badargs.lua x y` gives
  `main script select('#', ...)=0` (`01` P9).

## 3. Approach

Chosen: **A, expression first, then call** (§2.3). Rejected, each against `02`:

| Option | Rejected because |
|---|---|
| The issue's `.call::<Value>(target)` | a bare-expression module becomes a syntax error (`{ v = 'bare' }`, `...`) |
| B, statement first, expression as fallback | `tostring(1)` returns `Nil` (0.1.4: `"1"`); a syntax error reports `:1: … near 'local'` (0.1.4: `:2: … near <eof>`) |

Only A matches 0.1.4 on every row except the new arguments.

## 4. Architecture

Approved by the author on its own, 2026-11-23. The later decision that `require` returns one
value removes the path from `require`'s result. It adds nothing to the tree below.

### 4.1 Module tree

No new files. One module changes.

```text
crates/airsl/src/
 ├─ engine.rs                 unchanged   set_require() picks the disposition (engine.rs:280-289)
 ├─ require_loader.rs         CHANGED     the confined require: resolve → cache → run
 │    ├─ RequireLoader::install   unchanged shape: the Lua function returns one value
 │    ├─ load                     the first load returns the recorded value (nil → true)
 │    ├─ run                      builds the script path once, for the chunk name and the argument
 │    └─ compile                  NEW private fn: §2.3 steps 1–2, returns the mlua::Function
 ├─ paths/rules.rs            unchanged   rules::native::to_script_string reused for the path
 └─ types/require_target.rs   unchanged   RequireTarget::as_str() is the name
```

- **Why the change stays in `require_loader.rs`:** it is part of the file's one concept, running
  the confined `require`. A separate file for `compile` would be a `<concept>_<qualifier>` file
  beside its concept. The file grows by tens of lines of code plus tests, so no folder is needed.
- **Why `compile` exists as its own function:** compilation is a separate phase from reading and
  calling, and it has its own tests for every row of §2.3's table. It returns
  `mlua::Result<mlua::Function>`. `run` maps its error with `Error::lua(target, …)`, as today.
- **One string for both uses:** `run` computes `let script_path = to_script_string(path)` once.
  The chunk name is `format!("@{script_path}")` and the second argument is `script_path`, so the
  two cannot differ.
- **The module doc** (`require_loader.rs:1-14`) keeps its four-part shape. `Responsibilities:`
  gains one bullet: running a module, expression first, called with its name and path, the
  arguments Lua's own `require` passes.

### 4.2 Dependency direction (unchanged, no new crates)

```text
engine ──► require_loader ──► paths::{containment, rules::native}
                         ├──► types::RequireTarget
                         ├──► error::Error
                         └──► mlua (Function, Value)
```

### 4.3 Rust guideline rules applied (`claudestacks-guideline-rust:rust-guidelines` 0.1.5)

| Reference | Applied as |
|---|---|
| `modularity.md` Rule 1 | `require_loader.rs` keeps one responsibility; compilation is extracted as a named, testable phase |
| `modularity.md` Rule 2 | no new type; reuses `RequireTarget`, `to_script_string`, `Error::lua` |
| `mod-rs-export-only.md` | no `mod.rs`/`lib.rs` change; the sibling file's `//!` block keeps what / why / responsibilities / non-responsibilities |
| `static-dispatch.md` | concrete `mlua::Function`; no `dyn` |
| `strong-types.md` | no public signature changes, and no new private tuple |
| `unit-test-mandate.md` | every row of §2.2 and §2.3 and every item of §2.4 has a test in `require_loader.rs`'s own `mod tests` (§6) |
| `doc-comment-discipline.md` | no issue, spec or plan references in source; comments say why expression-first is rebuilt (the `eval` / `to_expression` constraint) and why `require` returns one value |
| `strict-quality.md` | `cargo make dod` green; no new `#[allow]` |
| `workspace.md` | member versions and the root `[workspace.dependencies]` `airsl` version move together; publish `airsl` before `airsl-cli` |
| `microsoft-guidelines.md` | `M-DOCUMENTED-MAGIC`: the `"return "` prefix carries a comment naming its origin (`Chunk::eval`'s order) |

## 5. Errors

No error is added, removed or reworded. None of these runs the module, so none passes it
arguments.

Refusals the loader raises, with their messages from `crates/airsl/src/error.rs`:

| Case | Variant | Message (error.rs line) |
|---|---|---|
| invalid target (`""`, `a/b`, `..`, over 128 bytes) | `InvalidName` | ``invalid {kind} `{value}`: {reason}`` (196) |
| module file unreadable or not UTF-8 | `ScriptRead` | ``cannot read script `{path}`: {source}`` (207) |
| resolves outside the root | `RequireEscape` | ``module `{module}` resolves outside the script directory `{root}` `` (217) |
| requires itself | `RequireCycle` | ``module `{module}` requires itself, directly or indirectly, under `{root}` `` (226) |
| not found | `RequireNotFound` | ``module `{module}` not found under `{root}` `` (303) |
| syntax error or runtime error in the module | `Lua` | `lua error in {chunk}: {source}` (24), where `chunk` is the target and `source` is the statement parse's error (§2.3) |

Refusals mlua raises while converting the argument, before the loader runs
(`require_loader.rs:116-117`, `target: mlua::LuaString`, `to_str()?`). Text from `01` P9:

| Argument | Message |
|---|---|
| a table | `bad argument #1: error converting Lua table to string (expected string or number)` |
| `nil` | `bad argument #1: error converting Lua nil to string (expected string or number)` |
| a non-UTF-8 string | `error converting Lua string to &str (invalid utf-8 sequence of 1 bytes from index 0)` |
| the number `1` | coerced to `"1"`, then ``module `1` not found under `…` `` |

A failed load still clears the in-progress marker (`require_loader.rs:186-189`), so a later
`require` re-runs the module instead of reporting a cycle.

## 6. Tests

Inline in `crates/airsl/src/require_loader.rs` `mod tests`, through `Engine` and `Script::from_file`
over `tempfile` fixtures, as the existing tests do. The expected path is computed the way `resolve`
computes it (`canonicalize`, `strip_verbatim`, `to_script_string`), so the tests hold on Windows.

| Test | Asserts |
|---|---|
| `a_required_module_receives_its_name_and_path_as_its_arguments` | in the module, `select('#', ...) == 2`, the first is the target as written, the second equals the expected path |
| `require_returns_exactly_one_value_on_the_first_load_and_on_a_cache_hit` | `select('#', require("m"))` is `1` on both calls, and both return the same table |
| `a_require_in_a_multi_value_position_contributes_one_value` | `{ require("a"), require("b") }` has 2 elements; `table.insert({}, require("a"))` succeeds |
| `a_module_reaches_its_sibling_through_its_own_name_from_either_root` | the issue's three files, each run on **its own fresh `Engine`** (the cache is per engine and keeps the first name, §2.4; `airsl test` builds one engine per file, `crates/airsl-cli/src/test_runner.rs:129`): from root `R` (entry `main.lua`) the result is `lib=ok base=[core.hooks.]`; from a script in `R/core/hooks` requiring `enforce` it is `lib=ok base=[]` |
| `a_module_written_as_a_bare_expression_still_loads` | `{ v = 'bare' }` returns a table with `v == "bare"` |
| `a_module_that_is_both_an_expression_and_a_statement_returns_the_expression` | `tostring(1)` makes `require` return `"1"` |
| `a_syntax_error_in_a_module_reports_the_statement_parse` | a module `local x = \n` fails with `Error::Lua` whose text contains `:2: unexpected symbol near <eof>` |
| `a_module_returning_nothing_yields_true_on_its_first_require` | first and second `require` both return `true`, one value each |
| `a_module_returning_false_yields_false_and_runs_once` | both `require`s return `false`; a counter the module bumps reads `1` |
| `a_module_returning_several_values_yields_only_the_first` | `return { v = 1 }, 'extra'`: `select('#', require("m")) == 1` and the value is the table |
| `a_file_reached_under_two_names_runs_once_and_sees_the_first_name` | `require("lib")` then `require("lib.init")` on one engine: the module ran once, both return the same table, and the table's recorded name is `lib` |

`compile` also gets its own unit tests, one per row of §2.3's table (all seven). Each calls the
compiled function with `("pkg.mod", "/abs/m.lua")` and checks the result against the `A
expression-first` value in `02`.

All 37 existing tests matching `require` in `airsl` keep passing unchanged
(`cargo test -q -p airsl --lib require` on 0.1.4: `test result: ok. 37 passed; 0 failed`). No public
item changes, so no doctest is added.

Done when `cargo make dod` is green.

## 7. Documentation

- **The book, `book/src/ch02-00-first-project.md`**, after the paragraph that ends "`require("lib.markdown")`
  loads `lib/markdown.lua`.": a short section telling the reader that a module receives its own
  name and path in `...`, and showing how to require a sibling with it. Its example compiles on
  0.1.4 (`01` P5: `3 file(s) compiled, 0 failed`):

  ```lua
  -- lib/toc.lua
  local name, path = ...
  local base = name:match("^(.-)[^.]*$")
  local markdown = require(base .. "markdown")

  local M = {}

  function M.print(text)
    print("-- " .. name .. " (" .. path:match("[^/]*$") .. ")")
    for _, heading in ipairs(markdown.headings(text)) do
      print(heading)
    end
  end

  return M
  ```

  ```lua
  -- toc.lua
  require("lib.toc").print("# Shopping\n## Hardware\n")
  ```

  `lib/markdown.lua` is the chapter's `markdown.lua` moved under `lib/`. The transcript is pasted
  from a run of `airsl run toc.lua` on the fixed checkout, never written by hand (project rule:
  every book example is run against the current checkout). By §2.1, that run must print exactly
  these lines, and a different output means the fix is wrong:

  ```text
  -- lib.toc (toc.lua)
  # Shopping
  ## Hardware
  ```

  The section follows the book's voice: no issue numbers, no history, no internal vocabulary.
- **The book's version transcripts**, `ch01-01-installation.md:23` and `ch01-03-airsl-doctor.md:8,42`
  (`airsl 0.1.4`), are re-run and re-pasted after the bump.
- **Kept at 0.1.4 on purpose:**
  - the pinned-install examples: `install/install.sh:5`, `install/install.ps1:6`,
    `book/src/ch01-01-installation.md:33`, `book/src/appendix-04-installation.md:21`,
    `crates/airsl-cli/README.md:18`;
  - the installers' "expected something like 0.1.4" hints: `install/install.sh:54`,
    `install/install.ps1:34`;
  - the CI pin: `.github/workflows/ci.yml:175-176,204,207`;
  - the README's tag illustration: `README.md:79`.

  Each names a release that is already published, or is an example of the version format.
- **`CHANGELOG.md`:** the `## Unreleased` section becomes `## airsl 0.1.5 — airsl-cli 0.1.5 — <date>`,
  where `<date>` is the date of the release commit, as the earlier entries use. It gains a
  `### Fixed` entry: a module loaded by `require` now receives its name and path in `...`, so it
  can require its siblings by a name relative to its own, and a module that returns nothing
  yields `true` on its first `require` as on later ones. The existing `### Changed` entries stay.
  Per §2.4 this is a compatible change, so it ships as a `z` bump.

## 8. Release

- `crates/airsl/Cargo.toml:4` and `crates/airsl-cli/Cargo.toml:4` go to `0.1.5`. The root
  `Cargo.toml:52` goes to `airsl = { version = "0.1.5", path = "crates/airsl" }`. `Cargo.lock` is
  updated by the build.
- Both crates are released together (org rule: the binary's lockfile pins the library, so
  publishing only the library reaches nobody).
- `cargo make publish-dry-run` (`Makefile.toml:203-226`) passes before handoff.
- **Out of the chain's hands:** pushing the tags `airsl-v0.1.5` and `airsl-cli-v0.1.5`, and
  `cargo publish -p airsl` then `-p airsl-cli`, happen after merge, on the author's go. These
  actions are outward-facing. The `airsl-cli-v*` tag drives `.github/workflows/airsl-cli-release.yml`.

## 9. Non-goals

- `require` under `pure` / `Minimal` (airsstack/airsl#12): a separate chain. `decide` is untouched
  (§2.4).
- Returning the path from `require` as a second value (§2.2).
- Lua's own `require` under `Full`, which already passes both arguments (`01` P4).
- `...` in the main script (§2.4).
- Normalising module names (`lib` vs `lib.init`) or keying the cache by name. The cache stays
  keyed by canonical path (§2.4).
- Re-running a `false`-returning module as stock Lua does (§2.4).
- Pinning the chunk mode to text. Binary chunks are already unreachable (§2.3).
- Bumping agentstacks (`airsl = "0.1.4"` in its root `Cargo.toml:15`). That happens in its own repo
  after 0.1.5 is published.
