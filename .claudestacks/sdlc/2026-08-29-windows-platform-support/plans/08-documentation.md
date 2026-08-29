---
status: approved
created: 2026-08-30
depends-on: [03, 04, 05, 06, 07]
---

# Documentation Implementation Plan

**Goal:** Restate every documented promise that Windows support makes false.

**Architecture:** Two halves that do not overlap. The first is a **mechanical sweep** — one `grep`
over `crates/`, the root `*.md` files, `.github/` and `Makefile.toml` for unix-specific vocabulary,
whose every hit is triaged. The second is the **known-site list** below, which seeds that sweep
without being asserted as its output. Both halves are necessary and neither contains the other: the
sweep catches what reading misses (a `CLOCK_MONOTONIC` claim and two `O_CREAT|O_EXCL` claims
survived two full drafts of the spec), and the seed list catches what the sweep misses (five of the
sites below carry no unix vocabulary at all — verified in Task 1). Edits keep the evidence standard
the rest of `crates/airsl/docs/` holds: a claim about code that exists carries a `file:line`, a
claim about code that does not says so explicitly.

**Tech Stack:** Markdown (Diátaxis), Rust doc comments, `cargo make dod` (rustdoc under
`RUSTDOCFLAGS="-D warnings"`), `grep`.

**Content authority:** spec §11, and the intent's desired outcome — "the `compile_error!` and every
documented unix-only promise are removed or restated."

---

## File structure

```
crates/airsl/src/lib.rs                — modify  delete the platform gate; crate doc states the targets
crates/airsl/src/error.rs              — modify  Error::UncheckablePath's doc (verify plan 07 first)
crates/airsl/src/modules/time.rs       — modify  the jiff rationale and the CLOCK_MONOTONIC comment
CLAUDE.md                              — modify  build requirement; the containment-rule sentence
README.md                              — modify  the "Linux and macOS only" paragraph
crates/airsl/README.md                 — modify  the "Linux and macOS only" paragraph
crates/airsl-cli/README.md             — modify  platform paragraph, denial example, hook launcher
crates/airsl/docs/README.md            — modify  status table gains Windows rows
crates/airsl/docs/architecture.md      — modify  stale Cargo.toml citation; C-compiler paragraph
crates/airsl/docs/sandbox.md           — modify  proc grant granularity
crates/airsl/docs/stdlib.md            — modify  determinism, monotonic, create_exclusive
crates/airsl/docs/how-to.md            — modify  atomic_write, create_exclusive, denial examples
crates/airsl/examples/README.md        — modify  the byte-for-byte promise and the sh carve-out
Cargo.toml                             — modify  the jiff dependency comment
.github/workflows/ci.yml               — modify  the comment explaining the matrix
CHANGELOG.md                           — modify  a new entry recording the platform change
.claudestacks/sdlc/2026-08-29-windows-platform-support/plans/INVENTORY.md
                                       — modify  append `## Docs sweep` (created by plan 01)
```

Two verifications recur at every task and are not repeated in prose below. Any task touching a
`.rs` file runs `cargo make dod`, because doc comments and doctests are inside the gate under
`RUSTDOCFLAGS="-D warnings"`. Any task touching a `.md` file re-runs the sweep scoped to that file
and expects its remaining hits to be exactly the triaged ones.

### Task 1 — Run the sweep and build the triage list

**Files:**
- Modify `.claudestacks/sdlc/2026-08-29-windows-platform-support/plans/INVENTORY.md`

**Steps:**

1. Run the sweep from the workspace root. This exact command is the plan's inventory mechanism, the
   same way spec §4 delegates its boundary list to `rustc` and §9 delegates its test list to a
   Windows run:
   ```
   $ grep -rniE 'unix|POSIX|/tmp|/etc|/bin|mode bits|CLOCK_|O_CREAT|O_EXCL|execvp|symlink|Linux|macOS' \
       --include='*.rs' --include='*.md' --include='*.toml' --include='*.yml' --include='*.lua' \
       crates/ *.md .github/ Makefile.toml
   ```
   It returned **121 hits across 41 files** on the tree as of 2026-08-30, before plans 02–07 land.
   That number will move — plans 02–07 add `paths/`, `#[cfg(unix)]` arms and Windows test helpers,
   all of which legitimately contain this vocabulary — so it is a baseline to record, never a figure
   to assert.
