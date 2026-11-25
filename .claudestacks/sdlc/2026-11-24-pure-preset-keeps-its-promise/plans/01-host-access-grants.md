---
status: done
created: 2026-11-25
built-on: d8c7bb7f05b4
---

# Host access in the grant set Implementation Plan

**Goal:** Give `GrantSet` the seven host-access members, with `Policy::pure()` built on the new `GrantSet::sealed()`.

**Architecture:** A new `sandbox/host_access.rs` holds the public `HostAccess` vocabulary (spec
§3.1). `GrantSet` stores the members as a private `u8` bit set, so every `const` constructor stays
`const` (spec §3.2, proven by E19). `declared()` holds all seven members, `sealed()` none, and
`Default` stays `declared()`. `Policy::pure()` switches to `sealed()` (spec §3.3). Nothing enforces a
member yet: modules start checking in plans 02–05. This plan also updates every existing test whose
expectation follows from `declared()` no longer being empty (spec §11.2, §14).

**Tech Stack:** Rust 2024, `mlua` 0.12.0, cargo-make.

**Content authority:** spec §3 (all), §10.1–§10.4 (sandbox rows), §11.1 (`host_access.rs`,
`grant_set.rs` rows), §11.2 (grant_set, policy, doctor, ext_doctor rows), §14.

---

## Files that change

```
crates/airsl/src/sandbox/host_access.rs   — create: HostAccess, ALL, as_str, Display, FromStr; tests
crates/airsl/src/sandbox/grant_set.rs     — modify: access bit set; sealed, with_access, allows, access; is_empty; Display; impl Default; docs; tests
crates/airsl/src/sandbox/policy.rs        — modify: pure() uses sealed(); pure() rustdoc; confined test
crates/airsl/src/sandbox/mod.rs           — modify: `pub mod host_access;`, `pub use`, Responsibilities bullet
crates/airsl/src/lib.rs                   — modify: re-export HostAccess
crates/airsl/src/extension/negotiate.rs   — modify: the presence-assertion test's emptiness check
crates/airsl-cli/src/doctor.rs            — modify: confined grants expectation
crates/airsl-cli/src/ext_doctor.rs        — modify: four whole-output expectations
```

## Order of work

### Task 1 — Add the `HostAccess` vocabulary

**Files:**
- Create `crates/airsl/src/sandbox/host_access.rs`. Its four-question module doc says why it is its
  own file (spec §10.2).
- Modify `crates/airsl/src/sandbox/mod.rs`: add `pub mod host_access;`, a `#[doc(inline)] pub use
  host_access::HostAccess;`, and a `host_access` bullet in its Responsibilities list (spec §10.1).
- Modify `crates/airsl/src/lib.rs`: add `HostAccess` to the `pub use sandbox::{…}` list.

**Change:**
- `HostAccess` is the enum declared in spec §3.1, with the derives and `#[non_exhaustive]` shown
  there (E12).
- `HostAccess::ALL` lists the seven members in declaration order.
- `as_str` returns the lowercase names in spec §3.1.
- `Display` writes `as_str`.
- `FromStr` accepts exactly those seven names. Any other input returns `Error::InvalidName` with
  `kind: "host access"` and the reason text from spec §3.1.
- The rustdoc carries one doctest on `FromStr` showing an accepted name and a rejected one
  (strict-quality: every public item with behaviour has a doctest).

**Proof:**
- `every_member_round_trips_through_its_name`: for each member of `ALL`, `as_str` parsed with
  `FromStr` gives the same member, and `to_string()` equals `as_str`.
- `parsing_rejects_an_unknown_name_with_the_expected_list`: `"bogus"`, `""` and `"STDOUT"` each give
  `Error::InvalidName`. The text for `bogus` equals the spec §9 `HostAccess::from_str` row.
- `all_lists_the_members_in_declaration_order`: `ALL` is stdin, stdout, stderr, clock, cwd, require,
  zones, and it is sorted under `Ord`.

**Done when:** `cargo make dod-crate airsl` passes, and `cargo make dod` is green. Commit
`feat(airsl): add the HostAccess vocabulary`.

