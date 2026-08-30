---
status: done
created: 2026-08-30
depends-on: [02]
---

# Environment Name Identity Implementation Plan

**Goal:** Give environment variable names one identity that matches the platform's own.

**Architecture:** A new `types::EnvName` becomes the single key type for the two places a variable
name is compared — `EnvGrant`'s allowlist (`crates/airsl/src/sandbox/grants.rs:150`) and
`Overlay`'s map (`crates/airsl/src/modules/env.rs:29-32`). Its `Eq`, `Ord` and `Hash` fold ASCII
case on Windows and compare exactly on unix, so the crate stops holding two different opinions
about whether `Path` and `PATH` are the same name. The fold rule itself is a pure function taking
an explicit `CaseRule`, with a `NATIVE` constant selecting the compile-time rule — the same shape
plan 02 established for `PathFlavor`, and for the same reason: the Windows branch has to be
exercisable on a unix host or it stays unrun until CI reaches a Windows runner.

`EnvName` is **total, not validating**. That is the crux of this plan and is spelled out in Task 1.

**Tech Stack:** Rust 1.94, `std::collections::{BTreeMap, BTreeSet}`, no new dependencies.

**Content authority:** spec §6, §6.1, §6.2, and premise §1.3.

**Why `depends-on: [02]`.** No code here consumes `crates/airsl/src/paths/`. The dependency is
ordering only: plan 02 introduces the flavour-parameterised-rule-plus-`native`-const idiom, and
`CaseRule`/`NATIVE` in Task 1 mirrors it. Landing them the other way round would put two spellings
of the same idiom in the tree and invite a reviewer to unify the second against the first.

---

## File structure

```
crates/airsl/src/types/env_name.rs   — create  EnvName, CaseRule, NATIVE, the fold rule
crates/airsl/src/types/mod.rs        — modify  register and re-export EnvName
crates/airsl/src/lib.rs              — modify  add EnvName to the type re-export at :79
crates/airsl/src/sandbox/grants.rs   — modify  EnvGrant's set is keyed by EnvName
crates/airsl/src/modules/env.rs      — modify  Overlay's map is keyed by EnvName; env.all() rebuilt
```

Every task ends with a scoped `cargo test`; Task 7 runs the gate. Tests are colocated
`#[cfg(test)] mod tests`, named as sentences, and comments explain the decision rather than the
line.

### Task 1 — `types::EnvName`, total by design

**Files:**
- Create `crates/airsl/src/types/env_name.rs`
- Modify `crates/airsl/src/types/mod.rs`
- Modify `crates/airsl/src/lib.rs`

**Steps:**

