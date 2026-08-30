---
status: done
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

> **All line numbers below were re-derived against the post-plan-03 tree** and are current as of
> that re-derivation. The banner this replaces warned that they predated plan 03; discharging it
> moved the `io(...)` call-site count from a claimed eighteen to an actual nineteen, shifted every
> citation in tasks 1, 2, 3 and 7, and surfaced `fs.rs:244` — a rendering returned straight to Lua
> that no task owned. Re-derive again before editing if anything has landed since: the plan's own
> rule is that a claim about code carries a `file:line` a reader can check, and a stale number
> breaks that silently rather than loudly.

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

1. Take plan 03's "Handoff to plan 04 — deferred consumer sites" list — every site that takes a
   value out of `PathGuard::read`/`write`. That list is complete for *consumers* and is not the
   list of *renderings*; spec §4 states the gap precisely and this task exists so a later task
   does not forget it.

   Those sites arrive already carrying `.as_path()`, applied by plan 03 solely to keep its own
   boundary compiling. Treat every one as unclassified: `.as_path()` there is a deferral marker,
   not a verdict, and at a rendering site it is the exact mistake step 2 guards against.
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
   - `crates/airsl/src/modules/fs.rs:289` calls `walk(target.as_path())`, and `walk` (`:405-422`)
     renders at both `:411` and `:418`. `.as_path()` clears the diagnostic and leaves both wrong.
   - `crates/airsl/src/modules/hash.rs:98-102` builds an inline `Error::Io` with
     `target.display()`. There is no helper to blame; it converts directly to
     `target.to_script_string()`.
4. Verify:
   ```
   $ grep -rn "as_path()" crates/airsl/src/modules/
   ```
   Expected: every remaining hit is one this task classified as a genuine filesystem handoff, and
   the count of hits removed plus hits kept equals the handoff list's length. A deferral marker
   silently kept is the failure mode this task exists to prevent.

   This verification was originally a count of live compile errors. It cannot be — plan 03 was
   amended to end on a compiling crate, so there are no diagnostics left to count; the deferral
   markers replace them as the thing to enumerate.

### Task 2 — `fs::io` takes `&ResolvedPath`

**Files:**
- Modify `crates/airsl/src/modules/fs.rs`
- Modify `crates/airsl/src/modules/hash.rs`

**Steps:**

1. Change `io` (`fs.rs:52-60`, signature `:53`) from `path: &StdPath` to `path: &ResolvedPath`,
   and `:54` from `path.display().to_string()` to `path.to_script_string()`. **Nineteen** call
   sites already pass a guard-derived value (`:87`, `:98`, `:121`, `:135`, `:137`, `:166`, `:173`,
   `:220`, `:256`, `:258`, `:273`, `:275`, `:299`, `:310`, `:321`, `:333`, `:347`, `:363`, `:377`)
   and need only their `.as_path()` deferral marker dropped — the value they already hold is the one
   the new signature wants. (As approved this read "compile unchanged", which assumed plan 03 left
   those sites broken rather than deferred.) The point of the signature change is that the diagnostic now lands on the
   rendering rather than on the handoff above it.
2. Three further call sites sit inside `atomic_write` (`:428-443`), and none should be silenced
   with `.as_path()`:
   - `:433` passes `directory`, a `&Path` derived from `target.parent()` at `:429`. It is not guard-derived
     and cannot be a `ResolvedPath` honestly. Add a sibling helper
     `fn io_at(operation: &'static str, rendered: String) -> impl FnOnce(std::io::Error) -> Error`
     and make `io` delegate to it. `atomic_write` calls
     `io_at("atomic_write", paths::rules::native::to_script_string(directory))`.
   - `:435`, `:436` and the inline `Error::Io` at `:437-441` pass `target`. Change
     `atomic_write`'s own parameter (`:428`) to `&ResolvedPath` — its only caller (`:147`) already
     passes one — and use `target.as_path()` for `persist` at `:437` and
     `target.to_script_string()` at `:439`.
3. Convert `hash.rs:100` to `target.to_script_string()`.
4. Leave the `/tmp` rationale comment immediately above `atomic_write` alone. It is unix reasoning and it is
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
   `walk_returns_sorted_paths_relative_to_the_root` (`fs.rs:649-661`) expects
   `"a.txt,sub,sub/b.txt"` at `:660`, and `walk_returns_sorted_matches_relative_to_the_root`
   (`glob.rs:245-272`) expects `"Cargo.toml,sub/Cargo.toml"` at `:268-271`. Both fail on Windows today. Confirm they appear in plan 01's `INVENTORY.md`
   under class 3 before changing any code — if they do not, the inventory is incomplete and that
   matters more than this task.
