---
status: done
created: 2026-11-26
depends-on: [01, 06]
built-on: af7c0dcc7368
---

# Rockspec Version Guard Implementation Plan

**Goal:** docs-check fails whenever the rockspecs or a published member's version drift from the
workspace version.

**Architecture:** A fifth guard, `crates/docs-check/src/rockspecs.rs`, is compiled for tests only
like the rest of the crate. Its pure function takes the repository root and returns one problem
line per finding, empty when all is well. This is the shape of the citation guards
(`crates/docs-check/src/citations/source.rs`, `problems`). It is the owner of every check in spec
§5.6.3, run by the `test` step of `dod`.

The guard reads three kinds of input, each by the rule §5.6.3 gives it:
- **Member manifests** (S21): `crates/airsl/Cargo.toml`, `crates/airsl-cli/Cargo.toml` and
  `crates/airsl-lua/Cargo.toml`, read with `toml` into a `toml::Value` and classified by the six
  steps of §5.6.3 "Member manifests", the first that applies.
- **The root manifest:** `[workspace.package].version` of `Cargo.toml`. When it cannot give a
  version, the rockspec checks are skipped and the member checks still run.
- **The rockspecs directory** (S22, S24): every entry of `crates/airsl-lua/rockspecs/`. A name
  starting with `.` is skipped with no message. Every other name is classified by the six numbered
  steps of §5.6.3 "How a rockspec file name is split": split from the right at its last two `-`.
  Only a current rockspec is opened, after `fs::metadata` shows a regular file; its lines are read as
  text, the first line starting `package = `, `version = ` and `   tag = ` (§3.4).

A TOML error is shown in form B (S24): toml's first `Display` line, then `: `, then the reason line,
which is the last non-empty line after the first; the first line alone when the `Display` is one
line. One helper builds it for the root and the members alike.

Every message is the §5.6.3 text, read from the spec, never reworded. The guard over the real tree
is one test; the rest run on temporary trees.

**Tech Stack:** Rust 1.94, crate `docs-check` (test-only), `toml` 1.2.2+spec-1.1.0 (workspace
dependency; `Cargo.lock:844-845`), `tempfile` (existing dev-dependency), std's `fs::metadata`,
`Metadata::is_file`, `fs::read_to_string`, `std::os::unix::fs::symlink` (unix tests). Cargo runs with
`CARGO_BUILD_JOBS=2` (`CLAUDE.md`).

**Content authority:** spec §1.2 S8, S16, S21, S22, S24, §1.3 (toml), §2.1 (docs-check
`Cargo.toml`, `src/lib.rs`, `src/rockspecs.rs`), §2.2 (`docs-check/src/rockspecs.rs` row), §2.5
("What enforces it"), §2.10 M2, M11, §3.4, §5.6.3, §5.6.7 (the docs-check rows), §6.4 (rows 1–2 and
the hidden-entry, file-type and TOML rows), §9.1 (`rockspecs.rs` tests).

---

## Files that change

```
crates/docs-check/Cargo.toml        — modify  + toml = { workspace = true } under [dev-dependencies]
Cargo.lock                          — modify  toml joins docs-check's dependency list (written by cargo)
crates/docs-check/src/rockspecs.rs  — create  the fifth guard and its tests
crates/docs-check/src/lib.rs        — modify  + #[cfg(test)] mod rockspecs; crate docs list the fifth guard
```

## Order of work

### Task 1 — Classify the member manifests

**Files:**
- Modify `crates/docs-check/Cargo.toml`: add `toml = { workspace = true }` under
  `[dev-dependencies]`. The declaration is at root `Cargo.toml:48`. The section comment already
  says why the dependencies are dev-only.
- Modify `Cargo.lock`: commit what cargo writes for the new dependency edge.
- Modify `crates/docs-check/src/lib.rs`:
  - add `#[cfg(test)] mod rockspecs;` in the file's order, so this task's tests compile;
  - add a `rockspecs` bullet to the crate doc's `Responsibilities:`: rockspec file names, their
    `package`/`version`/`tag`, and the published members' versions all follow the workspace version.
