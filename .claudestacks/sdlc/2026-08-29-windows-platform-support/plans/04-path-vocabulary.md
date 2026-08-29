---
status: approved
created: 2026-08-30
depends-on: [02]
---

# Path Vocabulary Implementation Plan

**Goal:** Make every path a script sees `/`-separated and never verbatim.

**Architecture:** Two halves that must both run or neither is worth anything. The first is
mechanical: plan 03 changed `PathGuard::read`/`write` to return `ResolvedPath`, so `rustc` hands
over the exhaustive list of the guard's *consumers* — this plan triages every rendering downstream
of each, and treats an inserted `.as_path()` as a defect rather than a fix. The second is by hand:
nine sites render a path that never passes through the guard, so no diagnostic will ever point at
them. `fs::io` is changed to take `&ResolvedPath` so that the highest-traffic rendering site in the
crate produces its own diagnostic instead of borrowing one. `normalize` is rebuilt rather than
patched, and `is_absolute`/`absolute` are deliberately left alone.

**Tech Stack:** Rust 1.94, `std::path`, `globset` 0.4, `walkdir` 2.5, `mlua`.

**Content authority:** spec §4 in full — §4.0 `is_absolute`/`absolute`, §4.1 `normalize`, §4.2 glob
patterns, §4.3 what is not a determinism problem — and premise §1.1.

---

## File structure

```
crates/airsl/src/modules/fs.rs          — modify  io/walk/atomic_write take ResolvedPath; temp + walk renderings
crates/airsl/src/modules/glob.rs        — modify  walk renderings; pin backslash_escape
crates/airsl/src/modules/proc.rs        — modify  which's rendering only
crates/airsl/src/modules/guard.rs       — modify  the grant-root list in a refusal
crates/airsl/src/modules/ext.rs         — modify  sorted_roots renders before it sorts
crates/airsl/src/modules/hash.rs        — modify  hash_file's Error::Io rendering
crates/airsl/src/modules/path.rs        — modify  normalize rebuilt; join/dirname/relative_to; five tests
crates/airsl/src/types/chunk_name.rs    — modify  from_path renders through the vocabulary
crates/airsl/src/require_loader.rs      — modify  the require chunk name and ScriptRead path
crates/airsl/src/extension/loaded.rs    — modify  the extension chunk name
```

Every task verifies with a scoped `cargo test`, and the plan closes on `cargo make dod-crate airsl`.
Every conversion uses `paths::rules::native::to_script_string` (plan 02) or
`ResolvedPath::to_script_string` (plan 02) — never a hand-rolled `.replace('\\', "/")`, which would
be a fourth copy of a rule that now has one home.

### Task 1 — Fix the triage rule before triaging

**Steps:**

1. Take plan 03 task 2 step 5's compile-error list — every site that takes a value out of
   `PathGuard::read`/`write`. That list is complete for *consumers* and is not the list of
   *renderings*; spec §4 states the gap precisely and this task exists so a later task does not
   forget it.
2. For each entry, classify it into exactly one of three:
   - **a rendering** — the value becomes a `String` handed to Lua, an error message, or a chunk
     name. Convert it with `to_script_string`.
   - **a filesystem handoff** — the value goes to `std::fs`, `walkdir`, or `tempfile`. `.as_path()`
     is correct here and is the only place it is.
   - **a handoff to a helper that renders below it** — `.as_path()` compiles and the `display()`
     inside stays natively spelled. **This is not a fix.** Either change the helper's parameter to
     `&ResolvedPath` (tasks 2 and 3 do this for `fs::io`, `fs::walk` and `fs::atomic_write`) or
     convert the rendering inside it in the same task.