### Task 2 — Store host access in `GrantSet`, with `pure` on `sealed()`

**Files:**
- Modify `crates/airsl/src/sandbox/grant_set.rs`:
  - a private bit-set field;
  - the new methods below;
  - `is_empty`, `Display` and `Default`;
  - rustdoc on `declared()` and `sealed()`;
  - the struct-level doc example, which today says "Nothing is granted by default";
  - tests.
- Modify `crates/airsl/src/sandbox/policy.rs`:
  - `pure()` returns `GrantSet::sealed()`;
  - its rustdoc states the guarantee and points a host widening it to `sealed()` (spec §3.3);
  - the `confined_restricts_the_surface_and_imposes_both_ceilings` test (`policy.rs:168`) stops
    asserting `grants().is_empty()` and asserts instead that `grants().access()` yields all seven
    members.
- Modify `crates/airsl/src/extension/negotiate.rs`: the test ending at `:591` asserts
  `n.policy().grants().is_empty()`. It changes to assert that the negotiated fs, env and proc
  axes are empty (`fs().is_empty()`, `env().is_empty()`, `proc().is_empty()`). That is what
  "presence assertions add nothing to the policy" means once `declared()` holds members.
- Modify `crates/airsl-cli/src/doctor.rs:100`: `confined` expects
  `grants:       access stdin,stdout,stderr,clock,cwd,require,zones`.
- Modify `crates/airsl-cli/src/ext_doctor.rs`. Each expectation gains the `access …` part, because
  negotiation still starts from `declared()` until plan 06:
  - `:351` and `:356` expect
    `grants:       access stdin,stdout,stderr,clock,cwd,require,zones; read {}; exec git`;
  - `:509` and `:514` expect `grants:       access stdin,stdout,stderr,clock,cwd,require,zones`.

**Change:**
- The bit-set field and the constructors and methods are exactly the rows of spec §3.2's table:
  - `declared()` sets every bit;
  - `sealed()` sets none;
  - `unrestricted()` keeps its reach;
  - `with_access` sets one bit;
  - `allows` is true when the set is unrestricted or the bit is set;
  - `access()` iterates the members in `ALL` order, all seven for `unrestricted`.
- `declared`, `sealed`, `unrestricted`, `with_access` and `allows` are `const fn`.
- `is_empty` also requires no bit to be set.
- `Display` puts `access m1,m2,…` first (spec §3.2).
- The `#[derive(Default)]` on `GrantSet` (`grant_set.rs:52`) is replaced by an `impl Default` that
  returns `declared()`.
- `HostAccess` gains a crate-private `const fn bit(self) -> u8` here, in the task that first calls it,
  so no task leaves dead code under `-D warnings`.
- `GrantSet`'s `Debug` stays derived.
- `sealed`, `with_access`, `allows` and `access` each get an `# Examples` doctest that builds through
  `sealed()` or `declared()` and asserts `allows`/`access` (strict-quality: a new public item has a
  doctest).

**Proof:**
- `declared_holds_every_host_access_member`: `allows` is true for every member of `ALL`.
- `sealed_grants_nothing_and_is_empty`: `allows` is false for every member, `is_empty()` is true, and
  `to_string()` is `none`.
- `unrestricted_allows_every_member`: `allows` is true for every member, and `access()` yields all
  seven.
- `with_access_is_idempotent`: `sealed().with_access(Stdout).with_access(Stdout)` equals
  `sealed().with_access(Stdout)`, and `access()` yields only `stdout`.
- `display_lists_access_first_then_the_allowlists`: `sealed().with_access(Stderr)` with a read root
  `a` renders `access stderr; read {a}`.
- `the_constructors_can_build_constants`: a `const` item built from `declared()`, and one built from
  `sealed().with_access(HostAccess::Require)`, give the expected `allows` answers.