2. Triage every hit into exactly one of four buckets and record the result in `INVENTORY.md` under
   a new `## Docs sweep` heading:
   - **restate** — a promise Windows makes false (the Task 2–10 sites, plus anything new).
   - **correct** — a claim that is stale rather than unix-specific (e.g. a `file:line` that drifted).
   - **no change** — vocabulary that is still true and still unix-specific *on purpose*: a
     `#[cfg(unix)]` arm, a unix-only example, `docs/README.md:7`'s "the mix of Python, Node and
     POSIX sh a project accumulates" (a description of what `airsl` replaces, not a platform claim).
   - **covered elsewhere** — a code comment another plan in this chain rewrites. Name the plan.
     Do not edit it here; a second edit to the same comment is how two plans disagree.
3. Record the sweep's known blind spot alongside the buckets, because a later reader will otherwise
   assume the grep was sufficient. Five of the seed sites below produce **zero** hits under the
   pattern and are reachable only from the seed list — verified against the current tree:
   `CLAUDE.md:87`, `crates/airsl/src/error.rs:261-265`, `crates/airsl/docs/stdlib.md:32-33`,
   `crates/airsl/examples/README.md:82-83`, `crates/airsl/docs/architecture.md:27`.
4. Verify:
   ```
   $ grep -c 'restate\|correct\|no change\|covered elsewhere' \
       .claudestacks/sdlc/2026-08-29-windows-platform-support/plans/INVENTORY.md
   ```
   Expected: every hit from step 1 appears in `INVENTORY.md` with a bucket. An untriaged hit is an
   unfinished task, not an accepted one.

### Task 2 — Delete the platform gate

**Files:**
- Modify `crates/airsl/src/lib.rs`

**Steps:**

1. Delete `crates/airsl/src/lib.rs:33-43` outright — the explanatory comment at `:33-38` and the
   `compile_error!` at `:39-43`, including the `#[cfg(not(any(unix, windows)))]` predicate plan 01
   left as a temporary escape hatch. Spec §11 says removed, not narrowed. `#![forbid(unsafe_code)]`
   at `:31` stays exactly where it is.
2. The comment being deleted is the crate's only platform statement, so the crate doc gains one in
   its place — a short paragraph after `:29`, before `#![forbid(unsafe_code)]`, naming the supported
   targets (Linux, macOS, `x86_64-pc-windows-msvc`) and pointing at where platform difference is
   decided rather than restating it. Cite the module, per the evidence rule: `src/paths/rules.rs`
   for the lexical rules and `src/paths/containment.rs` for the comparison.
3. Do **not** restate the behavioural differences here. `docs/sandbox.md` and `docs/stdlib.md` own
   them (Tasks 5 and 6); a crate doc that duplicates them becomes the copy that goes stale.
4. Verify:
   ```
   $ cargo make dod
   ```
   Expected: green on macOS, including the rustdoc step. No `compile_error!` remains — confirm with
   `grep -n 'compile_error' crates/airsl/src/lib.rs` returning nothing.

### Task 3 — Restate the platform promise in the four front doors

**Files:**
- Modify `CLAUDE.md`
- Modify `README.md`
- Modify `crates/airsl/README.md`
- Modify `crates/airsl-cli/README.md`

**Steps:**

1. `CLAUDE.md:11-14` currently reads "Build requirements that are not negotiable: **unix only**
   (`lib.rs` has a `compile_error!` off unix, because `modules::proc` decides executability from
   mode bits) and **a C compiler**". Drop the unix-only clause; keep the C-compiler clause and widen
   it — on `x86_64-pc-windows-msvc` the requirement is MSVC Build Tools, preinstalled on
   `windows-latest`. The `compile_error!` reference must go with the clause, since Task 2 deletes
   the thing it names.