3. Two entries are known in advance and are worth checking the list against, because they are the
   shape a plan author most easily mis-files:
   - `crates/airsl/src/modules/fs.rs:281` calls `walk(&target)`, and `walk` (`:397`) renders at
     both `:402` and `:409`. `.as_path()` clears the diagnostic and leaves both wrong.
   - `crates/airsl/src/modules/hash.rs:98-102` builds an inline `Error::Io` with
     `target.display()`. There is no helper to blame; it converts directly to
     `target.to_script_string()`.
4. Verify:
   ```
   $ cargo build -p airsl --all-targets 2>&1 | grep -c "^error"
   ```
   Expected: the count matches the number of entries triaged. A diagnostic cleared but not
   classified is the failure mode this task exists to prevent.

### Task 2 — `fs::io` takes `&ResolvedPath`

**Files:**
- Modify `crates/airsl/src/modules/fs.rs`
- Modify `crates/airsl/src/modules/hash.rs`

**Steps:**

1. Change `io` (`fs.rs:53-54`) from `path: &StdPath` to `path: &ResolvedPath`, and `:54` from
   `path.display().to_string()` to `path.to_script_string()`. About eighteen call sites already
   pass a guard-derived value (`:87`, `:97`, `:120`, `:134`, `:136`, `:165`, `:172`, `:216`,
   `:251`, `:252`, `:266`, `:267`, `:291`, `:302`, `:313`, `:324`, `:338`, `:354`, `:368`) and
   compile unchanged. The point of the signature change is that the diagnostic now lands on the
   rendering rather than on the handoff above it.
2. Two call sites break, both inside `atomic_write` (`:419-434`), and neither should be silenced
   with `.as_path()`:
   - `:424` passes `directory`, a `&Path` derived from `target.parent()`. It is not guard-derived
     and cannot be a `ResolvedPath` honestly. Add a sibling helper
     `fn io_at(operation: &'static str, rendered: String) -> impl FnOnce(std::io::Error) -> Error`
     and make `io` delegate to it. `atomic_write` calls
     `io_at("atomic_write", paths::rules::native::to_script_string(directory))`.
   - `:426`, `:427` and the inline `Error::Io` at `:428-432` pass `target`. Change
     `atomic_write`'s own parameter (`:419`) to `&ResolvedPath` — its only caller (`:146`) already
     passes one — and use `target.as_path()` for `persist` at `:428` and
     `target.to_script_string()` at `:430`.
3. Convert `hash.rs:100` to `target.to_script_string()`.
4. Leave the `/tmp` rationale comment at `fs.rs:417-418` alone. It is unix reasoning and it is
   wrong on Windows, but spec §7 owns it; a comment here records that so the next reader does not
   read the omission as an oversight.
5. Verify:
   ```
   $ cargo test -p airsl modules::fs modules::hash
   ```
   Expected: green on macOS with no assertion changes — on unix `to_script_string` is the identity,
   so this task is observably a no-op there and a spelling fix on Windows.

### Task 3 — The two `walk` residues

**Files:**
- Modify `crates/airsl/src/modules/fs.rs`
- Modify `crates/airsl/src/modules/glob.rs`

**Steps:**

1. The failing tests already exist and already assert the right answer:
   `walk_returns_sorted_paths_relative_to_the_root` (`fs.rs:641-652`) expects
   `"a.txt,sub,sub/b.txt"` at `:651`, and the `glob.walk` test expects `"Cargo.toml,sub/Cargo.toml"`
   at `glob.rs:268-271`. Both fail on Windows today. Confirm they appear in plan 01's `INVENTORY.md`
   under class 3 before changing any code — if they do not, the inventory is incomplete and that
   matters more than this task.
2. `fs.rs`: change `walk` (`:397`) to take `&ResolvedPath`, so the diagnostic lands on `:402` and
   `:409` rather than on the call at `:281`. Use `root.as_path()` for `WalkDir::new`,
   `root.to_script_string()` at `:402`, and
   `paths::rules::native::to_script_string(relative)` at `:409`.