1. **Write the doc comment first, because it is the deliverable.** `EnvName` is the deliberate
   exception to the Rust guideline's parse-don't-validate rule
   (`references/strong-types.md`, "Validate at construction … `TryFrom<&str>` / `parse` returning
   `Result<Self, Self::Error>` is the canonical entry point"), and the doc comment must say so and
   say why, or the next reviewer will correctly file it as a violation. The reasons, all four,
   belong in the comment:

   - `EnvName::new` is infallible. It wraps a `String` and rejects nothing — no `Result`, no
     `# Errors` section, no error type.
   - The surrounding API forces it. `EnvGrant::read` (`crates/airsl/src/sandbox/grants.rs:162-171`)
     is `#[must_use]` and returns `Self`, not `Result<Self, _>`; a rejecting constructor would have
     nowhere to report a failure. Widening `read` to fallible would be a breaking change to a
     public builder that spec §6 explicitly holds fixed.
   - `panic!` is denied by the workspace lints, so "reject by panicking" is not available either.
   - Rejecting nothing is nonetheless fail-closed. A name that cannot exist in a real environment —
     one containing `=`, or an interior NUL — simply never matches anything: not a host variable,
     not an overlay entry, not another grant name spelled legally. The failure mode of an
     unvalidated name is that it grants and reveals nothing, which is the outcome a validating
     constructor would have produced anyway.

   Also state the fold contract: `Eq`/`Ord`/`Hash` fold **ASCII** case on Windows and compare
   exactly on unix (premise §1.3 — the Windows environment block is case-insensitive
   unconditionally, so folding names there is sound), and that a non-ASCII name compares exactly on
   both, which is narrower than Windows' own Unicode fold and therefore fails closed for a grant.

   The module doc follows the fixed shape — what it is, why it is its own module,
   `Responsibilities:`, `Non-responsibilities:` — matching
   `crates/airsl/src/types/module_name.rs:1-11`. Its `Non-responsibilities:` line names validation
   explicitly, so the absence reads as a decision rather than an omission.

2. Write the failing tests. The fold rule is driven through **both** rules from this macOS host,
   which is the point of parameterising it:

   ```
   compare("PATH", "Path", CaseRule::FoldAscii) == Ordering::Equal
   compare("PATH", "Path", CaseRule::Exact)     != Ordering::Equal
   compare("PATH", "PATHEXT", FoldAscii)        == Ordering::Less
   compare("Path", "PATHEXT", FoldAscii)        == Ordering::Less   // folds before it compares
   compare("Ä", "ä", FoldAscii)                 != Ordering::Equal  // ASCII only, deliberately
   compare("", "", either)                      == Ordering::Equal
   ```

   Plus, on the type: under `NATIVE`, `EnvName::new("PATH") == EnvName::new("Path")` is `true` on
   Windows and `false` on unix (`#[cfg]`-conditional expectations, per spec §9's "an `EnvGrant` on
   `PATH` permits reading `Path` on Windows and does not on unix"); `as_str` returns the spelling
   as given, unfolded, on both; and equal names hash equal while unequal names are permitted to
   collide.

3. Implement:

   ```rust
   pub(crate) enum CaseRule { Exact, FoldAscii }

   pub(crate) const NATIVE: CaseRule =
       if cfg!(windows) { CaseRule::FoldAscii } else { CaseRule::Exact };

   pub(crate) fn compare(a: &str, b: &str, rule: CaseRule) -> core::cmp::Ordering;
   ```

   `compare` under `FoldAscii` compares `a.bytes().map(u8::to_ascii_uppercase)` against the same
   over `b` with `Iterator::cmp`, allocating nothing; under `Exact` it is `a.cmp(b)`. Uppercase
   rather than lowercase because that is the direction Windows itself folds an environment name.

4. `EnvName` derives only `Debug` and `Clone`. `PartialEq`, `Eq`, `PartialOrd`, `Ord` and `Hash`
   are hand-written and **all** delegate to `compare(.., NATIVE)` — deriving any one of them
   alongside a folded sibling breaks the `Hash`/`Eq` and `Ord`/`Eq` agreement that `BTreeSet` and
   `BTreeMap` rely on for correctness, not merely for tidiness. `Hash` writes
   `b.to_ascii_uppercase()` for each byte under `FoldAscii` plus a terminating byte so that
   `("AB","C")` and `("A","BC")` do not collide by construction.

5. Add `as_str(&self) -> &str`, `Display`, and `AsRef<str>`, matching
   `crates/airsl/src/types/module_name.rs:60-77`.

6. Make it `pub` and re-export it: `pub mod env_name;` plus `pub use env_name::EnvName;` in
   `crates/airsl/src/types/mod.rs:20-32`, and add `EnvName` to the list at
   `crates/airsl/src/lib.rs:81`. Its six siblings are all public and re-exported there; a
   `pub(crate)` item inside a `pub mod` whose `mod.rs` is export-only would need a `pub(crate) use`
   that reads as an unexplained exception. Update the `Responsibilities:` list in
   `crates/airsl/src/types/mod.rs:1-18` with an `EnvName` bullet — note that the module doc's
   opening line calls these "Validated newtypes", which is now false for one of the seven; reword
   it to name the exception rather than leaving the summary line wrong.

   `CaseRule`, `NATIVE` and `compare` stay `pub(crate)`: they are the mechanism, not the contract.

7. Because it is public, the guideline's newtype checklist applies. `EnvName` cannot supply the
   required error-path doctest — it has no error path. The `# Examples` doctest shows the happy
   path and, in place of an error path, the fail-closed one: constructing `EnvName::new("=C:")`
   succeeds and the doctest asserts it is simply an ordinary value that no real environment will
   ever match. That substitution is the documented exception, not an oversight.

8. Verify:
   ```
   $ cargo test -p airsl types::env_name
   ```
   Expected: the `CaseRule::FoldAscii` cases pass **on macOS**, proving the Windows rule is
   exercised without a Windows runner.

### Task 2 — Key `EnvGrant` by `EnvName`

**Files:**
- Modify `crates/airsl/src/sandbox/grants.rs`

**Steps:**

1. **Confirm the public-API claim before relying on it.** Read `crates/airsl/src/sandbox/grants.rs`
   and check each of these against the code, because the whole task is scoped by them:
   - the `names` field at `:150` is private (no `pub`);
   - `read` (`:162-171`) is generic over `S: Into<String>` and returns `Self`;
   - `allows` (`:173-177`) takes `&str` and returns `bool`;
   - `names` (`:179-185`) returns `impl Iterator<Item = &str>`;
   - `is_empty` (`:187-191`) returns `bool`.

   (Every number in this list was re-derived against the delivered tree; as approved they were
   each about ten lines low, and `allows`/`read` overlapped the wrong methods.)

   All five signatures survive this change unchanged. The **one** behavioural difference is
   `allows` on Windows. Record in the commit message and in the `EnvGrant` doc that this is the
   only public-behaviour delta.

2. Write the failing tests in `grants.rs`'s test module:
   - Split `env_names_are_matched_exactly_rather_than_by_prefix` (`:430-435`). The prefix half —
     a `HOME` grant does not admit `HOMEBREW_PREFIX` — is platform-independent and stays under that
     name. The case half, `!grant.allows("home")`, moves out: it is false on Windows.
   - Add `env_names_fold_case_on_windows_and_compare_exactly_on_unix`, with `#[cfg]`-conditional
     expectations: a grant of `PATH` admits `Path` and `path` on Windows and admits neither on
     unix. This is the test spec §9 names.
   - Check `env_names_enumerate_in_sorted_order` (`:437-442`) needs nothing: `ZED`/`ALPHA`/`MID`
     are already uppercase, so folded and exact ordering agree. Leave it.
   - `an_env_grant_admits_only_the_names_it_lists` (`:422-428`) and `every_grant_reports_emptiness`
     (`:473-479`) are unaffected.

3. Change the field to `BTreeSet<EnvName>`. `read` maps `Into::<String>::into` then `EnvName::new`;
   `names` maps `EnvName::as_str`; `is_empty` and `none()` are untouched (`BTreeSet::new` is `const`
   for any element type, so `none()` stays `const fn`).

4. `allows` becomes `self.names.contains(&EnvName::new(name))`. This allocates a `String` per call.
   Add a comment saying why the obvious avoidance is unavailable: a borrowed key type would need
   `EnvName: Borrow<str>`, and `Borrow`'s contract requires the borrowed `Ord` to agree with the
   owner's — which is exactly what folding breaks. One allocation per `env.get`/`env.set` is the
   price of not lying to `BTreeSet`.

5. Note the derived-`PartialEq` consequence on `EnvGrant` (`:148-151`) and, through it, `GrantSet`
   (`crates/airsl/src/sandbox/grant_set.rs:46`): on Windows two grants spelled with different
   casing now compare equal. That is correct — they authorise the same thing — and it is consistent
   rather than incidental, so it is documented, not worked around.

6. Confirm the two crate-internal readers need no change, since both consume `names()`'s `&str`:
   `crates/airsl/src/modules/ext.rs:181` (`ext.granted()`'s env table) and
   `crates/airsl/src/extension/negotiate.rs:220-232` (ceiling negotiation). On Windows a ceiling of
   `PATH` now admits a manifest requesting `Path`, and the negotiated grant stores `Path` — the
   same name under the platform's own identity. Verify by reading, not by assuming.

7. Verify:
   ```
   $ cargo test -p airsl sandbox::grants
   ```
   Expected: green on macOS, with the fold test taking its unix branch.

### Task 3 — Key `Overlay` by `EnvName`, and fix §6.1

**Files:**
- Modify `crates/airsl/src/modules/env.rs`

**Steps:**

1. Write the failing regression test first. This is spec §6.1 and it must fail before and pass
   after. Test `Overlay` directly rather than through `proc::which`, so the test names the defect
   rather than one of its symptoms — and build a fresh `Overlay::new()` rather than touching the
   process-wide singleton at `crates/airsl/src/modules/env.rs:218-221`, or the test leaks state
   into every other test in the binary.

   ```rust
   #[cfg(windows)]
   #[test]
   fn an_overlay_entry_is_found_under_every_casing_the_platform_calls_the_same_name() {
       // Before the fold this returned the *host's* PATH: `set` wrote the key `Path`, the
       // case-sensitive map missed on `PATH`, and `get` fell through to `std::env::var`, which
       // on Windows is case-insensitive and answers. `proc::which` (proc.rs:163) looks up
       // `PATH`, so it resolved against the host while the child spawned by `proc.run` — whose
       // `Command` env map folds — received the script's. `which` and `run` disagreed.
       let overlay = Overlay::new();
       overlay.set("Path", Some(String::from(r"C:\fixture")));
       assert_eq!(overlay.get("PATH").as_deref(), Some(r"C:\fixture"));
   }

   #[cfg(unix)]
   #[test]
   fn an_overlay_entry_is_found_only_under_the_casing_it_was_written_with() {
       // The unix twin of the test above: `Path` and `PATH` are two names here, so a `Path`
       // entry must not answer for `PATH`. Folding on unix would be a widening, not a fix.
       let overlay = Overlay::new();
       overlay.set("Path", Some(String::from("/fixture")));
       assert_ne!(overlay.get("PATH").as_deref(), Some("/fixture"));
   }
   ```

2. Change `entries` (`:29-32`) to `RwLock<BTreeMap<EnvName, Option<String>>>`. `set` (`:43-47`)
   inserts `EnvName::new(name)`. `get` (`:50-56`) builds the key once and looks it up.

3. **Preserve `get`'s two-level `Option` exactly.** The existing chain distinguishes three states
   and the distinction is load-bearing: outer `None` (no entry, or a poisoned lock) falls through
   to `std::env::var`; inner `None` means the name was explicitly unset by `env.set(name)` with no
   value and must read as `nil` without falling through. Collapsing it with a `flatten` or a `?`
   silently re-enables inheritance for a cleared name and breaks
   `set_with_no_value_makes_the_name_read_as_unset` (`:326-337`).

4. `child_entries` (`:62-67`) and the private `names` (`:70-75`) keep their `String`-returning
   signatures; they map `EnvName::as_str().to_owned()` over the keys. Add a comment on
   `child_entries` recording, per spec §4.3, why the Windows `Command` environment map is not a
   determinism exposure here: at most one overlay entry can exist per folded name, so `Command`'s
   case-insensitive map cannot collapse two of ours, and the child's environment block ordering is
   not script-observable. That is the reason no code change is needed, and it should be findable at
   the site rather than only in the spec.

5. Note and document one consequence of `BTreeMap::insert`: it replaces the value but **keeps the
   existing key**, so on Windows a script that calls `set("Path", …)` then `set("PATH", …)` stores
   one entry whose key is spelled `Path`. First spelling wins. That is deterministic, which is what
   matters; make it a comment so a later reader does not "fix" it into something that is not. Pin
   it with a `#[cfg(windows)]` test over `child_entries`.

6. Verify:
   ```
   $ cargo test -p airsl modules::env
   ```
   Expected: green on macOS with the unix twin taking effect; the Windows regression test is the
   one that flips red-to-green on the Windows leg.

### Task 4 — Rebuild `env.all()` per §6.2

**Files:**
- Modify `crates/airsl/src/modules/env.rs`

**Steps:**

1. Extract the name merge out of the closure at `:161-188` into a private free function, so it can
   be tested without an engine and without the host's real environment:

   ```rust
   /// The names `env.all()` reports, host spelling preferred, in a deterministic order.
   fn merged_names(
       host: impl IntoIterator<Item = String>,
       overlay: impl IntoIterator<Item = String>,
   ) -> Vec<String>
   ```

   It filters host names beginning with `=`, then inserts host names into a
   `BTreeMap<EnvName, String>` mapping folded identity to the spelling to report, then inserts
   overlay names with `entry(..).or_insert(..)` so a host spelling always wins, then returns the
   values in map order.

2. Write the failing tests against `merged_names` — most of them platform-independent:
   - `merged_names(["=C:", "=ExitCode", "HOME"], [])` → `["HOME"]`. Spec §6.2: `std::env::vars()`
     surfaces Windows' per-drive current-directory pseudo-variables, which are process-private
     state, not environment. Testable on macOS with a synthetic host list, which is why the
     function takes one.
   - An overlay-only name appears: `merged_names(["HOME"], ["EXTRA"])` → `["EXTRA", "HOME"]`.
   - Output order does not depend on host iteration order: the same inputs shuffled give the same
     vector.
   - `#[cfg(windows)]`: `merged_names(["Path"], ["PATH"])` → `["Path"]` — one entry, host casing.
     `#[cfg(unix)]`: the same inputs → `["PATH", "Path"]` — two names on a platform where they are
     two names.

   The fold half of the dedup is the only piece of this task that a unix host cannot exercise. It
   is thin — three lines keyed by `EnvName` — and the fold rule underneath it is exhaustively
   tested on both hosts by Task 1. Parameterising `merged_names` by `CaseRule` as well was
   considered and rejected: it would duplicate the rule selection that `EnvName` already owns, to
   cover logic the type's own tests already cover.

3. An overlay name beginning with `=` is **not** filtered. The `=` rule exists because
   `std::env::vars()` volunteers process-private state; an overlay entry is something the script
   itself wrote under a grant, and hiding a script's own write from `all()` while `get()` still
   returns it would be the more surprising behaviour. Say so in a comment next to the filter.

4. Rewire the unrestricted branch (`:164-175`) to
   `merged_names(std::env::vars().map(|(k, _)| k), o.names())`, then `o.get(&name)` per entry as
   today. Keep the sorted-output property and rewrite the comment at `:165-166`: sorting is still
   right, but the determinism that now actually matters is that the *key set* is fold-deduped, so a
   host `Path` and an overlay `PATH` cannot both appear.

5. Leave the restricted branch (`:176-184`) emitting the **grant's** spelling, not the host's.
   §6.2's host-casing rule governs the enumeration of the host environment; the restricted branch
   enumerates no host at all — its names come from the grant — and the property recorded at
   `:177-178`, "a script sees what it declared, not what the host inherited", is the one that
   governs there. Switching it to host casing would break that promise, and the branch cannot
   produce a duplicate anyway because grant names are already fold-deduped by `EnvGrant`'s
   `BTreeSet<EnvName>`. State this reasoning in the comment; it is the one interpretation call in
   this plan and should be visible to a reviewer.

6. Verify:
   ```
   $ cargo test -p airsl modules::env
   ```
   Expected: the `=` filter, the ordering and the host-preference cases all green on macOS.

### Task 5 — Repair `all_does_not_leak_the_hosts_environment`

**Files:**
- Modify `crates/airsl/src/modules/env.rs`

**Steps:**

1. Read the test at `crates/airsl/src/modules/env.rs:298-304`. It grants `AIRSL_TEST_ALL_2` and
   asserts `airsstack.env.all().PATH == nil`. On Windows this proves nothing: the host variable is
   spelled `Path`, a Lua table index is an exact byte match, and a leak would therefore surface
   under the key `Path` and leave `.PATH` nil regardless. The test would stay green with the leak
   present. It is not a Windows-only weakness either — the probe tests one spelling of one name.

2. Replace the single-key probe with a fold-insensitive scan of every key, so the assertion is
   about the property rather than about one string:

   ```lua
   local leaked = 0
   for name in pairs(airsstack.env.all()) do
     if name:upper() == 'PATH' then leaked = leaked + 1 end
   end
   return leaked
   ```

   Assert `0` under the declared grant.

3. Add the positive control in the same test, and say in a comment why it is there: a probe that
   can never fire is indistinguishable from a probe that finds nothing. Run the identical Lua under
   `Policy::trusted()` and assert the count is **greater than zero**, which establishes that the
   scan does detect a `PATH`-shaped key when one is present. Without this the repaired test is only
   a better-spelled version of the same false green.

4. Confirm by reading that the other ten tests in the module need no change, and record the reason
   for the two that are close to the line:
   - `a_trusted_policy_reads_anything` (`:352-357`) reads `PATH` through `env.get`, which reaches
     `std::env::var` — case-insensitive on Windows — so it passes there. It does not need the fold.
   - `all_returns_only_the_granted_names` (`:283-296`) asserts the grant's spelling comes back,
     which Task 4 step 5 deliberately preserves. It is the test that would fail if the restricted
     branch were switched to host casing.
   The remaining eight (`:248`, `:254`, `:263`, `:272`, `:306`, `:313`, `:326`, `:339`) use names
   that are unaffected by folding.

5. Verify:
   ```
   $ cargo test -p airsl modules::env -- --exact all_does_not_leak_the_hosts_environment
   ```
   Expected: green, and green for the right reason — the positive control asserts a non-zero count
   under `trusted`.

### Task 6 — Sweep the callers the compiler will not point at

**Files:**
- Modify `crates/airsl/src/modules/env.rs` (comments only, if anything)

**Steps:**

1. `EnvGrant`'s and `Overlay`'s signatures are unchanged, so `rustc` produces no diagnostic for any
   caller. Grep and read each of the four crate-internal sites that reason about an environment
   name and confirm none needs a change:
   ```
   $ rg -n "env\(\)\.(allows|names)|overlay\(\)|child_entries" crates/
   ```
   Expected hits and verdicts: `crates/airsl/src/modules/ext.rs:181` (consumes `&str`, unchanged),
   `crates/airsl/src/extension/negotiate.rs:220-232` (consumes `&str`, Windows behaviour follows
   the fold as intended), `crates/airsl/src/modules/proc.rs:214` (`which`'s PATH lookup —
   the §6.1 beneficiary, and the reason no change is needed there is that `Overlay::get` now folds),
   `crates/airsl-cli/src/cli.rs:118` (builds a grant from CLI flags, unchanged).

2. Do not change `proc.rs`. Its `which` is fixed by Task 3 without touching it; the `.exe`
   resolution it also needs belongs to the `proc` plan, not this one.

3. Verify:
   ```
   $ cargo test -p airsl && cargo test -p airsl-cli
   ```
   Expected: green with no test outside `types::env_name`, `sandbox::grants` and `modules::env`
   modified.

### Task 7 — Run the gate

**Steps:**

1. Verify:
   ```
   $ cargo make dod-crate airsl
   ```
   Expected: green, zero warnings. Watch specifically for `clippy::pedantic` on the hand-written
   `Hash`/`Ord` impls — a manual `Hash` beside a derived `PartialEq` (or the reverse) is exactly
   what `derived_hash_with_manual_eq` catches, and if it fires, Task 1 step 4 was not followed.

2. Verify the whole workspace, since `EnvName` reaches the public surface:
   ```
   $ cargo make dod
   ```
   Expected: green, including the `EnvName` doctest from Task 1 step 7.

---

## Verification summary (plan-level)

- `cargo make dod` green on macOS, and via CI on Linux and Windows.
- The §6.1 regression test exists, is `#[cfg(windows)]`, and demonstrably fails on the Windows leg
  before Task 3 and passes after — confirm by running it against the pre-change tree, not by
  assuming it.
- The fold rule is exercised for **both** `CaseRule` variants on a unix host, so the Windows
  identity is tested without a Windows runner.
- `EnvGrant`'s five public signatures — `none`, `read`, `allows`, `names`, `is_empty` — are
  byte-identical to their pre-change form; the only public behavioural delta in the crate is
  `allows` folding on Windows.
- `EnvName`'s doc comment states, in the type's own words, that it is total rather than validating,
  and names all four reasons: `EnvGrant::read` is infallible and `#[must_use]`, `panic!` is denied,
  a rejecting constructor has nowhere to report, and an impossible name matches nothing.
- `all_does_not_leak_the_hosts_environment` carries a positive control, so it can fail.
- `merged_names` filters `=`-prefixed host names, prefers host spelling, and is deterministic
  regardless of host iteration order.

---

## Amendments after approval

| What | Why |
|---|---|
| Task 1's `Hash`/`Eq` agreement test could not fail, and has been replaced | `names_that_compare_equal_also_hash_equal` hashed `"PATH"` against `"PATH"` - two identical strings, which agree under any deterministic `Hash`. The discriminating pair was `#[cfg(windows)]`-only, so on a unix host nothing checked the property at all. The `Hash` logic is now extracted into a rule-parameterised `hash_under(name, rule, state)` mirroring `compare(a, b, rule)`, and `hashing_agrees_with_comparison_under_either_rule` exercises both rules on any host - the same reason the comparison takes a rule explicitly. |
| Task 2's `Borrow` rustdoc made a false soundness claim, now corrected | It said violating `Borrow`'s contract "would be undefined behaviour for `BTreeSet`". It would not: `BTreeSet` is safe code and the standard library specifies only unspecified or incorrect results. The doc now describes the real consequence, a wrong-but-safe lookup miss. The decision not to implement `Borrow<str>` stands and is correctly reasoned. `allows` also moved from an allocating `contains` call to an allocation-free linear scan, since grant sets hold a handful of names and the old comment's "one allocation per call is the price" overstated the alternative. |
| Thirteen citations re-derived against the delivered tree | Every `EnvGrant` member number was roughly ten lines low and two pointed at the wrong method: `:143`→`:150` (the field), `read` `:155-164`→`:162-171`, `allows` `:166-170`→`:173-177`, `names` `:172-178`→`:179-185`, `is_empty` `:180-184`→`:187-191`, the `EnvGrant` derive `:141`→`:148-151`. Four test ranges in `grants.rs` were off by roughly sixty lines: `:358-363`→`:422-428`, `:366-370`→`:430-435`, `:373-377`→`:437-442`, `:397-402`→`:461-467`. `lib.rs:79`→`:81` (`:79` is the closing `};` of the `sandbox::{…}` block, not the `types::{…}` re-export). `proc.rs:99`→`:162-163` (`:99` is inside `run`, not `which`). `negotiate.rs:220-228`→`:219-228`. Task 2 step 1 already required confirming these before relying on them; doing so up front is the same check, run earlier. |
| Task 5's premise re-confirmed verbatim rather than assumed | `all_does_not_leak_the_hosts_environment` (`env.rs:298-304`) probes exactly one key, `.PATH`, with no case-folded scan and no positive control — the false-green shape the task describes. Reading it before rewriting it is what makes the task's claim checkable rather than asserted. |
| Two more citations re-derived at execution time, since two further plans had landed in `sandbox/grants.rs` after this plan's approval | `every_grant_reports_emptiness` `:461-467`→`:473-479` (a `ProcGrant` case-variant test was added just above it, pushing it down twelve lines). Task 6's caller sweep: `ext.rs:176`→`:181` (`for name in grants.env().names()`, the `granted()` env table), `negotiate.rs:219-228`→`:220-232` (the `env_read` block now spans slightly further, including its closing brace), `proc.rs:162-163`→`:214` (`which`'s own `env::overlay().get("PATH")` call — the earlier amendment's replacement citation still pointed inside `run`'s `Error::Io` construction, not `which`). |
