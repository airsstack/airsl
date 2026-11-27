---
status: done
created: 2026-11-26
depends-on: [07, 08, 10, 11]
built-on: af7c0dcc7368
---

# Repository Prose Implementation Plan

**Goal:** The repository's own prose describes the workspace as it now stands: four members, one
shared version, a gate that loads the native module, and releases that ship rocks.

The prose in question is the three READMEs, the CHANGELOG, `CLAUDE.md` and the values-and-json
example.

**Architecture:** The READMEs are short front doors into the book, so they state facts and link to
the book's stock-Lua chapter for detail. The lockstep release rule (S16) is written where releases
are described:
- root `README.md` § Releases;
- `CHANGELOG.md`'s preamble;
- the two crate READMEs' Releases sections.

`CLAUDE.md` does not describe releases, so it gains nothing for that rule (§2.5).

The `### Breaking` item of §2.5 goes into the CHANGELOG now, under a new `## Unreleased` heading
(S23). That is where Keep a Changelog 1.1.0 keeps changes not yet released, and the CHANGELOG
declares that format (`CHANGELOG.md:3-4`; `probes/240-keep-a-changelog-1.1.0.txt`). The heading has
no `[…]` link brackets, because no heading in the file uses them (probe 240). The maintainer who
prepares the release renames it to the release's `## airsl <version> — airsl-cli <version> —
<date>` line.

The S17 maintainer step joins the README's maintainer list, because that list is the repository's
release procedure (§11).

`CLAUDE.md` changes last. docs-check reads its code spans against the code, so every module path and
snake-case name it cites must already exist. A name that lives only outside `.rs`/`Cargo.toml` files,
such as `cargo_extra_args`, is written in prose, not in a code span (§11).

**Tech Stack:** Markdown, crate `docs-check` (its citation and example-output guards), the
`values-and-json` example (`cargo run -p airsl --example values-and-json`).

**Content authority:** spec §1.2 S2, S7, S16, S17, S23, §2.1 (`CHANGELOG.md`, `CLAUDE.md`,
`README.md`, `examples/values-and-json/`), §2.3 (last sentence), §2.5 ("Where the rule is written",
the `### Breaking` item and "Where the item is written"), §2.9 D2, D5, §5.6.5 (the rocks task's
variables), §9.3 (`rocks` row), §10.4, §11 (rows for `values-and-json`, `CHANGELOG.md` and the crate
READMEs, both root `README.md` rows, and `CLAUDE.md`).

---

## Files that change

```
CHANGELOG.md                                    — modify  preamble (:7-9) states one shared version and lockstep releases; + ## Unreleased with the ### Breaking item
crates/airsl/README.md                          — modify  Releases (:101-102): one shared version, lockstep releases
crates/airsl-cli/README.md                      — modify  Releases (:94-97) the same; command table gains --native
README.md                                       — modify  crate table, build and gate needs, Development, Releases, maintainer list, S17 release step
crates/airsl/examples/values-and-json/main.rs   — modify  the LuaJIT comment (:63-64) corrected
crates/airsl/examples/values-and-json/README.md — modify  the LuaJIT bullet (:49-51) corrected; ## Output unchanged
CLAUDE.md                                       — modify  members, build requirement, commands, docs-check's five checks, architecture bullets
```

## Order of work

### Task 1 — Write the lockstep release rule and the breaking change

**Files:**
- Modify `CHANGELOG.md`:
  - rewrite the preamble paragraph at `:7-9` ("The two crates are versioned independently …"). The
    crates `airsl` and `airsl-cli`, and the native module's rocks, share one version. Every release
    is a lockstep release of both crates, tagged `airsl-v<version>` and `airsl-cli-v<version>` on the
    same commit, and there is no library-only release (S16). Older entries keep their per-crate
    headings.
  - add `## Unreleased` (written so, without brackets) between the preamble (`CHANGELOG.md:1-10`)
    and the newest entry, `## airsl 0.2.0 — airsl-cli 0.2.0 — 2026-11-25` (`:12`), holding a
    `### Breaking` sub-heading. Its item is the one §2.5 words: `airsl` gains Cargo features, so
    `default-features = false` now needs `lua54` or `luajit`, and `Engine` is `Send + Sync` only with
    the default `send` feature (D2).

    Edges (§2.5): if a `## Unreleased` section already exists when this lands, the item joins it,
    under its `### Breaking` sub-heading, created if absent; there is never a second `## Unreleased`,
    and an empty one is never written. No other entry text is added. The release writes the rest.