2. `fs.rs`: change `walk` (`:405-422`, signature `:406`) to take `&ResolvedPath`, so the
   diagnostic lands on `:411` and `:418` rather than on the call at `:289`. Use `root.as_path()`
   for `WalkDir::new`, `root.to_script_string()` at `:411`, and
   `paths::rules::native::to_script_string(relative)` at `:418`.
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

### Task 4 — `fs.canonicalize`, `fs.tempdir` and `fs.tempfile`

**Files:**
- Modify `crates/airsl/src/modules/fs.rs`

**Steps:**

1. **`fs.canonicalize` (`:244`) first — it is the one that matters most and the one no task owned.**
   `Ok(target.as_path().to_string_lossy().into_owned())` hands a path *straight back to Lua* as the
   function's return value, not into an error message. Plan 03's handoff table filed it under
   "direct renderings" and called it "the reachable consequence that makes the verbatim hole more
   than theoretical"; this plan as approved then gave it to no task, because tasks 2, 3 and 4 were
   each scoped to a helper (`io`, `walk`, the temp pair) and `:244` belongs to none of them.

   Write the failing test first: `fs.canonicalize` of a granted path contains no `\` and does not
   begin `\\?\`. Then convert `:244` to `target.to_script_string()`. This is the highest-value
   single line in the plan: `ResolvedPath::to_script_string` strips the verbatim prefix *and*
   re-spells the separators, so one conversion closes both halves at the crate's most exposed
   rendering point.
2. Write the failing test for the temp pair: `fs.tempdir()` and `fs.tempfile()` return a path containing no
   `\`. Under `Policy::trusted` on a `windows-latest` runner today they return
   `C:\Users\RUNNER~1\AppData\Local\Temp\airsl-…`, because both are rooted at
   `std::env::temp_dir()` and never pass a rendering boundary. Assert on the absence of `\`
   rather than on an exact string, since the temp root is machine-specific.
3. Convert `:364` (`made.keep().to_string_lossy()`) and `:383` (`path.to_string_lossy()`) to
   `paths::rules::native::to_script_string`.
4. `:380` renders `checked`, a `ResolvedPath`, inside the inline `Error::Io` at `:378-382`. Plan 03
   left it as `checked.as_path().display().to_string()`; convert it to `checked.to_script_string()`.
5. Leave `:359` and `:373` alone — `base.to_string_lossy()` is *input* to `g.write`, and premise
   §1.1 says input is accepted in either form. A comment records this, because converting it would
   look like the consistent thing to do and would add a conversion the guard does not need.
6. Verify:
   ```
   $ cargo test -p airsl modules::fs
   ```
   Scoped to the whole module rather than to `temp`, because step 1's `canonicalize` test does not
   match that filter.

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

1. `guard.rs:121` renders the granted roots in a refusal. They arrive as a plain `&[PathBuf]` off
   `FsGrant` (`:109-112`), never guard-derived, so the type system will never mention them — plan 03
   task 2 step 4 deliberately left this line for here, with a comment saying so. Convert
   `roots.iter().map(|r| r.display().to_string())` to
   `roots.iter().map(|r| paths::rules::native::to_script_string(r))`, and delete that comment as
   part of the same change. `:130` was already converted when plan 03 changed `deny` to take
   `&ResolvedPath`; confirm, do not redo.

   Until this lands, a Windows denial message mixes separators — the offending path arrives from
   `resolved.to_script_string()` as `C:/a/b` while the roots beside it render as `C:\a`. That is
   the visible symptom to look for when checking the fix.
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
2. `chunk_name.rs:60-66`: replace `path.display().to_string()` at `:62-63` with
   `paths::rules::native::to_script_string(path)`. Apply it **before** `elide` (`:65`, defined at
   `:85-98`), not after — `to_script_string` also strips a verbatim prefix, which shortens the
   string, and eliding first would spend the 240-byte budget (`:25`, `:91`) on a prefix that is
   about to be removed. The `\n`/`\r`/`\0` repair at `:64` stays where it is; it is a different
   concern and `to_script_string` does not do it.
3. `require_loader.rs:199`: `set_name(format!("@{}", path.display()))` becomes the same conversion.
   Convert `:193-196`'s `path.display().to_string()` in `Error::ScriptRead` alongside it — same value,
   same rendering, and a message that disagrees with the traceback above it is worse than either
   spelling alone.
4. **Do not touch the module-cache key at `require_loader.rs:149`.** It is an internal `PathBuf`
   keyed on the canonical path and is never script-visible; re-spelling it would change a lookup
   key to fix a display problem. A comment says so, because it is the obvious next line to convert.
5. **The `root` field of the four `require` errors.** `require_loader.rs` renders
   `root.display().to_string()` at `:159-160` (`Error::RequireCycle`), `:208` (`Error::RequireNotFound`,
   the `canonicalize` failure arm), `:222` (`Error::RequireEscape`) and `:233`
   (`Error::RequireNotFound`, the exhausted-candidates arm). All four are script-visible messages
   and all four convert to `paths::rules::native::to_script_string`.

   Two of them are worse than a spelling problem. At `:222` and `:233` the `root` binding is the
   **shadowed, canonicalised** one from `:206`, so on Windows it renders `\\?\C:\…` — a verbatim
   path reaching a message a script author reads, which premise §1.4 forbids outright. At `:159`
   and `:208` `root` is still the caller-supplied `&Path` (the shadow at `:206` has not taken
   effect inside its own initialiser), so those two are native-separator only.

   Line numbers are post-plan-03: that plan added a comment and a `strip_verbatim` call to
   `resolve`, shifting everything below `:214` down.

6. `loaded.rs:117-123` builds the extension chunk name from `manifest.entry()`. That is a
   manifest-declared relative path whose components are all `Component::Normal`
   (`manifest.rs:272-278`), so on Windows it renders `src/main.lua` unchanged when the manifest
   wrote `/` — and `src\main.lua` when the manifest wrote `\`, which premise §1.1 accepts on input.
   Convert the `manifest.entry().display()` rendering inside that `format!` so the three sources
   agree regardless of how the manifest spelled it.
7. Verify:
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
     expectations at `:350-378` do not cover this input, so no current test changes — but the four
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
3. `absolute` (`:244-251`) renders through `normalize` at `:246`, so it inherits the fix and needs
   no change of its own. Say so in a comment rather than adding a second conversion.
4. Verify:
   ```
   $ cargo test -p airsl modules::path::tests::normalize
   ```
   Expected: the four existing `normalize` tests (`:350-378`) green, plus the new `/..` assertion
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
5. §4.0: `is_absolute` (`:112-117`) keeps `Path::is_absolute` and `absolute` (`:244-251`) keeps
   `std::path::absolute`. **Absoluteness is a property of the platform's path grammar, not of the
   separator.** Document that on both, and document that this is the one place where the outward
   vocabulary is uniform and the semantics are not.
6. Give the five unix-shaped tests platform-conditional expectations. The unix arm keeps today's
   assertion verbatim; only a Windows arm is added:
   - `is_absolute_distinguishes_the_two_kinds_of_path` (`:424-434`) — on Windows `'/a'` is **not**
     absolute; add `'C:/a'` → true and keep `'a'` → false on both.
   - `absolute_leaves_an_absolute_path_alone` (`:436-439`) — on Windows `absolute('/a/b')` prepends
     the current drive; assert `absolute('C:/a/b') == "C:/a/b"`.
   - `absolute_makes_a_relative_path_absolute` (`:441-446` — the spec cites `:442-447`, one line
     past the closing brace) — `starts_with('/')` at `:444` is false on Windows; assert
     `path.is_absolute` of the result there instead. Keep `ends_with("/a")` at `:445` on **both**
     arms: that half is the vocabulary claim and it must hold everywhere.
   - `absolute_normalises_what_it_produces` (`:448-451`) — assert `absolute('C:/a/b/../c')` →
     `"C:/a/c"` on Windows.
   - `absolute_does_not_require_the_path_to_exist` (`:453-459`) — drive-qualify the input on
     Windows.
7. Verify:
   ```
   $ cargo test -p airsl modules::path
   ```
   Expected: green, with **at least 27** tests — 27 exist today, five of those gain
   platform-conditional arms here, the other 22 keep their current expectations unchanged (which is
   the claim spec §4.0 makes and this run is the evidence for it), and task 8 adds its own on top.
   As approved this read "27 tests green", which silently assumed task 8 added assertions to
   existing tests rather than new ones; the invariant worth checking is that no existing
   expectation was rewritten, not that the count stayed still.

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


### Task 11 — The residue the residue table missed

**Files:**
- Modify `crates/airsl/src/script.rs`
- Modify `crates/airsl/src/sandbox/grant_set.rs`
- Modify `crates/airsl/src/extension/negotiate.rs`
- Modify `crates/airsl/src/extension/manifest.rs`
- Modify `crates/airsl/src/extension/loaded.rs`
- Modify `crates/airsl/src/extension/host.rs`
- Modify `crates/airsl/src/modules/fs.rs`

**Steps:**

1. This task exists because spec §4's residue table is **incomplete**, and says so about itself:
   "Three drafts tried to enumerate the boundaries by reading; the first missed six and the second
   missed four more." A fourth reading — a crate-wide
   `grep -rn --include="*.rs" "display()\|to_string_lossy" crates/airsl/src` filtered to non-test
   code — found eleven more renderings that no task in this plan owned. The rule they fall under is
   §4's opening sentence, which is broader than the table beneath it: *no path reaches a script, a
   Lua traceback, **or an error message** in native spelling.*

   Do the grep again before starting rather than trusting this list. The one durable lesson of §4
   is that reading finds fewer sites than grepping, every time.

2. Convert each of these to `paths::rules::native::to_script_string`. All are host- or
   script-visible message text:

   | Site | Reaches a reader as |
   |---|---|
   | `script.rs:66` | `Error::ScriptRead`'s `path` field |
   | `extension/host.rs:284` | the `path` field of the error raised when an extension root will not resolve |
   | `extension/negotiate.rs:46`, `:47` | `Capability: Display` — `fs.read \`{}\`` / `fs.write \`{}\`` in every negotiation denial |
   | `extension/negotiate.rs:254` | the `roots` helper, which joins a root list into denial text |
   | `extension/manifest.rs:179`, `:194` | manifest read/parse error paths |
   | `extension/loaded.rs:150`, `:154`, `:158` | `recheck_entry`'s three `Error::ManifestInvalid` messages |
   | `sandbox/grant_set.rs:154`, `:157` | `GrantSet: Display` — `read {}` / `write {}` |

