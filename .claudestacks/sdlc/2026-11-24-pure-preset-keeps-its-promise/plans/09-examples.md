---
status: approved
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