- Modify `crates/airsl/README.md` (`:101-102`) and `crates/airsl-cli/README.md` (`:94-97`): replace
  "versioned independently" with one shared version, lockstep releases, and one changelog.

**Change:** The written release rule matches what docs-check enforces (plan 07), and the breaking
feature change is recorded where the release will find it.

**Proof:** No test.
- `grep -n "^## " CHANGELOG.md` shows `## Unreleased` once, as the first `## ` heading after the
  preamble and above `## airsl 0.2.0 — airsl-cli 0.2.0 — 2026-11-25`, and `grep -n "^### Breaking"
  CHANGELOG.md` shows the sub-heading under it.
- `grep -rn "versioned independently" CHANGELOG.md README.md crates/*/README.md` finds nothing after
  this task and Task 2.
- `cargo make dod` (docs-check) is green.

**Done when:** `cargo make dod` is green. Commit `docs: one shared version and lockstep releases`.

### Task 2 — Update the root README and the CLI README for the native module

**Files:**
- Modify `README.md`:
  - **"What is in this repository"** (`:54-57`): the crate table gains `airsl-lua`, the native
    module shipped as the LuaRocks packages `airsstack` and `airsstack-luajit`, not on crates.io.
  - **Build needs** (`:59-61`): building from source still needs a C compiler, and the native module
    also builds for LuaJIT. Link the book's stock-Lua chapter with an absolute
    `https://airsstack.github.io/airsl/…` URL.
  - **Development** (`:66`, `:72-74`): `cargo make dod` also needs `lua5.4` and `luajit` on `PATH`,
    or `AIRSL_LUA54`/`AIRSL_LUAJIT` (D5). Add a line for `cargo make rocks`, naming what it also
    needs: the network, luarocks on `PATH` or `AIRSL_LUAROCKS` (§5.6.5), and
    `AIRSL_LUA54_DIR`/`AIRSL_LUAJIT_DIR` when luarocks must be pointed at an interpreter's directory
    (§9.3).
  - **Releases** (`:78-79`): one shared version, both tags pushed on the same commit, no
    library-only release (S16).
  - **Release flow** (`:83-85`): an `airsl-cli-v*` tag also builds the rocks before the GitHub
    Release and uploads them to luarocks.org after it.
  - **Maintainer list:**
    - name `.github/workflows/build-rocks.yml` and `.github/workflows/publish-rocks.yml`, both
      written by hand;
    - name the `LUAROCKS_API_KEY` secret and its prerequisite: a luarocks.org account without
      upload two-factor (§10.4).
  - **The S17 step**, owned by the maintainer who pushes the `airsl-cli-v*` tag. After the first
    release whose `publish-rocks` job succeeds, they run against luarocks.org:
    - `luarocks --lua-version 5.4 install --local airsstack <version>`;
    - `luarocks --lua-version 5.1 install --local airsstack-luajit <version>`;
    - `airsl lua setup --native`, on macOS, on Linux, and for the hint on Windows.

    They paste the output into `book/src/ch01-05-stock-lua.md`'s output-less sections and merge that
    as a `docs:` pull request.
- Modify `crates/airsl-cli/README.md`: the `airsl lua setup [DIR]` row of the command table (`:67`)
  shows `[--native [--lua 5.4|luajit]]` and says it can also install the native module with
  LuaRocks.

**Change:** A contributor reading the README knows what `dod` needs and how rocks are released. A
maintainer has the post-release step.

**Proof:** No test. docs-check's citation guard reads `README.md` code spans. Any `.rs` path cited
must exist and carry no line suffix.

**Done when:** `cargo make dod` is green. Commit `docs: describe airsl-lua, the gate's interpreters
and the rock release`.

### Task 3 — Correct the values-and-json LuaJIT claim

