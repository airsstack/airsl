---
status: done
created: 2026-11-25
depends-on: [01, 02, 03, 04, 05, 07]
built-on: d8c7bb7f05b4
---

# Shipped examples run under the new rules Implementation Plan

**Goal:** Keep every program in `crates/airsl/examples` running, with its README transcript matching what it prints.

**Architecture:** `cargo make examples` (`Makefile.toml:98-146`; CI `ci.yml:90-91`) runs every
example directory and `airsl check`s every shipped `.lua`. Spec §12 names the examples that break:
`text-toolkit` and `multi-file-project`, plus `policy-presets` and `hello-eval`, whose output
changes. Every other example's README transcript is re-run and compared. Each example keeps
demonstrating what it was written for: the pure ones grant exactly the members they use.

**Tech Stack:** Rust 2024, Lua 5.4, cargo-make (`examples`).

**Content authority:** spec §12 (the Examples table and the sentence after it).

---

## Files that change

```
crates/airsl/examples/text-toolkit/main.rs                — modify: pure with stdout and clock granted; the policy line
crates/airsl/examples/text-toolkit/toolkit.lua            — modify: write through stdio.write; header comment
crates/airsl/examples/text-toolkit/README.md              — modify: "grants nothing" sentences; transcript
crates/airsl/examples/multi-file-project/main.rs          — modify: stdout on the pure row; a pure row with require granted
crates/airsl/examples/multi-file-project/which_require.lua — modify: write through stdio.write; recognise the refusing require
crates/airsl/examples/multi-file-project/README.md        — modify: transcript
crates/airsl/examples/policy-presets/README.md            — modify: transcript
crates/airsl/examples/hello-eval/README.md                — modify: transcript and the "grants nothing" sentence
crates/airsl/examples/README.md                           — modify: the text-toolkit row
crates/airsl/examples/text-toolkit/toolkit_test.lua       — modify: the "needs no grant" comments (:7, :80)
crates/airsl/examples/denials-are-data/README.md          — modify: "identical under every policy" (:58-64)
crates/airsl/examples/denials-are-data/denials.lua        — modify: the same claim in a comment (:49)
Makefile.toml                                             — modify: the shipped-tests comment (:153-155)
crates/airsl/examples/*/README.md                         — modify: any other transcript the re-run shows changed
```

## Order of work

### Task 1 — Run `text-toolkit` under `pure` with exactly stdout and the clock

**Files:**
- Modify `crates/airsl/examples/text-toolkit/main.rs`:
  - build the engine on
    `Policy::pure().with_grants(GrantSet::sealed().with_access(HostAccess::Stdout).with_access(HostAccess::Clock))`;
  - the `policy:` line printed at `:24-32` prints the grant set's `Display` instead of the
    `none`/`some` choice;
  - the module doc (`main.rs:1-10`), which says the example "needs no grant at all" under "the
    tightest preset, granting nothing", names the two members it is given.
- Modify `crates/airsl/examples/text-toolkit/toolkit.lua`:
  - a local function writing a line through `airsstack.stdio.write` replaces every `print` call;
  - the header comment (`:7-8`), which says `time.now` and `path.absolute` "need no authority",
    names `clock` and `cwd` instead.
- Modify `crates/airsl/examples/text-toolkit/README.md` (`:3`, `:5-8`, `:101`, `:118`),
  `crates/airsl/examples/text-toolkit/toolkit_test.lua` (`:7`, `:80`), the `toolkit.lua` header
  (`:1-10`), and the `crates/airsl/examples/README.md:52` row: the example grants `stdout` and `clock` on `pure`, not nothing. Re-run the transcript and paste
  it.

**Change:** The example's output is the same lines as before, apart from the policy line. Its point
is unchanged: `pure`, given only what the toolkit uses.

**Proof:** This is an example, not a library file, so the proof is its run.
- `cargo run -q -p airsl --example text-toolkit` exits 0.
- Its output matches the README transcript pasted in this task.

**Done when:** `cargo make examples` passes and the Definition of Done (`cargo make dod`) is green.
Commit `docs(examples): run text-toolkit under pure with stdout and the clock`.

### Task 2 — Show the refusing and the granted `require` in `multi-file-project`

**Files:**
- Modify `crates/airsl/examples/multi-file-project/which_require.lua`:
  - lines are written through `airsstack.stdio.write` instead of `print`;
  - `flavour()` and `parent_target()` (`:14-39`) recognise the refusing `require`. Today they test
    `require == nil`; now they also catch a `pcall(require, …)` whose error contains
    `require denied:`, and report it as `refused`.
- Modify `crates/airsl/examples/multi-file-project/main.rs`:
  - the row loop at `:54-65` gives its `pure` row
    `Policy::pure().with_grants(GrantSet::sealed().with_access(HostAccess::Stdout))`;
  - a fourth row, `pure+require`, adds `HostAccess::Require`;
  - the comment above the loop names four rows.
- Modify `crates/airsl/examples/multi-file-project/README.md`: re-run the transcript and paste it; the
  prose at `:79-83` ("answers with three values") names the fourth, `Refused`.
- Modify `crates/airsl/examples/README.md:53`: "three different functions" becomes the four rows.

**Change:** The four rows read: `trusted` native; `confined` confined; `pure` refused;
`pure+require` confined. The one shared script writes every row through `stdio`, which `trusted` and
`confined` grant and the two `pure` rows are given.

**Proof:**
- `cargo run -q -p airsl --example multi-file-project` exits 0.
- It prints the four rows above, matching the pasted README transcript.

**Done when:** `cargo make examples` passes and `cargo make dod` is green. Commit
`docs(examples): show the refusing and the granted require`.

### Task 3 — Re-run every other example transcript