3. `glob.rs`: `base` (`:102`) is a `ResolvedPath` after plan 03. Use `base.as_path()` at `:106` and
   `:112`, `base.to_script_string()` at `:109`, and
   `paths::rules::native::to_script_string(relative)` at `:118`.
4. `glob.rs:117` matches `relative` as a `&Path`, not as the rendered string. Leave it that way and
   say why in a comment: `globset` normalises candidate separators itself (§4.2), so matching the
   native `Path` and *returning* the `/` rendering is one decision, not an inconsistency.
5. Verify:
   ```
   $ cargo test -p airsl modules::fs::tests::walk modules::glob
   ```
   Expected: green on macOS unchanged; the two existing assertions above become the Windows proof.

### Task 4 — `fs.tempdir` and `fs.tempfile`

**Files:**
- Modify `crates/airsl/src/modules/fs.rs`

**Steps:**

1. Write the failing test first: `fs.tempdir()` and `fs.tempfile()` return a path containing no
   `\`. Under `Policy::trusted` on a `windows-latest` runner today they return
   `C:\Users\RUNNER~1\AppData\Local\Temp\airsl-…`, because both are rooted at
   `std::env::temp_dir()` and never pass a rendering boundary. Assert on the absence of `\`
   rather than on an exact string, since the temp root is machine-specific.
2. Convert `:355` (`made.keep().to_string_lossy()`) and `:374` (`path.to_string_lossy()`) to
   `paths::rules::native::to_script_string`.
3. `:371` renders `checked`, a `ResolvedPath`, inside an inline `Error::Io`. It is a task 1
   diagnostic; convert it to `checked.to_script_string()`.
4. Leave `:350` and `:364` alone — `base.to_string_lossy()` is *input* to `g.write`, and premise
   §1.1 says input is accepted in either form. A comment records this, because converting it would
   look like the consistent thing to do and would add a conversion the guard does not need.
5. Verify:
   ```
   $ cargo test -p airsl modules::fs::tests::temp
   ```

### Task 5 — `proc.which`'s rendering

**Files:**
- Modify `crates/airsl/src/modules/proc.rs`

**Steps:**

1. Write the failing test: on Windows, `proc.which('where')` returns a `/`-separated absolute path.
2. Convert `:167` (`found.to_string_lossy().into_owned()`) to
   `paths::rules::native::to_script_string`.
3. **Scope this task to the rendering only.** Plan 05 rewrites the same function's candidate list
   (`executable_candidates`, spec §5.1) and replaces `is_executable` (`:171-175`), which is where
   `.exe` resolution and the `PermissionsExt` removal belong. `:163-166` — the `PATH` split and the
   `directory.join(program)` — is plan 05's, not this plan's. Touching only `:167` keeps the two
   plans from colliding on the same lines.
4. Verify:
   ```
   $ cargo test -p airsl modules::proc
   ```
   Expected: green on macOS unchanged; on Windows this test still fails until plan 05 lands, because
   `which('where')` cannot yet find `where.exe`. Note that in the test's own comment rather than
   weakening the assertion.

### Task 6 — The residue no diagnostic points at

**Files:**
- Modify `crates/airsl/src/modules/guard.rs`
- Modify `crates/airsl/src/modules/ext.rs`

**Steps:**

1. `guard.rs:114` renders the granted roots in a refusal. They arrive as a plain `&[PathBuf]` off
   `FsGrant` (`:107-108`), never guard-derived, so the type system will never mention them — plan 03
   task 2 step 4 deliberately left this line for here. Convert
   `roots.iter().map(|r| r.display().to_string())` to
   `roots.iter().map(|r| paths::rules::native::to_script_string(r))`. `:123` was already converted
   when plan 03 changed `deny` to take `&ResolvedPath`; confirm, do not redo.
2. `ext.rs`'s `sorted_roots` is at `:136-143` — the spec cites `:138-143`, which starts one line
   into the function; the rendering is `:139` and the sort is `:141`. **The sort runs over the
   rendered strings**, so the separator changes ordering and not only spelling. Write that test
   first: with roots `C:/a/b` and `C:/aZ`, byte order puts `C:/a/b` first under `/` (0x2F) and
   `C:\aZ` first under `\` (0x5C), because `Z` is 0x5A and sits between the two separators. Two
   hosts granting the same roots would then get different `granted()` output on different platforms
   — exactly what the comment at `:126-130` says the sort exists to rule out.
3. Convert `:139` to `paths::rules::native::to_script_string`, and rewrite the comment at
   `:131-135`. That comment currently argues `to_string_lossy` over `.display()` on the grounds that
   `granted()` is machine-read; the argument survives and now reaches further — the same reason the
   choice was made explicit is the reason the separator is pinned rather than inherited.
4. Verify:
   ```
   $ cargo test -p airsl modules::ext modules::guard
   ```

### Task 7 — The three chunk-name sources, converted together

**Files:**
- Modify `crates/airsl/src/types/chunk_name.rs`
- Modify `crates/airsl/src/require_loader.rs`
- Modify `crates/airsl/src/extension/loaded.rs`

**Steps:**

1. These must move in one task. `chunk_name.rs:60-66` names every file-loaded script (via
   `script.rs:72`) and `require_loader.rs:197` names every `require`d module; if only one is
   converted, a traceback spells the same file two ways depending on how it was loaded. Write that
   as the test: load a nested module both ways and assert the two names are byte-identical.
2. `chunk_name.rs:60-66`: replace `path.display().to_string()` at `:61-63` with
   `paths::rules::native::to_script_string(path)`. Apply it **before** `elide` (`:65`, defined at
   `:85-98`), not after — `to_script_string` also strips a verbatim prefix, which shortens the
   string, and eliding first would spend the 240-byte budget (`:88`, `:91`) on a prefix that is
   about to be removed. The `\n`/`\r`/`\0` repair at `:64` stays where it is; it is a different
   concern and `to_script_string` does not do it.
3. `require_loader.rs:197`: `set_name(format!("@{}", path.display()))` becomes the same conversion.
   Convert `:192`'s `path.display().to_string()` in `Error::ScriptRead` alongside it — same value,
   same rendering, and a message that disagrees with the traceback above it is worse than either
   spelling alone.
4. **Do not touch the module-cache key at `require_loader.rs:147`.** It is an internal `PathBuf`
   keyed on the canonical path and is never script-visible; re-spelling it would change a lookup
   key to fix a display problem. A comment says so, because it is the obvious next line to convert.
5. `loaded.rs:117-121` builds the extension chunk name from `manifest.entry()`. That is a
   manifest-declared relative path whose components are all `Component::Normal`
   (`manifest.rs:272-278`), so on Windows it renders `src/main.lua` unchanged when the manifest
   wrote `/` — and `src\main.lua` when the manifest wrote `\`, which premise §1.1 accepts on input.
   Convert `:120` so the three sources agree regardless of how the manifest spelled it.
6. Verify:
   ```
   $ cargo test -p airsl types::chunk_name require_loader extension::loaded
   ```

### Task 8 — Rebuild `normalize`

**Files:**
- Modify `crates/airsl/src/modules/path.rs`

**Steps:**

1. Write the failing tests first. The unix-observable change is deliberate and is stated in spec
   §4.1 and §12; it goes in as an assertion, not as a discovery:
   - `normalize('/..')` → `"/"`. Today it returns `"..//"`: `out.pop()` on `RootDir` returns false
     at `:183`, the `..` lands in `leading` at `:184`, and `:197` splices the two. The existing
     expectations at `:351-376` do not cover this input, so no current test changes — but the four
     that exist must stay green.
   - `normalize('/a/../..')` → `"/"`.
   - `normalize('a/../../b')` → `"../b"` and `normalize('../a')` → `"../a"`, both unchanged: a `..`
     above a *relative* start still survives, because there is no root to absorb it.
   - `normalize('')` → `"."` and `normalize('./.')` → `"."`, unchanged.
   - `#[cfg(windows)]`: `normalize('C:a/../..')` → `"C:"`. Today it yields `"../C:"`, which is
     garbage — the `Prefix` lands in `out`, the `..` cannot pop it, and `:197` prepends the escaped
     `..` in front of the drive.
   - `#[cfg(windows)]`: `normalize('C:/a/../..')` → `"C:/"`, and `normalize(r'C:\a\..\b')` →
     `"C:/b"`.