2. `README.md:31-33`, `crates/airsl/README.md:21-23`, `crates/airsl-cli/README.md:12-14` — three
   near-identical "**Linux and macOS only.**" paragraphs, each giving mode bits as the reason. Each
   becomes a supported-targets statement. Keep them near-identical: they are the same promise to
   three audiences, and letting them drift apart is how one of them stays wrong.
3. State the one thing a Windows reader must know before installing, and no more: `airsstack.proc`
   runs `.exe` only, so `npm`, `npx`, `yarn` and `tsc` — which ship as `.cmd` shims — are not
   reachable (spec premise 1.2). This is the accepted cost the spec says should not be understated,
   and a front door that omits it makes a user discover it at runtime.
4. Verify:
   ```
   $ grep -niE 'unix only|Linux and macOS only|compile_error' README.md CLAUDE.md crates/airsl/README.md crates/airsl-cli/README.md
   ```
   Expected: no hits.

### Task 4 — Restate the containment vocabulary

**Files:**
- Modify `CLAUDE.md`
- Modify `crates/airsl/src/error.rs`

**Steps:**

1. `CLAUDE.md:87` reads "`modules/guard.rs` (`PathGuard`) is the *only* place the filesystem
   containment rule is written." After plan 03 that sentence names half the rule. `PathGuard::resolve`
   stays in `crates/airsl/src/modules/guard.rs` (spec §2 rejected relocating it), but the root
   comparison moved to `crates/airsl/src/paths/containment.rs`, shared with `require_loader`,
   `manifest::validate_entry` and `loaded::recheck_entry`. Rewrite the sentence to name **both**
   halves and say which is which — resolution in `guard.rs`, comparison in `paths::containment` —
   because the value of the original sentence was telling a reader where to go, and a sentence that
   sends them to one of two places is worse than one that sends them to both.
2. Re-verify the second half of that bullet — "Every `fs` call, plus `hash.hash_file` and
   `glob.walk`, funnels through it" — against the tree after plans 03–06. It is a claim about call
   sites, and this chain changes call sites.
3. `crates/airsl/src/error.rs:261-265` — `Error::UncheckablePath`'s doc says the path "could not be
   resolved to something a grant can be checked against" and that the variant is separate from
   `Error::Denied` because "the policy did not refuse this, the path could not be given a meaning to
   refuse". A verbatim or device-namespace spelling strains that: such a path *can* be given a
   meaning, and `airsl` chooses to refuse the spelling. Widen the doc to cover "a spelling this
   runtime will not reason about", keeping the `Denied` contrast, which is still correct — the
   refusal is a property of the runtime's path vocabulary, not of any grant.
4. **Check before writing.** Plan 07 may already have restated this doc, and plan 03 introduces the
   `Unrepresentable` refusals that make it necessary. Read `crates/airsl/src/error.rs` first; if the
   doc already covers the spelling case, verify it against spec §8 and record that in
   `INVENTORY.md` rather than editing it again.
5. Verify:
   ```
   $ cargo make dod
   ```
   Expected: green. The rustdoc step renders the widened doc; `# Errors` sections naming
   `UncheckablePath` elsewhere still compile.

### Task 5 — Restate the stdlib guarantees that change

**Files:**
- Modify `crates/airsl/docs/stdlib.md`
- Modify `crates/airsl/docs/how-to.md`
- Modify `crates/airsl/src/modules/time.rs`

**Steps:**

1. **`fs.create_exclusive` is not `O_CREAT|O_EXCL`.** `crates/airsl/docs/stdlib.md:129` and
   `crates/airsl/docs/how-to.md:53` both name a POSIX flag pair with no Windows equivalent. The
   *property* survives — it rests on `create_new`, not on the flags — so describe the property
   (an atomic claim; the loser is told it lost rather than raising) and drop the flag names. Add
   spec §7's Windows caveat: on a case-insensitive volume the name space is coarser, so `CLAIM` and
   `claim` are one file, which makes the primitive stronger there rather than weaker but changes
   which names collide.