3. `sandbox/grant_set.rs:154`/`:157` is the one worth arguing rather than just doing. `GrantSet`'s
   `Display` is what `airsl doctor` prints to a human terminal, where a Windows reader might
   reasonably expect `C:\a`. Convert it anyway, and say why in a comment: the same roots are
   already rendered `/`-spelled by the guard's refusal message and by `ext.granted()`, so leaving
   `Display` native would make one runtime spell one root two ways depending on which message the
   reader happened to hit. One vocabulary, uniformly, is worth more than matching shell convention
   in one of three places.

4. **`modules/fs.rs:280` is not a conversion.** `entry.file_name().to_string_lossy()` renders a
   single directory-entry name, which cannot contain a separator on either platform. Add a comment
   saying that — the plan-level sweep flags every surviving `to_string_lossy`, and a survivor with
   no comment is indistinguishable from one that was missed.

5. Write the tests that can actually discriminate. Most of these are identity no-ops on unix, so
   prefer one test that drives `paths::rules::to_script_string` with an explicit
   `PathFlavor::Windows` over several that assert identity — the flavour-parameterised rule is
   testable on this host and the compile-time wrapper is not. Where a test cannot discriminate on
   macOS, its comment says so.

6. Verify:
   ```
   $ cargo test -p airsl script:: sandbox:: extension:: modules::fs
   ```