2. Rewrite `:173-205`. No string concatenation survives: assemble into a `PathBuf` and render once
   through `paths::rules::native::to_script_string`.
   - Track `rooted: bool`, set when a `Component::RootDir` or `Component::Prefix` is pushed.
   - On `ParentDir` where `out.pop()` returns false: if `rooted`, absorb it — this is the rule every
     operating system already applies to `/..`. Otherwise increment a `leading` count.
   - Assemble: when `leading > 0`, build a fresh `PathBuf`, push `".."` that many times, then push
     `out`. `leading > 0` implies `!rooted`, so `out` is relative and `PathBuf::push` cannot replace
     what came before it — state that invariant in a comment, because it is the reason a single
     `push` is safe here and the reason the old code needed a `format!`.
   - Empty result stays `"."` (`:200-204`).
3. `absolute` (`:244-250`) renders through `normalize` at `:246`, so it inherits the fix and needs
   no change of its own. Say so in a comment rather than adding a second conversion.
4. Verify:
   ```
   $ cargo test -p airsl modules::path::tests::normalize
   ```
   Expected: the four existing `normalize` tests (`:351-376`) green, plus the new `/..` assertion
   and the two Windows-only ones compiled out on macOS.

### Task 9 — The rest of `path.rs`, and the §4.0 exception