2. **`atomic_write`'s rationale is unix reasoning.** `crates/airsl/docs/how-to.md:33` gives "`/tmp`
   is not used, because a rename across filesystems is not atomic." The staging-in-the-target-
   directory decision is right on both platforms; the *reason* needs the Windows half — a rename
   across volumes, and `NamedTempFile::persist` failing with a sharing violation where unix would
   succeed, if another process holds the target open (spec §7). Note that plan 07 rewrites the twin
   comment at `crates/airsl/src/modules/fs.rs:417-418`; this is the user-facing statement of the
   same thing and the two must agree.
3. **`monotonic` does not read `CLOCK_MONOTONIC` everywhere.** `crates/airsl/docs/stdlib.md:105` and
   the code comment at `crates/airsl/src/modules/time.rs:93` both name it. The behaviour is correct
   on both platforms — `Instant` is `QueryPerformanceCounter` on Windows — so state the *property*
   (unaffected by the wall clock being adjusted under a running script) and name the mechanism per
   platform, or not at all. The surrounding argument at `stdlib.md:106-109`, that no datetime crate
   can supply this, is platform-independent and stays.
4. `stdlib.md:105` cites `src/modules/time.rs:103`, the `let monotonic = lua` binding. Editing the
   comment at `:93` shifts that line. Re-check the citation after the edit — this is the exact
   failure mode Task 11 exists for, and it is cheaper to catch here.
5. **The determinism statement is refined, not weakened.** `crates/airsl/docs/stdlib.md:32-33` reads
   "Sorted JSON keys, sorted directory listings, C-locale byte ordering, stable iteration." Every
   clause survives spec §4 and each gains precision: paths reaching a script are `/`-separated and
   never verbatim on every platform (§4), `walkdir`'s `sort_by_file_name` is an `OsStr` byte
   comparison so a case-insensitive filesystem does not perturb order (§4.3), and `globset`'s
   `backslash_escape` is pinned to `true` rather than inherited from the platform (§4.2). Do not
   soften this into "deterministic where practical" — determinism is a stated correctness property
   of the crate and the Windows work upholds it.
6. Verify:
   ```
   $ cargo make dod
   ```
   Expected: green (the `time.rs` comment is inside the gate), and
   `grep -n 'O_CREAT\|CLOCK_' crates/airsl/docs/stdlib.md crates/airsl/docs/how-to.md crates/airsl/src/modules/time.rs`
   returns nothing.

### Task 6 — Restate the `proc` surface and the denial examples

**Files:**
- Modify `crates/airsl/docs/sandbox.md`
- Modify `crates/airsl/docs/how-to.md`
- Modify `crates/airsl-cli/README.md`

**Steps:**

1. `crates/airsl/docs/sandbox.md:227-230` describes `proc` grant granularity as "an allowlist of
   executable names, matched on the program as written — so `/bin/echo` is refused when `echo` is
   granted". Two additions from spec §5.2, both load-bearing:
   - The `.exe` suffix lives in resolution (`paths::rules::executable_candidates`), not in the
     grant, so a grant of `git` permits `run{'git'}` on Windows exactly as on unix — one script and
     one grant work on both platforms unchanged.
   - The comparison stays exact and case-sensitive, so `run{'GIT'}` under a `git` grant is refused
     on Windows even though the filesystem would resolve it. That is the narrowest available
     comparison and therefore fails closed. Say so; a reader who finds it by experiment will read it
     as a bug.
2. `crates/airsl/docs/how-to.md:94` — "`--allow-exec git` does not permit `/usr/bin/git`" — keep the
   example and note that a bare name is the portable spelling, since a path-shaped program is
   reachable only under `trusted` on either platform.
3. `crates/airsl/docs/how-to.md:175` and `crates/airsl-cli/README.md:105` both show the same denial
   output using `/etc/hostname` and `/home/me/journal`. These are illustrations, not assertions, and
   the useful edit is small: keep one unix example rather than duplicating every message twice, and
   state once that the roots list is rendered in the `/` vocabulary on every platform (spec §4, the
   `crates/airsl/src/modules/guard.rs:114` row), so a Windows reader knows `C:/home/me/journal` is
   what they will see rather than a backslash spelling.
