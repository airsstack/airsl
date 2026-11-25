---
status: approved
created: 2026-11-25
depends-on: [01]
built-on: d8c7bb7f05b4
---

# CLI --allow-access Implementation Plan

**Goal:** Let every `airsl` subcommand that takes grant flags grant host-access members, `doctor` included.

**Architecture:** `Grants` (`cli.rs:89-109`) gains a repeatable `--allow-access NAME`, parsed
exactly in the shape E11 proves: `PossibleValuesParser` over the seven names into a `Vec<String>`.
The names come from `HostAccess::ALL` mapped through `as_str`, so there is no CLI mirror enum
(spec §10.4). `into_grant_set` turns each name into a `HostAccess` with `HostAccess::from_str`, and
keeps the `Ok` values. The parser has already restricted the names to `ALL`, so no `Err` can occur;
a comment says so. That avoids `unwrap`, which the workspace denies, and a typed `try_map` parser
that no probe ran.
`into_grant_set` (`cli.rs:112-120`) folds the flags onto the preset's own grants instead of
`GrantSet::declared()`, which keeps `pure` sealed. `Command::Doctor` flattens `Grants`, and
`main.rs` routes it through `resolve_policy`, as `run` and `test` are routed (spec §8).

**Tech Stack:** Rust 2024, clap 4.6.6 (derive), cargo-make.

**Content authority:** spec §8, §9 (`--allow-access` rows), §10.1 (CLI rows), §11.1 (`airsl-cli`
row).

---

## Files that change

```
crates/airsl-cli/src/cli.rs      — modify: Grants.access; into_grant_set folds onto a base; resolve_policy passes the preset's grants; Doctor gains grants; tests
crates/airsl-cli/src/main.rs     — modify: Doctor goes through resolve_policy
crates/airsl-cli/src/doctor.rs   — modify: tests for a pure policy with access granted
```

## Order of work

### Task 1 — Parse `--allow-access` and fold the flags onto the preset

**Files:**
- Modify `crates/airsl-cli/src/cli.rs`:
  - the `access: Vec<String>` field on `Grants`, with the help text and `value_name` from spec §8,
    and `value_parser = PossibleValuesParser::new(<HostAccess::ALL names>).map(|s| s)` as in E11;
  - `into_grant_set` takes the base `GrantSet`;
  - `resolve_policy` (`:268-292`) passes `policy.grants().clone()`;
  - the `Grants` doc comment (`:84-88`), which says "Nothing is granted by default, on any preset
    below `trusted`", is rewritten to say host access is held by `confined` and not by `pure`;
  - tests.

**Change:**
- `into_grant_set(self, base: GrantSet) -> GrantSet` parses each `access` name with
  `HostAccess::from_str`, keeps the `Ok` values, and adds each with `with_access`. Then it applies
  the fs, env and exec flags exactly as today.
- `resolve_policy` keeps its `trusted` branch (`:283-290`), which ignores the flags.
- Every subcommand flattening `Grants` (`run`, `test`, and `ExtFlags` for `ext doctor` and
  `ext fire`) accepts the flag with no further change.

**Proof:**
- `allow_access_parses_each_member_and_repeats`:
  - `run --allow-access stdout --allow-access require x.lua` parses to `["stdout", "require"]`;
  - every name in `HostAccess::ALL` parses;
  - `--allow-access stdout --allow-access stdout` parses (E11).
- `allow_access_rejects_an_unknown_name`: `--allow-access bogus` fails to parse, and the rendered
  error contains `invalid value 'bogus' for '--allow-access <NAME>'` and the possible-values line
  from spec §9 (E11).
- `pure_flags_fold_onto_sealed_not_declared`:
  - `resolve_policy(PolicyName::Pure, None, None, grants)` with `--allow-access stderr` allows
    `stderr` only;
  - with no flags, it is `is_empty()`.
- `confined_allow_access_changes_nothing`: for `PolicyName::Confined`, the grants with
  `--allow-access stdout` equal the grants with no flags.
- `trusted_ignores_allow_access`: `PolicyName::Trusted` with the flag is still `is_unrestricted()`.

**Done when:** `cargo make dod-crate airsl-cli` passes, and `cargo make dod` is green. Commit
`feat(airsl-cli): add --allow-access`.

### Task 2 — Let `doctor` take the grant flags

**Files:**
- Modify `crates/airsl-cli/src/cli.rs`:
  - `Command::Doctor` gains `#[command(flatten)] grants: Grants`;
  - the existing `doctor_describes_the_confined_preset_by_default` (`:477-482`) and
    `doctor_can_describe_another_preset` (`:484-491`) destructure with `..`;
  - tests.
- Modify `crates/airsl-cli/src/main.rs:57-60`: build the policy with
  `cli::resolve_policy(policy, None, None, grants)`, then print `doctor::report` of it.
- Modify `crates/airsl-cli/src/doctor.rs`: tests.

**Change:** `doctor` reports exactly the policy `run` would build for the same preset and grant
flags. It gets no ceiling flags (spec §16).

**Proof:**
- `doctor_accepts_the_grant_flags`: `doctor --policy pure --allow-access stderr --allow-read .`
  parses into `Command::Doctor` with those grants.
- `doctor_reports_pure_with_stderr_as_access_stderr`: `report` of the policy that `resolve_policy`
  builds for `Pure` with `--allow-access stderr` contains `grants:       access stderr`.
- `doctor_reports_pure_with_no_flags_as_none`: the same, with no flags, contains
  `grants:       none`.

**Done when:** `cargo make dod` passes. Commit `feat(airsl-cli): let doctor take the grant flags`.

---

## Risks

- **The help text.** It is user-visible and appears in the book's command reference (plan 10).
  It is written once, here, from spec §8.
- **The fold.** Changing `into_grant_set`'s base is the one step that could silently widen `pure`,
  if the base were `declared()`. `pure_flags_fold_onto_sealed_not_declared` pins it.

## Proof

- `cargo make dod` is green.
- `cargo run -p airsl-cli -- doctor --policy pure --allow-access stderr` prints
  `grants:       access stderr`.
- `cargo run -p airsl-cli -- run --policy pure --allow-access bogus x.lua` exits 2 with clap's
  message (E11).