**Files:**
- Modify `crates/airsl/examples/values-and-json/main.rs` (`:63-64`): replace the comment "On 5.1 or
  LuaJIT this would read `1.7e9`" with the true difference. LuaJIT encodes `1700000000` as Lua 5.4
  does. The difference is a float such as `2^53`, which encodes as `9007199254740992` on LuaJIT and
  `9007199254740992.0` on Lua 5.4 (`probes/149-luajit-json-number.txt`). Keep the assertion
  unchanged.
- Modify `crates/airsl/examples/values-and-json/README.md` (`:49-51`): correct the bullet the same
  way, keeping why airsl pins 5.4. Leave the `## Output` block unchanged.

**Change:** The example no longer claims a LuaJIT behaviour that does not happen (S7).

**Proof:**
- docs-check's example-output guard still passes. The README's `## Output` equals what the example
  prints, because the program's output is unchanged.
- `cargo run -p airsl --example values-and-json` prints the same output as before.

**Done when:** `cargo make dod` is green. Commit `docs(airsl): correct the values-and-json LuaJIT
claim`.

### Task 4 — Bring `CLAUDE.md` up to date

**Files:**
- Modify `CLAUDE.md`:
  - **"What this is"** (`:7-10`): a fourth member, `crates/airsl-lua`, unpublished on crates.io and
    shipped as LuaRocks rocks.
  - **Build requirement** (`:12-16`): `dod` also needs `lua5.4` and `luajit`, or
    `AIRSL_LUA54`/`AIRSL_LUAJIT`.
  - **Commands:**
    - native-load runs inside `test`;
    - `cargo make rocks` is outside `dod`, with its variables: `AIRSL_LUAROCKS`, `AIRSL_LUA54_DIR`
      and `AIRSL_LUAJIT_DIR`, beside the interpreters' `AIRSL_LUA54` and `AIRSL_LUAJIT`;
    - a `--workspace` command, an editor's check included, must `--exclude airsl-lua`, while bare
      cargo commands work through `default-members` (F15).
  - **docs-check's list** (`:37`, "these four things"): becomes five, with a bullet for the
    rockspec/version guard. The rockspec names and their `package`, `version` and `tag`, and the
    published members' versions, follow `[workspace.package] version`.
  - **Architecture:**
    - the VM line (`:91`): `lua54,vendored,serialize,send` becomes airsl's default feature set, with
      `luajit` and `module` used only by the native module;
    - the `modules::stdlib()` bullet (`:122-123`): a new module is registered in `standalone()`,
      unless it exists only inside airsl, like `ext`. Name `modules::standalone` beside it;
    - the `Engine` bullet (`:134`): `Send + Sync` with the default `send` feature;
    - a bullet for `crates/airsl-lua`. It covers `ModuleSet::open` and the one-expression entry in
      `crates/airsl-lua/src/entry.rs`, and the check in `crates/airsl-lua/lua/check.lua` (a `.lua`
      path, which docs-check does not read);
    - a bullet for the `crates/airsl-cli/src/lua/` folder (`command.rs`, `native.rs`,
      `luarocks.rs`).
  - **Code spans:** write `luaopen_airsstack` in a code span only because `entry.rs`'s `//!` holds
    it (plan 04). Write `cargo_extra_args` in prose, never in a code span (§11).
  - **No line-number suffix** on any cited path.

**Change:** `CLAUDE.md` matches the code and the gate, and docs-check holds it to that.

**Proof:**
- `cargo make dod-crate docs-check` exits 0. Its module-path and lone-name guards accept every new
  code span, and a misspelt one fails there.
- `cargo make dod` is green.

**Done when:** `cargo make dod-crate docs-check` exits 0 and `cargo make dod` is green. Commit
`docs(claude): describe the native module, features and the five docs-check guards`.

---

## Risks

- The `## Unreleased` heading is new to this CHANGELOG, whose entries so far were written at release
  (probe 240: no Unreleased section before). The maintainer who prepares the release that ships this
  change renames it to its version line and keeps the `### Breaking` item (§2.5); nothing automates
  the rename.
- `CLAUDE.md` code spans fail docs-check when a name moves. That is the intended guard, and Task 4
  runs it last for that reason.
- The S17 step is a written procedure, not automation. Nothing reminds a maintainer to run it
  besides the README list.

## Proof

- `cargo make dod` is green, including docs-check's citation, module-path and example-output
  guards.