- Create `crates/docs-check/src/rockspecs.rs`:
  - its `//!` follows the house shape:
    - what it guards: rockspec names and contents, and member versions, follow the workspace
      version;
    - why it is its own module: it reads the root manifest, three member manifests and a directory
      no other guard reads;
    - `Responsibilities:`;
    - `Non-responsibilities:` whether a rockspec builds (that is `cargo make rocks`, and the rock
      workflow), and the keys of a member manifest other than `version` (cargo's to check).
  - `problems(root)`, the guard's one entry, returning the problem lines. In this task it reads the
    three member manifests, each with `fs::read_to_string` (an error gives `{path}: could not be
    read: {io error}`), then classifies it by §5.6.3 "Member manifests", the first that applies:
    1. not valid TOML → `{path}: is not valid TOML: {toml error}`;
    2. no `package` key, or `package` is not a table → `{path}: has no `[package]` table, so it
       cannot inherit the workspace version (`version.workspace = true`)`;
    3. `[package]` has no `version` key → `{path}: declares no version; it must inherit the workspace
       version (`version.workspace = true`) so airsl, airsl-cli and airsl-lua share one number`;
    4. `version` is a table holding exactly one key, `workspace`, whose value is the boolean `true`
       → no message (the dotted and the inline form read the same, probe 238);
    5. `version` is a string, an empty one included → the `sets its own version, which must come
       from the workspace …` line;
    6. any other value → `{path}: has a version that is {type}; it must be `version.workspace = true`
       so airsl, airsl-cli and airsl-lua share one number`, `{type}` named from
       `Value::type_str()` by the §5.6.3 table (`integer` → `an integer`, `float` → `a float`,
       `boolean` → `a boolean`, `datetime` → `a date-time`, `array` → `an array`, `table` →
       ``a table other than `{ workspace = true }` ``).
  - `toml_error_line`, the form-B helper: the error's `Display` split with `str::lines`; its first
    line, then `: `, then the last non-empty line after the first; the first line alone when there
    is no such line.
  - in its `tests` module, a fixture helper that writes a whole valid tree into a temporary
    directory: a root `Cargo.toml` with `[workspace.package] version = "0.2.0"`, the three member
    manifests inheriting it, and both current rockspecs with their `package`, `version` and `tag`
    lines on lines 2, 3 and 6 (§8.1 layout). Every test of this plan starts from it and changes one
    input, so the inputs a test does not touch give no line at any task of this plan.

**Change:** A member that does not inherit the workspace version fails docs-check with one line
that names its shape. A member that is not valid TOML gets one line holding toml's position and its
reason, never the source excerpt.

**Proof:** Each test is written first and seen failing (the module does not exist), on the fixture
tree with one member manifest changed:
- `a_member_that_does_not_inherit_the_workspace_version_is_reported`: `crates/airsl/Cargo.toml`
  with `version = "0.2.0"` gives exactly the §5.6.3 `crates/airsl/Cargo.toml: sets its own version …`
  line.
- `a_member_without_a_package_table_is_reported`: a manifest with `[lib]` only gives the "has no
  `[package]` table" line.
- `a_member_without_a_version_is_reported`: a `[package]` with no `version` gives the "declares no
  version" line.
- `a_member_version_of_another_type_is_named_by_its_type`: `version = 1` gives "has a version that is
  an integer"; `version.workspace = false` gives ``has a version that is a table other than
  `{ workspace = true }` `` (probe 238's breaker).
- `an_inline_workspace_table_counts_as_inheriting`: `version = { workspace = true }` gives no line.
- `a_member_that_is_not_valid_toml_shows_the_position_and_the_reason`: a member holding `[package`
  gives the one line ``crates/airsl/Cargo.toml: is not valid TOML: TOML parse error at line 1,
  column 9: unclosed table, expected `]` `` (probe 249).
- `a_toml_error_without_a_position_is_shown_alone`: a member whose `[package]` holds a dotted key of
  100 parts gives `crates/airsl/Cargo.toml: is not valid TOML: recursion limit`, with no `: ` after
  it (probe 249).

**Done when:** `cargo test -p docs-check rockspecs` passes and `cargo make dod` is green. Commit
`test(docs-check): require every published crate to inherit the workspace version`.

### Task 2 — Read the workspace version and classify each rockspecs entry by name

**Files:**
- Modify `crates/docs-check/src/rockspecs.rs`. `problems` also:
  1. reads the root `Cargo.toml`, reporting `Cargo.toml: could not be read: {io error}`,
     `Cargo.toml: is not valid TOML: {toml error}` (through `toml_error_line`) or `Cargo.toml: has
     no string `version` under `[workspace.package]`, which the rockspecs follow`. On any of these
     the rockspec checks below are skipped; the member checks of Task 1 still run.
  2. lists `crates/airsl-lua/rockspecs`, reporting `crates/airsl-lua/rockspecs: could not be listed:
     {io error}`.
  3. takes each entry's name with `to_string_lossy` and skips, with no message, every name that
     starts with `.`, whatever kind of entry it is (S24).
  4. classifies every other entry by name, the first that applies (§5.6.3, steps 1–5):
     1. the name does not end `.rockspec`, case-sensitive → `{path}: is not a rockspec; only the
        two rockspecs live here`;
     2. with `.rockspec` stripped, split from the right at the last two `-` (revision after the
        last, version between the two, package before); fewer than two `-`, an empty package or
        version, or a revision that is not one or more ASCII digits →
        ``{path}: is not named `<package>-<version>-<revision>.rockspec` ``;
     3. the package is neither `airsstack` nor `airsstack-luajit`, compared exactly →
        `{path}: names package {package}, which is neither airsstack nor airsstack-luajit`;
     4. the version is not the workspace version, compared as strings → `{path}: names version
        {version}, which is not the workspace version {workspace}; rename it rather than keep a
        copy`;
     5. the revision is not `1`, compared as strings → `{path}: has revision {revision}; a release
        publishes revision 1 only`;
     6. otherwise it is the current rockspec of its package, handed to Task 3's step.
  5. reports `crates/airsl-lua/rockspecs: has no `<package>-<version>-1.rockspec`, which the
     workspace version <version> needs; a version bump renames both rockspecs` for each package
     with no current entry, `airsstack` first. A current name counts as present whatever kind of
     entry it is.

**Change:** A version bump without renaming, a leftover copy, a stray file, a name that does not
split, a second revision or a third package each fail docs-check with one line naming the file. A
`.DS_Store` or `.gitkeep` is ignored.

**Proof:** Each test is written first and seen failing, on the fixture tree with the root manifest
or the rockspecs directory changed:
- `a_workspace_version_that_is_missing_or_not_a_string_is_reported`: a root manifest with no
  `[workspace.package].version`, and one where it is an integer, each give the "has no string
  `version`" line and no rockspec line.
- `a_missing_rockspec_is_reported`: with workspace version `0.3.0` and only
  `airsstack-0.2.0-1.rockspec` present, the problems include the "has no
  `airsstack-0.3.0-1.rockspec` …" line and the "names version 0.2.0, which is not the workspace
  version 0.3.0 …" line (§6.4 row 1).
- `a_leftover_rockspec_of_another_version_is_reported`: `airsstack-0.1.0-1.rockspec` beside the
  current pair gives exactly the "names version 0.1.0, which is not the workspace version 0.2.0;
  rename it rather than keep a copy" line.
- `a_revision_other_than_one_is_reported`: `airsstack-0.2.0-2.rockspec` gives the "has revision 2"
  line.
- `an_unknown_package_is_reported`: `other-0.2.0-1.rockspec` gives the "names package other" line.
- `a_file_that_is_not_a_rockspec_is_reported`: `notes.txt` gives the "is not a rockspec" line.
- `a_rockspec_name_that_does_not_split_is_reported`: `airsstack.rockspec`, `airsstack-0.2.0.rockspec`,
  `airsstack-luajit-0.2.0.rockspec`, `airsstack--1.rockspec` (an empty part) and
  `airsstack-0.2.0-x.rockspec` (a revision that is not digits) each give the
  ``is not named `<package>-<version>-<revision>.rockspec` `` line (probe 239 (a)).
- `the_luajit_rockspec_name_splits_as_package_airsstack_luajit`: with workspace `0.2.0` and only
  `airsstack-luajit-0.2.0-1.rockspec` present, the only line is the "has no
  `airsstack-0.2.0-1.rockspec`" line; the LuaJIT file gets no "names package" or "names version"
  line.
- `a_hidden_entry_is_skipped`: `.DS_Store`, `.gitkeep`, `.rockspec` and a hidden directory beside
  the current pair give no line; a directory holding only those four gives the two "has no" lines,
  `airsstack` first (probe 245).

**Done when:** `cargo test -p docs-check rockspecs` passes and `cargo make dod` is green. Commit
`test(docs-check): require the rockspec names to follow the workspace version`.

### Task 3 — Check a current rockspec's file type before reading it

**Files:**
- Modify `crates/docs-check/src/rockspecs.rs`. For a current rockspec (Task 2, step 6), before it is
  opened (§5.6.3 step 6, S24):
  - `fs::metadata` (which follows a symlink) failing → `{path}: could not be read: {io error}`;
  - anything but a regular file (`Metadata::is_file` false: a directory, a FIFO, a socket, a
    symlink to one of those) → `{path}: is not a regular file, so it cannot be a rockspec`, and the
    entry is never opened;
  - a regular file is read with `fs::read_to_string`; its error, content that is not UTF-8
    included, gives `{path}: could not be read: {io error}`.

  The same calls run on every OS, so the outcome does not depend on how an OS fails to read a
  directory (probes 245, 251).

**Change:** A directory, FIFO or symlink to a directory under a current name gets one line and is
never opened, so a FIFO cannot block `dod`. A dangling symlink or non-UTF-8 content gets the
"could not be read" line.

**Proof:** Each test is written first and seen failing:
- `a_current_name_that_is_a_directory_is_not_a_regular_file` (every OS): a directory named
  `airsstack-0.2.0-1.rockspec` gives exactly the "is not a regular file, so it cannot be a rockspec"
  line and no "has no" line for `airsstack`.
- `a_current_name_that_is_a_fifo_is_reported_without_being_opened` (`#[cfg(unix)]`): a FIFO named
  `airsstack-luajit-0.2.0-1.rockspec`, made with the `mkfifo` command through
  `std::process::Command` as probe 245's fixtures did on macOS and probe 251's on Linux, gives the
  "is not a regular file" line, and the test finishes (an open would block on a FIFO with no
  writer).
- `a_dangling_symlink_is_reported_as_unreadable` (`#[cfg(unix)]`): a symlink under a current name
  pointing nowhere gives a line starting `{path}: could not be read: `; the test asserts that
  prefix, not the OS's text.
- `a_rockspec_that_is_not_utf8_is_reported_as_unreadable`: a regular file under a current name
  holding byte `0xff` gives `{path}: could not be read: stream did not contain valid UTF-8`
  (probe 245; the same on Windows, probe 251).

**Done when:** `cargo test -p docs-check rockspecs` passes and `cargo make dod` is green. Commit
`test(docs-check): check a current rockspec's file type before opening it`.

### Task 4 — Check the lines inside each current rockspec and guard the real tree

**Files:**
- Modify `crates/docs-check/src/rockspecs.rs`. For each current rockspec read in Task 3, at the
  1-based line where each key is first found, it reports:
  - `{path}:{line}: declares `package = "…"`, which does not match its file name; luarocks refuses
    such a rockspec`;
  - `{path}:{line}: declares `version = "…"`, which does not match its file name; luarocks refuses
    such a rockspec`, where the expected value is `<version>-1`;
  - `{path}:{line}: declares `tag = "…"`, which is not the release tag `airsl-cli-v<version>``.

  A key with no line gives `{path}: declares no `{key} = "…"` line`, `{key}` being `package`,
  `version` or `tag`; a file missing several keys gets one line each, in the order `package`,
  `version`, `tag` (§5.6.3, S21). An entry that Task 2 or Task 3 reported is never line-checked.
- Add the guard over the real tree, which asserts that `problems(workspace::root())` is empty.

**Change:** A rockspec renamed without editing its contents, or one naming another tag, fails
docs-check at the offending line. The real repository is held to every check of this plan.

**Proof:** Each test is written first and seen failing:
- `a_package_line_that_disagrees_with_the_file_name_is_reported`: `package = "airsstack-lua"` in
  `airsstack-0.2.0-1.rockspec` gives the `:2:` line of §5.6.3.
- `a_version_line_that_disagrees_with_the_file_name_is_reported`: `version = "0.1.0-1"` gives the
  `:3:` line. This is the case luarocks refuses (`probes/202-final-rockspecs.txt`, §6.4 row 2).
- `a_tag_that_is_not_the_release_tag_is_reported`: `tag = "airsl-cli-v0.1.0"` gives the `:6:` line.
- `a_missing_line_is_reported`: three rockspecs, each lacking one of `package`, `version` and
  `tag`, give `declares no `package = "…"` line`, `declares no `version = "…"` line` and
  `declares no `tag = "…"` line` respectively, each naming its own key.
- `a_leftover_rockspec_gets_one_message_and_no_line_checks`: `airsstack-0.1.0-1.rockspec` whose
  contents also disagree gives exactly one line for that file.
- `an_unreadable_rockspec_is_reported`: a current rockspec holding byte `0xff` and none of the three
  keys gives exactly one line for that file, the "could not be read" line, and no "declares no"
  line: the per-line checks are skipped for an input that cannot be read.
- `a_symlink_to_a_rockspec_file_is_read` (`#[cfg(unix)]`): a current name that is a symlink to a
  regular file whose `package` line disagrees gives that `:2:` line, so the target was read
  (probe 245).
- `rockspecs_match_the_workspace_version`: the real repository has no problem.

**Done when:** `cargo test -p docs-check` passes, `cargo make dod-crate docs-check` exits 0, and
`cargo make dod` is green. Commit `test(docs-check): check the package, version and tag inside each
rockspec`.

---

## Risks

- A future rockspec that reformats its keys (other indentation, or `tag` not on its own line) is
  read as "declares no … line". The check reads text by line prefix, as §3.4 states. That is the
  intended strictness, not a bug.
- The FIFO, symlink and dangling-symlink tests run on unix only; Windows runs the directory and
  UTF-8 cases (§9.1). The Windows and Linux `dod` legs of the pull request are their first runs on
  those systems inside this crate; the rule itself ran on all three runners (probe 251, R14).
- Not enforced, by design: tagging. A maintainer can still push an `airsl-v*` tag alone (§2.5).
  Plan 12 writes the rule into the READMEs and the CHANGELOG.

## Proof

- `cargo test -p docs-check` passes, including `rockspecs_match_the_workspace_version` over the real
  tree.
- `cargo make dod` is green.

## Review findings

(reviewer verdict: compliant with amendments, DoD green, blocking: none)
- 🟡 module doc of `crates/docs-check/src/rockspecs.rs` claimed no other guard reads the manifests (`citations::module` does) — fixed: reworded to "the only guard that reads the `rockspecs/` directory and parses the manifests as TOML for the version"; `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → exit 0.
- 🟡 test `an_unreadable_rockspec_is_reported` had been renamed `..._and_not_line_checked` — fixed: restored the plan's name; `cargo test -p docs-check rockspecs` → 32 passed.
- 🟢 four tests added beyond the plan's list (cover behaviour the plan requires) — accepted.
- 🟢 plan asks one commit per task; the four tasks are one diff — open: split or squash at the author's commit gate.
- 🟢 nine minor items (untested error paths incl. four of six version type names; the FIFO test would hang rather than fail if the special-file fix were reverted) — open, accepted as nits. — fixed — 2026-11-27 findings-fix pass: tests added for the unlistable rockspecs dir, an unreadable member manifest, an empty-string version, the float/boolean/array/date-time type names plus three table variants, the case-sensitive `.rockspec` suffix and `toml_error_line`'s last non-blank line; the `toml_error_line` doc says "non-blank"; test consts grouped and imported (`use super::{MEMBERS, ROCKSPECS_DIR as DIR}`); the FIFO test runs the check on a worker thread with `recv_timeout(5s)`; the duplicate body of `a_rockspec_that_is_not_utf8_is_reported_as_unreadable` now has its own fixture (a valid `package` line, then bytes `0xff 0xfe`) — evidence: with the `metadata().is_file()` check removed the FIFO test failed in 5.01s ("the check blocked on the FIFO: …") and passed once restored; `CARGO_BUILD_JOBS=2 cargo make dod-crate docs-check` → exit 0, 125 passed.
- 🟢 (fix-pass review nit 11) `a_rockspec_that_is_not_utf8_is_reported_as_unreadable` and `an_unreadable_rockspec_is_reported` had identical bodies; `a_member_version_of_another_type_is_named_by_its_type` overlaps the new type loop — fixed — the non-UTF-8 test was briefly deleted, then restored under the plan's name with a distinct fixture; the member-type test is kept because the plan names it and it covers two members reported together — evidence: docs-check 125 tests green.
- 2026-11-27 findings-fix pass: CI round 3 (run 45942172171) all 10 ci.yml jobs green on ubuntu/macos/windows; local `CARGO_BUILD_JOBS=2 cargo make dod` exit 0.

## Deviations

- 2026-11-27 — The mismatch messages were first built with a literal `…` where the declared value belongs; corrected in the fix pass to print the declared value (e.g. ``declares `package = "airsstack-lua"` ``), the missing-key message keeps its literal `…`. Tests assert the real values; 4 failed before the fix, 32 passed after.
- 2026-11-27 — The plan elides the full "sets its own version …" message; built as `{path}: sets its own version, which must come from the workspace (`version.workspace = true`) so airsl, airsl-cli and airsl-lua share one number`.
- 2026-11-27 — `[workspace.package` parse error is reported at column 19 (test asserts the real output); the plan's `[package` column 9 matched.
- 2026-11-27 — `toml` added as a docs-check dev-dependency (Cargo.lock updated).
- 2026-11-27 — Execution process: one coder ran tasks 1–4 sequentially (they share `rockspecs.rs`) instead of one coder per task; no per-task briefer spawns.
- 2026-11-27 — A sixth docs-check check, `crates/docs-check/src/book_links.rs` (every `https://airsstack.github.io/airsl/…` URL in CLAUDE.md, CHANGELOG.md, the READMEs and the `.rs` files resolves to a SUMMARY-listed chapter, a heading id or an `install/` file), was added beyond the spec's five; `CLAUDE.md` now says "six things". It is owned by plan 11's F11.
- 2026-11-27 — `rockspecs.rs` gained seven tests beyond the plan's list (the error paths of the nine nits); the FIFO test runs the check on a worker thread with `recv_timeout(5s)` so a reverted special-file check fails instead of hanging.
- 2026-11-27 — `a_rockspec_that_is_not_utf8_is_reported_as_unreadable` writes `package = "airsstack"` then bytes `0xff 0xfe`, a fixture distinct from `an_unreadable_rockspec_is_reported`'s lone `0xff`; the plan's text gives both the same shape.