**Files:**
- Modify `crates/airsl/src/modules/path.rs`

**Steps:**

1. Write the failing tests: `path.join('a', 'b')` → `"a/b"` on every platform (today `"a\b"` on
   Windows), `path.dirname('a/b/c')` → `"a/b"`, and `path.relative_to('a/b/c', 'a')` → `"b/c"`.
2. Convert the three renderings that can carry a separator:
   - `join` at `:75` — `out.to_string_lossy()` over a `PathBuf` built by `push`.
   - `dirname` at `:137` — `parent()` is multi-component.
   - `relative_to` at `:228` — `strip_prefix`'s remainder is multi-component.
3. Leave `basename` (`:147`), `stem` (`:155`) and `extension` (`:165`) alone. Each returns a single
   `OsStr` component that cannot contain a separator on either platform, so a conversion there would
   be a no-op added for symmetry. A comment says which of the two reasons applies, since "it was
   missed" and "it cannot matter" look identical from the outside.
4. `relative_to` normalises both sides first (`:218-219`), so after task 8 it compares `/`-spelled
   strings. `Path::new("C:/a/b")` parses `/` as a separator on Windows, so `strip_prefix` at `:221`
   still works component-wise. Confirm with the sibling-prefix test at `:415-422`, which must stay
   green.
5. §4.0: `is_absolute` (`:112-117`) keeps `Path::is_absolute` and `absolute` (`:244-245`) keeps
   `std::path::absolute`. **Absoluteness is a property of the platform's path grammar, not of the
   separator.** Document that on both, and document that this is the one place where the outward
   vocabulary is uniform and the semantics are not.