- `grep -rn "versioned independently" CHANGELOG.md README.md crates/*/README.md` finds nothing.

## Review findings

(reviewer verdict: non-compliant, code 1🔴 3🟡 5🔵 1❓ · spec 1🟡, DoD green (`CARGO_BUILD_JOBS=2 cargo make dod` exit 0; `cargo make dod-crate docs-check`, 98 passed), blocking: C1, S1, C2)
- C1 🔴 `CLAUDE.md` "What this is": "All of them share the one version in `[workspace.package]`" was false (`crates/docs-check/Cargo.toml` is `0.0.0`; docs-check's rockspecs guard covers only airsl, airsl-cli, airsl-lua) — fixed: the shared-version sentence is dropped; the rockspec bullet names airsl, airsl-cli, airsl-lua; `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → exit 0, 98 passed; `CARGO_BUILD_JOBS=2 cargo make dod` → exit 0.
- C2 🟡 `README.md` maintainer list, first bullet: "The same tag also builds the rocks" read as the `airsl-v*` tag just said to start nothing — fixed: now "The `airsl-cli-v*` tag also builds the rocks"; `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → exit 0, 98 passed; `CARGO_BUILD_JOBS=2 cargo make dod` → exit 0.
- C3 🟡 `CHANGELOG.md` `## Unreleased` › `### Breaking`: the item named `lua54`/`luajit` and `send` but not `vendored`, so a `default-features = false` dependent with only `lua54` gets mlua-sys 0.11.0's pkg-config probe for a system Lua — fixed: the item adds that `default-features = false` also needs `vendored`, or mlua looks for a system Lua through pkg-config, checked against `crates/airsl/Cargo.toml` `[features]` (default = lua54, vendored, send); `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → exit 0, 98 passed; `CARGO_BUILD_JOBS=2 cargo make dod` → exit 0.
- C4 🟡 `crates/airsl/examples/values-and-json/encode.lua`, comment on `observed_at`: "would encode as `1.7e9`" was false and contradicted the corrected main.rs comment — fixed: the comment now says `1700000000.0`; `airsl run` printed `{"a":1700000000,"b":1700000000.0}`; example output unchanged; `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → exit 0, 98 passed; `CARGO_BUILD_JOBS=2 cargo make dod` → exit 0.
- C5 🔵 `README.md` S17 maintainer step: "the output-less sections of `book/src/ch01-05-stock-lua.md`" was vague and paired output with command lines the chapter does not show — fixed: the step names the "Installing" blocks (`airsl lua setup --native`, the two `luarocks install --local` lines), the Homebrew lines and the Windows lines, and says each output goes under its own command line; `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → exit 0, 98 passed; `CARGO_BUILD_JOBS=2 cargo make dod` → exit 0.
- C6 🔵 `README.md` "What is in this repository": the LuaJIT clause sat under the C-compiler "because" — fixed: split into two sentences; `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → exit 0, 98 passed; `CARGO_BUILD_JOBS=2 cargo make dod` → exit 0.
- C7 🔵 `CLAUDE.md` airsl-lua bullet: `rockspecs/` was said to hold "packages" — fixed: it holds "the two rockspecs, the recipes for the two LuaRocks packages"; `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → exit 0, 98 passed; `CARGO_BUILD_JOBS=2 cargo make dod` → exit 0.
- C8 🔵 `CLAUDE.md` Engine bullet: the added line was 106 columns — fixed: rewrapped to 100; `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → exit 0, 98 passed; `CARGO_BUILD_JOBS=2 cargo make dod` → exit 0.
- C9 🔵 diff-wide: no reversion guard fails if the LuaJIT comment/bullet, the removed "versioned independently" or CLAUDE.md naming `native-refusals` is reverted — fixed for the CLAUDE.md part: the seventh docs-check check (`crates/docs-check/src/tasks.rs`) requires `CLAUDE.md` to write every step of `dod` and of its dependencies as `cargo make <step>`; with the sentence's `cargo make native-refusals` removed the check prints `CLAUDE.md: never names `native-refusals`, which `cargo make dod` runs through `test``, and `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → exit 0, 167 passed. The LuaJIT comment/bullet and the removed "versioned independently" stay unguarded: the plan accepts "No test" for them.
- C10 ❓ `README.md` maintainer list: no step told the maintainer to rename `## Unreleased` at release — fixed: a maintainer step now says to rename `## Unreleased` to `## airsl <version> — airsl-cli <version> — <date>` before tagging; `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → exit 0, 98 passed; `CARGO_BUILD_JOBS=2 cargo make dod` → exit 0.
- S1 🟡 plan Architecture ("`CLAUDE.md` does not describe releases, so it gains nothing for that rule"): CLAUDE.md "What this is" added "a release is a lockstep release of `airsl` and `airsl-cli`", an unauthorized addition — fixed: the lockstep/shared-version sentence is dropped from CLAUDE.md; the rockspec bullet names airsl, airsl-cli, airsl-lua; `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → exit 0, 98 passed; `CARGO_BUILD_JOBS=2 cargo make dod` → exit 0.
- chain open finding — CLAUDE.md names `native-refusals` in the gate: closed by this plan (CLAUDE.md now says the `test` step runs `test-cargo`, `native-load`, `native-refusals`, matching Makefile.toml).

- 🟢 (fix-pass review, report 20 area) `CLAUDE.md`'s docs-check paragraph after the sixth check — checked: "six things" matches the checks (examples, `.rs` paths, module paths, includes, rockspecs, book links), the new bullet matches `book_links.rs` (scanned files, chapter/SUMMARY rule, `install/` rule, root/`index.html` with the anchor caveat), and "It does not check … anything else written in backticks" still holds — evidence: reviewer read against the code; `CARGO_BUILD_JOBS=2 cargo make dod` → exit 0.
- 2026-11-27 findings-fix pass: CI round 3 (run 45942172171) all 10 ci.yml jobs green on ubuntu/macos/windows; local `CARGO_BUILD_JOBS=2 cargo make dod` exit 0.

## Deviations

- 2026-11-27 — `CLAUDE.md` gained a paragraph describing the `dod` steps (clippy and doc per ABI, `test` = `test-cargo` + `native-load` + `native-refusals`, `test-doc` skipping airsl-lua, `rocks` outside dod, per Makefile.toml); this closes the chain's open finding.
- 2026-11-27 — `CLAUDE.md`'s docs-check sentence changed from "four things" to "five things", with a rockspec/version bullet (Task 4).
- 2026-11-27 — C4 edited `crates/airsl/examples/values-and-json/encode.lua`, a file Task 3 does not list: its comment's `1.7e9` claim was false (a Lua 5.4 float encodes as `1700000000.0`, probed with `airsl run`) and contradicted the corrected main.rs comment in the same example.
- 2026-11-27 — C3 added the `vendored` clause to the Breaking item beyond the plan's wording (mlua-sys 0.11.0 probes pkg-config for a system Lua without it).
- 2026-11-27 — C10 added a maintainer step the plan's Architecture/Risks describe but no task wrote.
- 2026-11-27 — Execution process: one coder ran tasks 1–4; no briefer spawns.
- 2026-11-27 — `CLAUDE.md`'s docs-check sentence now says "six things", not the "five things" of Task 4, with a bullet for the book-link check (`crates/docs-check/src/book_links.rs`, plan 11's F11) beyond the spec's five; the bullet names the scanned files (CLAUDE.md, CHANGELOG.md, the READMEs, `.rs` files outside docs-check) and the root/`index.html`/`print.html`/`404.html` rule.
- 2026-11-27 — The workspace version was bumped to 0.3.0 (`Cargo.toml`, `Cargo.lock`, both rockspecs renamed to `-0.3.0-1`, the `ch01-05-stock-lua.md` transcripts re-run) because 0.2.0 had already been released on 2026-11-25.
- 2026-11-27 — The seventh docs-check check, `tasks` (`crates/docs-check/src/tasks.rs`), has no basis in the spec: it was added to close C9. It requires every `cargo make <task>` in `CLAUDE.md`, the READMEs and the book chapters to name a task `Makefile.toml` declares, and `CLAUDE.md` to write every step of `dod` as `cargo make <step>`; `CLAUDE.md`'s docs-check sentence now says "seven things" with a bullet for it. Spec §2.5's "0.2.0 until release" is overtaken by the 0.3.0 bump recorded above.