---

## Verification summary (plan-level)

- `cargo make dod-crate airsl` green on macOS, and `cargo make dod` green on all three platforms via
  CI.
- **No rendering site remains.** `grep -rn --include="*.rs" "display()\|to_string_lossy" crates/airsl/src`
  outside `#[cfg(test)]` blocks returns only sites a comment justifies: input handed to the guard
  (`fs.rs:365`, `:380`), single-component names (`path.rs:147`, `:155`, `:165` and `fs.rs:280`), the
  module-cache key (`require_loader.rs:151`), and the two rendering rules' own implementations
  (`paths/rules.rs:197`, `:212`). As approved this list named four sites and missed eleven; task 11
  exists because the sweep, not the reading, is what produced the true list.
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

---

## Amendments after approval

| What | Why |
|---|---|
| Task 8's rebuilt `normalize` shipped a trailing-separator bug, found in review and fixed | `PathBuf::push` appends a separator even when the pushed path is **empty**, so any result that was purely leading `..`s gained one: `normalize("..")` gave `"../"`, `"../.."` gave `"../../"`, `"a/../.."` gave `"../"`, `"./.."` gave `"../"`. Wrong output on **every** platform, unrelated to the deliberate `/..` change. The task's own assertion list missed it because every input it named (`../a`, `a/../../b`) left the remainder non-empty. The assembly now pushes `out.components()` one at a time, so an empty remainder contributes nothing. Pinned by `normalize_of_a_purely_leading_dotdot_result_has_no_trailing_separator`. The comment's stated invariant was true but insufficient: it covered relativity and not emptiness. |
| Task 6 step 2's test was delivered against the rendering rule rather than against `sorted_roots`, and has been corrected | The step specifies a test over `sorted_roots`, which is callable on this host. As delivered it duplicated a `paths::rules` test verbatim and never called `sorted_roots`, so reverting that function would have left it green. Now split: one test names the rendering rule, and `sorted_roots_sorts_the_rendered_strings_not_the_paths_own_ordering` calls `sorted_roots` and exploits a real divergence between `PathBuf`'s component-wise `Ord` and byte-order string sort. |
| Recorded a spec-level disagreement rather than acting on it: `Component::Prefix` sets `rooted` for a bare `Disk` prefix | On Windows `C:a` is drive-relative, so absorbing a `..` above it drops a real parent step (`normalize("C:a/../..")` yields `"C:"`, where `C:..` preserves the information). Raised in review; the narrower rule is written up in the spec's `normalize` section. **Not changed** - it is delivered exactly as that section specifies and as this plan's tests assert, no host here can run the Windows arm, and altering it changes what the runtime promises on Windows. That decision belongs to whoever owns this chain. |
| Task 7 gained a step covering the four `root.display()` renderings in `require_loader.rs` (`:159`, `:208`, `:222`, `:233`) | No task owned them. Task 7 as approved covered only the chunk name (`:197`) and `Error::ScriptRead` (`:192`); task 6 covered the guard and `ext` residue. `:222` and `:233` render the *canonicalised* root, so on Windows they put a verbatim `\\?\C:\…` into a script-visible error message — a premise §1.4 violation, not merely a separator inconsistency. Found by reading the function after plan 03 rewrote it; no diagnostic points at any of the four. |
| **Task 11 added**: eleven further renderings in `script.rs`, `grant_set.rs`, `negotiate.rs`, `manifest.rs` and `loaded.rs`, plus a justifying comment on `fs.rs:280` | Spec §4's residue table lists ten sites and openly says earlier drafts of it missed six, then four more. A crate-wide grep — rather than a fourth reading — found eleven the table still omits, every one an error or denial message and so covered by §4's opening rule (*no path reaches a script, a Lua traceback, or an error message in native spelling*). Surfaced when `loaded::recheck_entry` turned up as an orphan during task 7 and the sweep was widened to the whole crate. The plan-level verification summary asserted this sweep would come back clean while naming only four justified survivors, so it would have failed against the plan's own tasks. |
| Task 4 renamed and given a first step converting `fs.rs:244`, `fs.canonicalize`'s return value | **No task owned it.** Plan 03's handoff filed `:244` under "direct renderings" and named it the reachable consequence that makes the verbatim hole more than theoretical — it hands a path *straight back to Lua*, not into an error message. Tasks 2, 3 and 4 were each scoped to a helper (`io`, `walk`, the temp pair) and `:244` belongs to none, so the plan's most exposed rendering point would have shipped unconverted while every message around it was fixed. |
| Every `file:line` in tasks 1, 2, 3 and 7 re-derived against the delivered tree; the drift banner rewritten to say it was discharged | The banner asked for exactly this and named task 2 as untrusted. Discharging it moved the `io(...)` call-site count from a claimed eighteen to an actual **nineteen**, corrected `walk` `:397→:405-422`, `atomic_write` `:419-434→:428-443`, the `walk` call site `:281→:289`, three `require_loader.rs` numbers (`:147→:149`, `:192→:193-196`, `:197→:199`), the `chunk_name.rs` render span, five `path.rs` test ranges, and `loaded.rs` `:117-121→:117-123`. |
| Task 9 step 7's "27 tests green" relaxed to "at least 27", with the invariant restated | It contradicted task 8, which adds tests. `path.rs` has 27 today (verified); the claim worth making is that no *existing* expectation is rewritten, not that the count holds still. |
| Task 7's steps renumbered `1,2,3,4,6,7,6` → `1,2,3,4,5,6,7` | Fallout from inserting the `root.display()` amendment step in the previous round. |
| Task 1 steps 1 and 4, and task 2 step 1, re-pointed from live compile diagnostics to plan 03's recorded handoff list and `.as_path()` deferral markers | Plan 04 as approved required the crate to be non-compiling at plan 03's boundary — task 1 step 4 counted `^error` lines and task 2 step 1 expected eighteen sites to "compile unchanged". Plan 03 was amended to end green, since ending a plan red contradicts the repo's rule that anything short of `cargo make dod` is not a green result. The inventory is unchanged and still compiler-derived; only where it is read from moved, from live diagnostics to a written list plus a greppable marker. |