4. `crates/airsl-cli/README.md:286-292` — the `#!/bin/sh` hook launcher. Spec §12 defers *shipping*
   a `.cmd` launcher; §11 keeps the documentation change in scope. Add a sentence saying a Windows
   host needs its own launcher and that none is shipped — a claim about code that does not exist,
   stated explicitly, per the evidence rule. Do not paste an untested `.cmd` script into a README:
   the existing `sh` launcher's every line is a decision (exit 0 on every path, `exec` deliberately
   unused), and an unverified translation would carry none of that.
5. Verify:
   ```
   $ grep -nE '/bin/|/etc/|/usr/' crates/airsl/docs/sandbox.md crates/airsl/docs/how-to.md crates/airsl-cli/README.md
   ```
   Expected: only the hits Task 1 triaged as deliberate unix illustrations, each now sitting under a
   sentence that says what the Windows spelling is.

### Task 7 — Correct the build-requirement citations

**Files:**
- Modify `crates/airsl/docs/architecture.md`
- Modify `crates/airsl/src/modules/time.rs`
- Modify `Cargo.toml`

**Steps:**

1. `crates/airsl/docs/architecture.md:27` cites `Cargo.toml:44` for the `mlua` dependency. That
   citation is **stale**: the `mlua` line is `Cargo.toml:19` on the current tree. Correct it. This
   is the evidence rule failing in the exact way it is meant to prevent, and it is worth noting in
   `INVENTORY.md` as a `correct` bucket entry rather than fixing silently.
2. Rewrite the surrounding paragraph's build requirement in the same edit. "A C compiler is
   therefore a build requirement" stays true; on `x86_64-pc-windows-msvc` that compiler is MSVC
   Build Tools, and `windows-latest` preinstalls them so CI needs no extra step
   (`rust-toolchain.toml:9` pins `1.94` with `profile = "minimal"`, which resolves to the MSVC
   target there).
3. `crates/airsl/src/modules/time.rs:3-5` and `Cargo.toml:34-35` both justify choosing `jiff` over
   `time`/`chrono` because "it reads `/etc/localtime` directly instead of going through libc
   `tzset`, which is not thread-safe". The **choice stays correct** and the reason stays correct on
   unix; it needs a Windows clause, because `jiff` bundles tzdb there and there is no
   `/etc/localtime` to read. The thread-safety argument — the reason an embeddable crate cares — is
   what generalises, so lead with it and keep `/etc/localtime` as the unix mechanism.
4. Keep the two in sync. They are the same decision recorded in two places, and the `Cargo.toml`
   comment is the one a dependency review reads.
5. Verify:
   ```
   $ cargo make dod
   ```
   Expected: green, and `grep -n 'Cargo.toml:' crates/airsl/docs/architecture.md` shows `:19`.
   Confirm by hand that `sed -n '19p' Cargo.toml` is the `mlua` line.

### Task 8 — Restate the examples carve-out

**Files:**
- Modify `crates/airsl/examples/README.md`

**Steps:**

1. `crates/airsl/examples/README.md:71-73` promises "No wall-clock timestamps, no absolute paths, no
   figures that differ between Linux and macOS … running the example again reproduces it byte for
   byte." Widen the platform clause to include Windows for the twelve examples that run there, which
   spec premise 1.1 makes achievable: `airsl` returns `/`-separated paths on every platform
   precisely so an identical script produces identical bytes.
2. `crates/airsl/examples/README.md:82-83` — "**Only `sh` is ever executed.** An example that wanted
   some other program would be an example that fails on a machine without it." That reasoning is
   still right and now excludes Windows by it: `sh` is the program not present there. Per the spec
   §10 carve-out, `crates/airsl/examples/env-and-proc` and `crates/airsl/examples/denials-are-data`
   stay unix-only and nothing replaces them on Windows (spec §12 — a claim about code that does not
   exist, said explicitly).