6. Give the five unix-shaped tests platform-conditional expectations. The unix arm keeps today's
   assertion verbatim; only a Windows arm is added:
   - `is_absolute_distinguishes_the_two_kinds_of_path` (`:425-434`) — on Windows `'/a'` is **not**
     absolute; add `'C:/a'` → true and keep `'a'` → false on both.
   - `absolute_leaves_an_absolute_path_alone` (`:437-439`) — on Windows `absolute('/a/b')` prepends
     the current drive; assert `absolute('C:/a/b') == "C:/a/b"`.
   - `absolute_makes_a_relative_path_absolute` (`:442-446` — the spec cites `:442-447`, one line
     past the closing brace) — `starts_with('/')` at `:444` is false on Windows; assert
     `path.is_absolute` of the result there instead. Keep `ends_with("/a")` at `:445` on **both**
     arms: that half is the vocabulary claim and it must hold everywhere.
   - `absolute_normalises_what_it_produces` (`:449-451`) — assert `absolute('C:/a/b/../c')` →
     `"C:/a/c"` on Windows.
   - `absolute_does_not_require_the_path_to_exist` (`:454-459`) — drive-qualify the input on
     Windows.
7. Verify:
   ```
   $ cargo test -p airsl modules::path
   ```
   Expected: 27 tests green. Five carry platform-conditional arms; the other 22 keep their current
   expectations unchanged, which is the claim spec §4.0 makes and this run is the evidence for it.

### Task 10 — Pin `globset`'s backslash escaping

**Files:**
- Modify `crates/airsl/src/modules/glob.rs`

**Steps:**

1. Write the failing test: a pattern escaping a literal asterisk means the same thing on both
   platforms. `glob.match('a\*.rs', 'a*.rs')` → true, and — the half that matters —
   `glob.match('a\*.rs', 'ab.rs')` → **false**. Under the platform default the second is true on
   Windows, because the `\` is not an escape there and `*` stays a wildcard.
2. Add `.backslash_escape(true)` to the builder chain in `matcher` (`:62-72`), beside
   `.literal_separator(true)` at `:64`.
3. Comment the *why*, matching the density of the `literal_separator` rationale already at `:46-61`:
   `globset` defaults this to `!is_separator('\\')` (`globset-0.4.20/src/glob.rs:244`), which is
   false on Windows. Inheriting it would let a pattern meaning "literal asterisk" become a wildcard
   there — a **widening**, which `:52-56` states is the one direction a matcher must never be wrong
   in. The value is pinned rather than inherited for the same reason `literal_separator` is.
4. Note in the same comment that patterns are always written in the `/` vocabulary and that
   `globset` normalises candidates itself, so no candidate-side conversion is added — that is the
   decision task 3 step 4 already recorded from the other side.
5. Verify:
   ```
   $ cargo test -p airsl modules::glob
   ```
   Expected: green on macOS, where the default was already `true` — so this task is a pin on unix
   and a fix on Windows.

---

## Verification summary (plan-level)

- `cargo make dod-crate airsl` green on macOS, and `cargo make dod` green on all three platforms via
  CI.
- **No rendering site remains.** `grep -n "display()\|to_string_lossy" crates/airsl/src` outside
  `#[cfg(test)]` blocks returns only sites a comment justifies: input handed to the guard
  (`fs.rs:350`, `:364`), single-component names (`path.rs:147`, `:155`, `:165`), and the module-cache
  key (`require_loader.rs:147`).
- **No `.as_path()` sits above an unconverted rendering.** Every `.as_path()` in the diff is followed
  by a filesystem call, never by a `display()`. Confirm by reading each one — this is the check task 1
  exists to make possible and the one the compiler cannot make.
- The three chunk-name sources agree: a script loaded from a file and the same file reached through
  `require` produce byte-identical traceback names.
- `normalize('/..')` returns `"/"` on unix. This is the plan's one deliberate unix-observable change
  and it is asserted, not discovered.
- `path.rs` has 27 tests: 5 with platform-conditional arms, 22 unchanged.
- Plan 01's `INVENTORY.md` class-3 entries (natively-rendered paths in a message or chunk name) are
  all accounted for by a task above, or the shortfall is reported rather than absorbed.