**Files:**
- Modify `crates/airsl/examples/policy-presets/README.md`. Its `grants=` lines change for
  `confined` (`main.rs:56` prints `policy.grants()`), and its probe line changes for `pure`.
- Modify `crates/airsl/examples/hello-eval/README.md`. `:20` (`grants: none`) becomes the
  `confined` access list; the sentence at `:37` ("grants nothing") says what `confined` grants.
- Modify every other `crates/airsl/examples/*/README.md` whose transcript differs on re-run. Two
  are already known: `custom-module` (`main.rs:64` prints `describe(context.grants())` under
  `confined`) and `denials-are-data` (its comment at `main.rs:23` discusses "none are granted").

Also modify these stale prose lines:
- `crates/airsl/examples/denials-are-data/README.md:58-64` and
  `crates/airsl/examples/denials-are-data/denials.lua:49`: "behave identically under every policy"
  no longer holds for `path.absolute`. They name it, or narrow the claim to the functions that still
  need nothing.
- `crates/airsl/examples/README.md:92`: a shipped test runs under `confined`, which holds host access
  but no fs, env or exec grants.
- `Makefile.toml:153-155`: the comment saying shipped tests run "with no grants" says the same.

**Change:** Transcripts and the prose lines above, each checked against a run of this checkout.
Apart from the comment in `denials.lua`, no example program changes in this task.

**Proof:**
- For each example, `cargo run -q -p airsl --example <name>` output equals its README transcript.

**Done when:** `cargo make examples` passes and `cargo make dod` is green. Commit
`docs(examples): re-run transcripts for host access`.

---

## Risks

- **CI is red until this plan.** CI's `cargo make examples` fails from plan 02 or 04 until this plan
  lands. `examples` is not part of `dod`, so every earlier task's done-check holds; do not push an
  intermediate state.

- **A second refusal path.** `which_require.lua`'s classification by error text is the riskiest
  step. It now has two refusal paths: the invalid target that `confined` gives `../secrets`, and
  the `require denied:` refusal. They must stay distinct, and the `pure` row must not print
  "searched for it as a path".
- **Unrelated transcript drift.** An example whose transcript changes for a reason outside this
  spec, such as timing, is recorded as a deviation by `execute`, not rewritten to hide it.

## Proof

- `cargo make examples` and `cargo make dod` are green.

## Review findings

One reviewer over Tasks 1–3: `0🔴 4🟡 11🔵` (code), `1🟡 1🔵` (plan). Blocking: the three false-claim 🟡 below. One fix coder took every finding but the gate gap. After it: `cargo make dod-crate airsl` exit 0; `cargo make examples` exit 0 (`airsl check` 19 compiled, `airsl test` 19 passed); each changed example's stdout byte-equal to a fenced block in its README (checked by script).

- 🟡 hello-eval README/`main.rs`: "the grant set is never consulted" (every eval's `require` decision reads it) — fixed: the chunks call no host function that needs a grant.
- 🟡 text-toolkit README/`main.rs`/`toolkit.lua`: "`time` reaches nothing outside" beside a `clock` grant — fixed: names `clock` for `time.now`/`monotonic` and `cwd` for `path.absolute`.
- 🟡 `toolkit_test.lua`: "the one host-access member it uses, `clock`" (it also uses `cwd`) — fixed.
- 🟡 gate gap: `cargo make examples` checks exit status only, so transcript drift and a regressed `pure+require` row go uncaught — fixed 2026-11-25: `cargo make examples` now fails unless each example's stdout appears verbatim (CRLF-normalised) in its README; proved by changing `sums to 55` to `56` in hello-eval's README → exit 1, "example hello-eval: its output does not appear verbatim in crates/airsl/examples/hello-eval/README.md".
- 🔵 multi-file-project README: `pure` "has the same function" — fixed: "a different function".
- 🔵 multi-file-project `main.rs`: "whose meaning the surface changes" — fixed: "the surface and the grants change".
- 🔵 "four functions" (four rows, three functions) — fixed in `main.rs` header, its README transcript, and `examples/README.md`.
- 🔵 `which_require.lua`: the trusted row ran a real `require("../secrets")` search twice — fixed: `package` is tested first; output byte-equal.
- 🔵 denials-are-data README: "the one function of the two" — fixed: "`path.absolute` is the exception within `path`".
- 🔵 policy-presets README history sentence misattributed — fixed: dropped.
- 🔵 one axis with three names (exec/executable/subprocess) — fixed: "exec grant".
- 🔵 stale `file:line` citations in multi-file-project, text-toolkit, policy-presets, hello-eval — fixed: symbol citations. Older `file:line` citations in the other example READMEs predate this chain and stay.
- 🔵 text-toolkit: muddled reason for leaving `time.now`/`path.absolute` out — fixed: non-determinism only.
- 🔵 prose lines over 100 columns — fixed in the files edited; some untouched policy-presets lines remain.
- 🔵 (plan) comment-only edits in hello-eval and values-and-json `main.rs` outside Task 3's "no program changes" — accepted: the old text was false.

## Deviations

- 2026-11-25 — Tasks 1–3 share `crates/airsl/examples/README.md`, so one coder built them in order.
- 2026-11-25 — Started while plan 07 Task 2 was in flight; files disjoint.
- 2026-11-25 — Beyond the plan's file list: `hello-eval/main.rs`, `values-and-json/{main.rs,README.md}` (comment/prose only, the old text was false); `text-toolkit/README.md`'s `airsl test` transcript re-pasted (a test name had changed before this chain); the multi-file-project label column widened `%-8s` → `%-12s` to fit `pure+require`.
- 2026-11-25 — Real transcript, multi-file-project: rows `trusted require=native`, `confined require=confined`, `pure require=refused` ("refused, require is not granted"), `pure+require require=confined`, `memory require=absent`.