3. Document the **mechanism**, not just the exception, because the mechanism is what keeps the
   carve-out honest: an example opts out by carrying a `unix-only` marker file in its own directory
   containing the reason; `cargo make examples` still discovers every directory by glob and
   *reports* what it skipped. A newly added example with no marker runs on Windows and fails loudly
   there. This preserves the property the glob exists to protect, recorded at `Makefile.toml:108-110`
   — "an example that is added but not listed would silently never run, which is the failure mode a
   coverage suite can least afford". (Spec §10 cites `Makefile.toml:103-106` for this comment; that
   citation has drifted and the sentence is at `:108-110` on the current tree. Use the verified one.)
4. Note that these two sites carry **no** unix vocabulary and are invisible to the Task 1 sweep;
   `:82-83` produces zero hits under the pattern. Record that in `INVENTORY.md`.
5. Verify:
   ```
   $ ls crates/airsl/examples/env-and-proc/unix-only crates/airsl/examples/denials-are-data/unix-only
   ```
   Expected: both marker files exist (created by the plan that ports `Makefile.toml`), and the README
   text matches what the task actually does. If they do not exist, the README is describing a
   mechanism that is not built — stop and reconcile with that plan rather than documenting an
   intention.

### Task 9 — Add the Windows rows to the status table

**Files:**
- Modify `crates/airsl/docs/README.md`

**Steps:**

1. `crates/airsl/docs/README.md:22-40` is the status table, and `:42-43` states the rule it follows:
   "Where a claim is about code that exists, it carries a `file:line`. Where it is about code that
   does not, it says so." Every new row obeys it. The extension-system rows are the model — the
   manifest-parser row reads `**implemented** (`src/extension/manifest.rs:189`)`, naming a function
   a reader can go check rather than a bare "yes".
2. Add rows for what this chain builds, each citing the function that implements it:
   - `x86_64-pc-windows-msvc` as a supported target — **implemented**, citing the CI matrix leg.
   - Platform path rules and the outward `/` vocabulary — **implemented**, citing
     `src/paths/rules.rs` and `src/paths/resolved.rs`.
   - The shared containment predicate — **implemented**, citing `src/paths/containment.rs`.
   - `EnvName` case identity — **implemented**, citing `src/types/env_name.rs`.
   - `.exe` resolution in `proc` — **implemented**, citing `src/modules/proc.rs`.
   Take the exact line numbers from the tree after plans 02–07, never from this plan.
3. Mark the spec §12 non-goals **explicitly not implemented** rather than omitting them, since
   omission reads as "not applicable": `%PATHEXT%` walking, long-path (`>MAX_PATH`) support as an
   engineered feature, a Windows `proc` example, a `.cmd` hook launcher.
4. Update the status prose at `:16-20`, which currently ends "what remains unbuilt is Tier 3". It
   needs a platform sentence, since a reader arriving at this file forms their platform expectation
   here before reaching any README.
5. Verify:
   ```
   $ grep -nE '\*\*implemented\*\* \(`[^`]+:[0-9]+`\)|\*\*not implemented\*\*' crates/airsl/docs/README.md
   ```
   Expected: every new row either carries a `file:line` or says the code does not exist. A row with
   neither is the failure this rule was written against.

### Task 10 — Rewrite the CI comment and add the CHANGELOG entry

**Files:**
- Modify `.github/workflows/ci.yml`
- Modify `CHANGELOG.md`

**Steps:**

1. `.github/workflows/ci.yml:32-33` reads "Unix only, deliberately. `lib.rs` refuses to build off
   unix and says why; adding windows-latest back here without doing that work first only
   re-reports it." (`:31` is the blank `#` separator above it.) The work was done, so the comment
   becomes the record of *why the third leg is there*: the matrix compiles vendored Lua from C on
   three toolchains, and the host modules that touch paths, processes and time are where a
   platform break hides — the argument already made at `:28-30`, now extended rather than
   contradicted. Keep the `deny` job's `ubuntu-latest`-only comment as it is; `deny.toml:13-17`
   deliberately views the whole graph regardless of target.
2. Record why the Developer Mode step exists, next to it — spec §9's symlink tests need
   unprivileged symlink creation, and a maintainer reading the workflow should not have to find
   that in a plan.