- Existing tests, changed (spec §11.2):
  - `declared_is_the_default_reach` (`:191-194`) is unchanged and still passes.
  - `declared_grants_nothing_and_waives_nothing` (`:196-201`) becomes
    `sealed_grants_nothing_and_waives_nothing`, built on `sealed()`.
  - `each_reach_renders_for_a_report` (`:216-218`) expects `sealed()` → `none`, and `declared()` →
    `access stdin,stdout,stderr,clock,cwd,require,zones`.
  - `display_renders_roots_through_the_script_vocabulary_not_native_spelling` (`:221-233`) expects
    `access stdin,stdout,stderr,clock,cwd,require,zones; read {a}; write {b}`.

**Done when:** `cargo make dod` passes (both crates, since the CLI expectations change). Commit
`feat(airsl): hold host access in GrantSet and build pure on sealed()`.

---

## Risks

- `Policy::pure()` now has an empty grant set, but no module checks a member until plans 02–05. So
  between this plan and plan 05, the library's `pure` behaves exactly as on 0.1.6.
- The CLI's `--policy pure` is wider than `Policy::pure()` until plan 07. `into_grant_set` still
  folds the flags onto `GrantSet::declared()` (`cli.rs:112-113`, `:289`), which now holds every
  member.
- The `ext_doctor` and `doctor` expectations change again in plan 06 only if that plan changes
  output. It does not: a confined ceiling still yields all seven members.
- Replacing the derived `Default` with a hand-written one is the step most likely to regress
  silently. `declared_is_the_default_reach` pins it.

## Proof

- `cargo make dod` is green.
- `cargo run -p airsl-cli -- doctor --policy pure` prints `grants:       none`.
- `cargo run -p airsl-cli -- doctor --policy confined` prints the seven members.

## Review findings

- 🟡 grant_set.rs:38 — struct doc said "starts from `sealed()`" but the example built on `declared()`. Fixed: example builds on `sealed().with_access(..)`. `cargo make dod` exit 0 (2026-11-25, after all fixes): airsl 643 lib tests + 16 doctests, airsl-cli 86 tests; fmt, clippy -D warnings, rustdoc -D warnings green.
- 🟡 grant_set.rs:426 — `allows` proven const only at runtime. Fixed: `const _: () = { … core::mem::forget(g); }` block; removing `const` from `allows` fails with E0015 at grant_set.rs:444 (coder mutation run).
- 🟡 policy.rs:91 — reverting `pure()` to `declared()` failed no test. Fixed: `pure_is_tighter_than_confined_on_every_axis_that_can_be` asserts `pure.grants().is_empty()` and no member allowed.
- 🔵 host_access.rs:9 — `Responsibilities:` not a list; doctests not under `# Examples`. Fixed.
- 🔵 sealed tests overlap (plan-named); `unrestricted().with_access(x) != unrestricted()`; host_access doc names CLI/refusal users ahead of plans 02–07; negotiate.rs:600 joined `&&` assert — open, accepted.
- 🔵 unplanned `HostAccess::ALL_BITS` and three extra tests — accepted (coverage only).
- Plan proof: `airsl doctor --policy pure` → `  grants:       none`; `--policy confined` → `  grants:       access stdin,stdout,stderr,clock,cwd,require,zones`.

## Deviations

- 2026-11-25 — Ledger taken with `grep -n '^### Task'` instead of a `task-briefer` ledger spawn; briefs were spawned per task as specified.
- 2026-11-25 — Plans 01–06 ran in one execute run on one working tree, with file-disjoint tasks from different plans coded concurrently; each plan was reviewed from a frozen patch snapshot of its own files.
- 2026-11-25 — Not committed: the execute instruction did not ask for commits, so the per-task commit messages the plan names were not created. All changes sit in the working tree.
- 2026-11-25 — Disk at 100% (242 MB free) mid-run; regenerable probe `target/` dirs in the session scratchpad were deleted to continue (614 MB free).
- 2026-11-25 — `the_constructors_can_build_constants`: a `const` item calling `.allows` on a temporary fails E0493 (GrantSet has a destructor). Built: const GrantSet items with runtime `allows` asserts, plus a const block that `forget`s the set to prove `allows` is `const fn`.
- 2026-11-25 — Added crate-private `HostAccess::ALL_BITS` beside `bit` so `declared()` stays `const` without a loop.