3. `CHANGELOG.md` has no `## Unreleased` heading today: every heading is a released pair
   (`:12`, `:42`, `:102`, `:115`). Add one, and say in the entry that the version numbers are
   deliberately not chosen — publishing a release that advertises the target is a spec §12 non-goal
   and belongs to a separate chain.
4. The entry records the platform change and the two unix-observable behavioural changes spec §12
   names, because those are the only things an existing unix user's `cargo update` would notice:
   `normalize("/..")` returns `"/"` where it returned `"..//"`, and a path containing an interior
   NUL is now refused at the guard as `Error::UncheckablePath` rather than failing later as
   `Error::Io`. Follow the file's own format — Keep a Changelog headings, prose that says what
   changed and why, not a bare bullet list.
5. Verify:
   ```
   $ grep -n 'Unix only, deliberately' .github/workflows/ci.yml && head -20 CHANGELOG.md
   ```
   Expected: the `grep` returns nothing, and the CHANGELOG's first heading below the preamble is
   `## Unreleased`.

### Task 11 — Close the sweep and re-verify every citation

**Steps:**

1. Re-run the Task 1 command verbatim. Every remaining hit must map to a Task 1 triage entry in the
   `no change` or `covered elsewhere` bucket. A hit with no entry means a plan landed text nobody
   triaged.
2. Re-verify every `file:line` citation in the documentation, not only the ones this plan edited.
   Plans 02–07 add and move code, and a citation into a moved file is now wrong whether or not this
   plan touched it. Enumerate them:
   ```
   $ grep -rnoE '`(src|crates)/[A-Za-z0-9_/.-]+\.rs:[0-9]+(-[0-9]+)?`' \
       crates/airsl/docs/*.md crates/airsl/examples/README.md CLAUDE.md README.md \
       crates/airsl/README.md crates/airsl-cli/README.md
   ```
   Check each against the tree. The ones most at risk are those pointing into files plans 03–07
   modify: `src/modules/time.rs:103` (`stdlib.md:105`), `src/modules/ext.rs:123` and `:149`
   (`stdlib.md`), `src/types/chunk_name.rs:60` and `src/script.rs:118` (`examples/README.md`),
   and the `src/extension/*` citations in `extensions.md` and `docs/README.md`.
3. **Do not "correct" `crates/airsl/docs/sandbox.md`'s `src/state.rs` and `src/debug.rs`
   citations.** Those files do not exist in this crate — they are `mlua`'s sources
   (`sandbox.md:135` cites `src/state.rs:673` for `Lua::sandbox`), and a mechanical check will flag
   them as broken every time. Note it in `INVENTORY.md` so the next sweep does not re-litigate it.
4. Verify:
   ```
   $ cargo make dod
   ```
   Expected: green — fmt, clippy, rustdoc, tests, doctests, all warnings-as-errors. The rustdoc step
   is what proves the edited doc comments in `lib.rs`, `error.rs` and `time.rs` still render, and the
   doctest step is what proves no example in a rewritten doc block was broken by the rewrite.

---

## Verification summary (plan-level)

- `cargo make dod` green on macOS, and via CI on Linux and Windows. Doc comments and doctests are
  inside the gate; a documentation plan that does not run it has not been verified.
- The Task 1 sweep, re-run at Task 11, returns only hits carrying a triage entry. The count is
  recorded in `INVENTORY.md`, not asserted here.
- `grep -rn 'compile_error' crates/airsl/src/` returns nothing.
- No document claims unix-only support, and no document names `O_CREAT|O_EXCL`, `CLOCK_MONOTONIC`,
  or mode bits as a cross-platform mechanism.
- Every `file:line` in the documentation resolves to what it claims, checked after plans 02–07
  landed rather than before. `crates/airsl/docs/architecture.md`'s `mlua` citation reads
  `Cargo.toml:19`.
- `crates/airsl/docs/README.md`'s status table has a Windows row for every area this chain built,
  each with a `file:line`, and an explicit not-implemented row for each spec §12 non-goal.
- Every unix-specific statement that survives does so because a triage entry says why — a
  `#[cfg(unix)]` arm, a unix-only example, or a description of what `airsl` replaces.
