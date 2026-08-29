---
status: approved
created: 2026-08-30
depends-on: [03, 04, 05, 06]
---

# Filesystem, Errors, Tests and Tooling Implementation Plan

**Goal:** Bring the filesystem guarantees, error text, test suite and build tooling to green on
Windows.

**Architecture:** Four bodies of work that share one property — none of them changes a design
decision, all of them close the gap between a decision already taken and what the tree says. §7
weakens three `fs` guarantees and documents each at the point it is stated. §8 restates one error
variant's doc; no new variant is added. §9 is the test port, and it is **driven by
`INVENTORY.md`** — the recorded output of plan 01's actual Windows run — not by reading the test
files. §10 ports the two remaining POSIX-shell `cargo make` tasks to duckscript and finalises the
CI workflow. This plan runs last because §9's inventory cannot be cleared until plans 03–06 have
changed the behaviour the failing assertions are measuring.

**Tech Stack:** Rust 1.94, `tempfile`, `std::os::windows::fs`, `cargo-make` duckscript,
GitHub Actions `windows-latest`.

**Content authority:** spec §7 (filesystem guarantees), §8 (error handling), §9 (testing
strategy), §10 (build, CI, tooling). Documentation outside code comments — `docs/`, the
READMEs, `CHANGELOG.md` — is §11 and belongs to plan 08, not here.

---

## File structure

```
crates/airsl/src/modules/fs.rs           — modify  §7's three guarantee comments
crates/airsl/src/error.rs                — modify  restate UncheckablePath's doc
crates/airsl/src/test_support.rs         — create  #[cfg(test)] link_file / link_dir
crates/airsl/src/lib.rs                  — modify  register `#[cfg(test)] mod test_support;`
crates/airsl/src/modules/guard.rs        — modify  2 symlink sites, class 2 + class 3 tests
crates/airsl/src/require_loader.rs       — modify  2 symlink sites
crates/airsl/src/sandbox/grants.rs       — modify  1 symlink site
crates/airsl/src/extension/negotiate.rs  — modify  1 symlink site
crates/airsl/src/extension/manifest.rs   — modify  1 symlink site
crates/airsl/src/extension/loaded.rs     — modify  1 symlink site, 2 class 1 sites
crates/airsl/src/extension/host.rs       — modify  2 class 1 sites
crates/airsl/src/script.rs               — modify  2 class 3 tests
crates/airsl-cli/src/ext_fire.rs         — modify  1 class 1 site
crates/airsl-cli/src/check.rs            — modify  1 class 1 site, 1 discover test
crates/airsl-cli/src/test_runner.rs      — modify  1 discover test
crates/airsl/examples/env-and-proc/unix-only        — create  carve-out marker + reason
crates/airsl/examples/denials-are-data/unix-only    — create  carve-out marker + reason
Makefile.toml                            — modify  port `examples` and `dod-crate`; skip publish
.github/workflows/ci.yml                 — modify  remove plan 01's temporary scaffolding
```

Every Rust task verifies with `cargo make dod-crate airsl` (or `airsl-cli`) locally and proves the
Windows half on the CI leg plan 01 added. **A Rust change made in this plan that cannot be
observed on macOS is not evidence of anything until that leg is green**; state that in the
commit rather than assuming it.

### Task 1 — Document the three filesystem guarantees that weaken

**Files:**
- Modify `crates/airsl/src/modules/fs.rs`

**Steps:**

1. `atomic_write` (`crates/airsl/src/modules/fs.rs:415-434`). The property is unchanged — the
   staging file is still created in the target's directory and still renamed — but two things in
   the current text are wrong on Windows. Rewrite the doc comment at `:417-418`: "`/tmp` is
   routinely a different filesystem" is unix reasoning for a rule that holds on Windows for a
   different reason (`%TEMP%` is routinely a different volume, and `MoveFileEx` across volumes is a
   copy, not a rename). Then add the honest weakening: `NamedTempFile::persist` at `:428` can fail
   with a sharing violation on Windows where unix would succeed, because another process holding
   the target open blocks the replace rather than being silently detached from the old inode. The
   call still either fully succeeds or leaves the old contents — *atomic on success* survives;
   *succeeds whenever the directory is writable* does not.
2. `fs.stat`'s `readonly` (`crates/airsl/src/modules/fs.rs:229`). `meta.permissions().readonly()`
   is mode bits on unix and `FILE_ATTRIBUTE_READONLY` on Windows — a per-file attribute with no
   owner/group/other dimension, and one that says nothing about ACLs, which are the real
   Windows access control. Add a comment at the set site saying exactly that, so a reader does not
   take the field for a portable permission model.
3. `fs.create_exclusive` (`crates/airsl/src/modules/fs.rs:152-179`). The exclusivity property
   holds on both platforms, because it rests on `create_new` and not on a POSIX flag pair — say
   so, since `docs/stdlib.md` and `docs/how-to.md` currently claim it "is `O_CREAT|O_EXCL`" and
   plan 08 will point at this comment. Then record what is coarser: on a case-insensitive volume
   `CLAIM` and `claim` are one file, so the set of claimants a single claim excludes is *wider* on
   Windows, not narrower. Wider is the safe direction for a mutual-exclusion primitive, which is
   why this is documented rather than fixed — and §1.3's no-folding rule forbids fixing it here
   anyway, since NTFS per-directory case sensitivity means `airsl` cannot know which it got.
4. No behaviour changes in this task. If a code line moves, the task has exceeded its scope.
5. Verify:
   ```
   $ cargo make dod-crate airsl
   ```
   Expected: green, zero warnings, no test count change — 3 comments, 0 assertions.

### Task 2 — Restate `Error::UncheckablePath`

**Files:**
- Modify `crates/airsl/src/error.rs`

**Steps:**

1. The doc at `crates/airsl/src/error.rs:261-265` says the variant is separate from
   `Error::Denied` because "the policy did not refuse this, the path could not be given a meaning
   to refuse". Plan 03 made this variant carry the refusal of a verbatim or device-namespace
   spelling, which strains that sentence: `\\?\C:\x` *can* be given a meaning, and `airsl` chooses
   not to reason about it.
2. Widen the doc to cover both causes: a path with no filesystem answer (the existing `..`-through-
   a-missing-directory case, which stays as the worked example) **and** a spelling this runtime
   will not reason about. Keep the reason `Denied` is still wrong — the refusal is a property of
   the runtime's path vocabulary, not of any grant, so a grant change would not make it succeed.
3. Do **not** add a variant, a field, or a `#[non_exhaustive]` change. §8 is explicit that the
   public error surface does not grow, and the `reason: &'static str` field already carries the
   distinction to a reader of the message.
4. Verify:
   ```
   $ cargo make dod-crate airsl
   ```
   Expected: green. `missing_docs` and rustdoc-as-errors mean a malformed doc fails the gate here.

### Task 3 — Triage `INVENTORY.md` into this plan's work list

**Steps:**

1. Read `.claudestacks/sdlc/2026-08-29-windows-platform-support/plans/INVENTORY.md`, the recorded
   output of plan 01's Windows run. **That file is the work list.** The three classes below seed
   it and are not claimed to be complete — §9 says so, and four rounds of reading the code failed
   to predict the residue, which is why plan 01 exists at all.
2. Re-run the suite on the current tree, because plans 03–06 have landed since plan 01 recorded
   it. Every inventory entry is now in one of four states, and each is marked in the file:
   - **fixed upstream** — plans 03–06 changed the behaviour; the assertion now passes. Name the
     plan. Do not re-fix it here.
   - **class 1 / 2 / 3** — task 4, 6 below.
   - **CLI discover** — task 7.
   - **residue** — anything else. Task 8.
3. A failure marked "fixed upstream" that is still failing is a **finding, not a task**: it means
   an earlier plan's change did not do what it claimed. Report it rather than patching the
   assertion here; patching a test to match a wrong behaviour is how a containment guarantee gets
   quietly dropped.
4. Verify:
   ```
   $ cargo test --workspace --all-targets --all-features    # on the Windows runner
   ```
   Expected: a current failure list, every entry cross-marked against `INVENTORY.md` in one of the
   four states, and no entry unmarked.

### Task 4 — Class 1: temp paths interpolated into escape-processing literals

**Files:**
- Modify `crates/airsl/src/extension/host.rs`
- Modify `crates/airsl/src/extension/loaded.rs`
- Modify `crates/airsl-cli/src/ext_fire.rs`
- Modify `crates/airsl-cli/src/check.rs`

**Steps:**

1. Understand the failure before changing a site. A `windows-latest` temp path is
   `C:\Users\RUNNER~1\AppData\Local\Temp\…`; `\U` is not a valid TOML escape, so a manifest built
   with `format!("… fs.write = [\"{}\"]", dir.path().display())` does not parse, and the test fails
   with a TOML error that names neither the temp path nor Windows. The same path in a Lua
   single-quoted string is a Lua escape error. Both literal kinds process escapes; that is the
   whole class.
2. Fix by normalising the interpolated path to `/` before it reaches the literal — premise 1.1's
   input vocabulary accepts either separator, so the fixture means the same thing on both
   platforms and the unix expectation does not move. The five known sites:
   - `crates/airsl/src/extension/host.rs:467-471` — TOML basic string, `fs.write = ["{}"]`
   - `crates/airsl/src/extension/host.rs:476` — Lua single-quoted, `airsstack.fs.write('{}', …)`
   - `crates/airsl/src/extension/loaded.rs:375-379` — TOML basic string
   - `crates/airsl/src/extension/loaded.rs:384` — Lua single-quoted
   - `crates/airsl-cli/src/ext_fire.rs:318-326` — both literal kinds in one fixture
   - `crates/airsl-cli/src/check.rs:160` — Lua, `error('ran: {}')`
3. Do **not** reach for `paths::rules::to_script_string` from `airsl-cli`: it is `pub(crate)` and
   §9 forbids widening the public API to serve test code. Inside `airsl`'s own test modules it is
   reachable and is the right call; in `airsl-cli` the two sites normalise locally. That asymmetry
   is deliberate — note it in a comment at the CLI sites so the next reader does not "unify" it by
   exporting the helper.
4. Escape-processing interpolation is a class, not six lines. Grep the workspace for
   `format!` arguments carrying `.display()` or `.to_string_lossy()` into a `"` or `'` delimited
   TOML or Lua fixture, and fix every hit the inventory confirms — the six above are the ones plan
   01 saw fail, not the ones that exist.
5. Verify:
   ```
   $ cargo test -p airsl extension:: && cargo test -p airsl-cli --all-targets
   ```
   Expected: green on macOS unchanged; the Windows leg no longer reports a TOML or Lua parse error
   from a temp path.

### Task 5 — One symlink helper, eight call sites

**Files:**
- Create `crates/airsl/src/test_support.rs`
- Modify `crates/airsl/src/lib.rs`
- Modify `crates/airsl/src/modules/guard.rs`, `crates/airsl/src/require_loader.rs`,
  `crates/airsl/src/sandbox/grants.rs`, `crates/airsl/src/extension/negotiate.rs`,
  `crates/airsl/src/extension/manifest.rs`, `crates/airsl/src/extension/loaded.rs`

**Steps:**

1. Write `crates/airsl/src/test_support.rs` as a `#[cfg(test)]` module exposing exactly two
   functions:
   ```rust
   pub(crate) fn link_file(target: &Path, link: &Path) -> std::io::Result<()>;
   pub(crate) fn link_dir(target: &Path, link: &Path) -> std::io::Result<()>;
   ```
   On unix both delegate to `std::os::unix::fs::symlink`, which is what all eight sites call
   today. **This is not a substitution: the unix behaviour must be byte-identical afterwards.**
   On Windows they delegate to `std::os::windows::fs::symlink_file` and `symlink_dir`.
   Register it in `crates/airsl/src/lib.rs` as `#[cfg(test)] mod test_support;` beside the private
   modules at `:45-52`. The crate has no `tests/` directory and adds none — this is a crate-root
   test module, not an integration-test tree.
2. Route the three **directory** targets through `link_dir`:
   - `crates/airsl/src/modules/guard.rs:265` — links to `outside_root.join("sub")`
   - `crates/airsl/src/sandbox/grants.rs:311` — links to `base.join("real")`
   - `crates/airsl/src/extension/negotiate.rs:515` — links to `outside.path()`
3. Route the five **file** targets through `link_file`:
   - `crates/airsl/src/modules/guard.rs:245` — `outside.path().join("secret")`
   - `crates/airsl/src/require_loader.rs:333` — `outside.path().join("secrets.lua")`
   - `crates/airsl/src/require_loader.rs:348` — `decoy.join("m.lua")`
   - `crates/airsl/src/extension/manifest.rs:822` — `target`
   - `crates/airsl/src/extension/loaded.rs:515` — `outside_file`
4. Check each target against what the test actually creates before committing the split. Windows
   distinguishes the two at creation time and never repairs a mismatch: a `symlink_file` pointing
   at a directory resolves — so a test asserting the link exists still passes — but is not
   traversable, so `link/../secret` fails for the wrong reason and the containment assertion goes
   green without testing containment. That is a false green on the crate's most safety-critical
   property.
5. The helper **fails loudly** when creation is denied. On Windows an unprivileged
   `symlink_*` returns `ERROR_PRIVILEGE_NOT_HELD` unless Developer Mode is on; the helper maps that
   to a panic naming Developer Mode and the `reg add` line from §9, so a maintainer running
   locally is told what to enable. It must never skip, never `return Ok(())`, and never be wrapped
   in `#[ignore]` — these are the tests that establish the containment property, and a silent skip
   converts them into decoration. CI is already covered: plan 01 added the Developer Mode step.
6. Verify:
   ```
   $ grep -rn "os::unix::fs::symlink" crates/ ; cargo test -p airsl
   ```
   Expected: the grep returns **only** `test_support.rs`; all eight call sites now name
   `link_file`/`link_dir`; macOS test results identical to before this task.

### Task 6 — Classes 2 and 3

**Files:**
- Modify `crates/airsl/src/modules/guard.rs`
- Modify `crates/airsl/src/script.rs`

**Steps:**

1. **Class 2 — assertions of an error that cannot occur on Windows.** Plan 03's task 5 step 3
   already claims `crates/airsl/src/modules/guard.rs:295-304`
   (`a_dotdot_below_a_directory_that_does_not_exist_is_refused_rather_than_guessed`) and `:255-273`
   (`a_dotdot_through_a_symlink_does_not_escape`). **Check the tree before touching either.** If
   plan 03 gave them platform-conditional expectations, this task's work on them is to confirm it
   against the inventory and move on; if it did not, do it here — `GetFullPathNameW` collapses
   `..` before the guard sees it (§3.4), so the `ParentDir` refusal is unreachable on Windows.
   Neither test is deleted: the unix assertion is still the one that matters on unix, and the
   Windows arm asserts the §3.4 invariant instead — the guard's verdict matches what the OS opens.
2. **Class 3 — assertions of a natively-rendered path.** These fail because §4 (plan 04) re-spells
   the rendering while the expectation is still built with `.display()`:
   - `crates/airsl/src/modules/guard.rs:326-341`
     (`a_refusal_names_the_roots_that_were_granted`) asserts the message contains
     `root.display().to_string()`. Plan 04 converts the granted-root rendering at `guard.rs:114`
     to `/`, so on Windows the message and the expectation disagree. Build the expectation in the
     script vocabulary instead of the native one.
   - `crates/airsl/src/script.rs:219` asserts `script.name().as_str() == path.display().to_string()`
     — the chunk name comes from `types::chunk_name.rs:60-66`, which plan 04 converts.
   - `crates/airsl/src/script.rs:278` asserts the chunk name does **not** contain
     `dir.path().display().to_string()`. This one is a *weakening* on Windows if left alone: a
     `/`-spelled name trivially fails to contain a `\`-spelled directory, so the assertion passes
     without testing anything. Normalise the needle so the test still asserts what it was written
     to assert.
3. `script.rs:278` is the shape to look for in the residue: a class-3 assertion phrased
   negatively goes green by accident rather than red. Grep the suite for `!.*contains(` over a
   `.display()` needle and check each hit.
4. Verify:
   ```
   $ cargo test -p airsl modules::guard script
   ```
   Expected: green on macOS with the assertions unweakened; on Windows, class 2 and class 3 entries
   clear from the inventory.

### Task 7 — The two CLI tests §4 does not reach

**Files:**
- Modify `crates/airsl-cli/src/check.rs`
- Modify `crates/airsl-cli/src/test_runner.rs`

**Steps:**

1. `crates/airsl-cli/src/check.rs:199` expects `["a.lua", "a_test.lua", "lib/m.lua", "z.lua"]` and
   `crates/airsl-cli/src/test_runner.rs:247` expects `["a_test.lua", "sub/m_test.lua",
   "z_test.lua"]`. Neither is fixed by §4, and the reason is worth stating in the code: the
   compared values are built **inside the test bodies** (`check.rs:188-196`,
   `test_runner.rs:240-243`) by `strip_prefix(dir.path()).to_string_lossy()` over `PathBuf`s that
   `discover` returns, and `discover` is the CLI's own — the path never crosses an `airsl` module
   boundary, so no conversion in the library can reach it.
2. Normalise the separator in each test body, at the `to_string_lossy()` that produces the compared
   string. Keep the `/`-joined literals: they are the assertion's point.
3. Add a comment at both sites recording why the fix is local. `paths::rules` is `pub(crate)`, so
   `airsl-cli` cannot reach it, and §9 decides deliberately not to widen `airsl`'s public API —
   exporting a path-spelling helper to satisfy two test expectations would put a permanent item on
   the public surface to serve test code. Without the comment the next reader deletes the
   duplication by exporting the helper, which is the outcome the spec argued against.
4. Verify:
   ```
   $ cargo test -p airsl-cli --all-targets
   ```
   Expected: green on macOS unchanged; both discovery tests pass on Windows against `/`-joined
   expectations.

### Task 8 — Clear the residue

**Steps:**

1. Work every inventory entry still marked **residue** after tasks 4–7. §9 states plainly that the
   three classes are not claimed to be complete, and plan 01's fourth bucket exists because reading
   the code did not predict what running it found.
2. For each, decide and record which it is before fixing:
   - a **test-side** assumption (a temp path, a separator, a shell fixture) → fix the test;
   - a **behaviour** difference the spec anticipated → confirm it matches what §3–§7 says and pin
     it with a platform-conditional assertion;
   - a **behaviour** difference the spec did **not** anticipate → stop. That is a spec finding, not
     a test fix. Report it; do not resolve it by editing an assertion.
3. Windows `proc` fixtures land here if plan 05 did not take them: §9 puts `where.exe` in place of
   `sh`/`echo`/`false` on Windows, and gives
   `arguments_are_passed_without_a_shell` (`crates/airsl/src/modules/proc.rs:238-249`) a Windows
   payload exercising `&`, `|`, `^` and `%FOO%` — the unix `'a b; rm -rf *'` payload proves nothing
   about the no-shell property on a platform with different metacharacters. Check plan 05's tree
   first; do not do it twice.
4. Verify:
   ```
   $ cargo make dod    # on the Windows runner
   ```
   Expected: the full gate green on Windows. Every `INVENTORY.md` entry is marked fixed, with the
   plan that fixed it — or escalated as a spec finding. No entry is closed as "flaky" or "skipped".

### Task 9 — Port `examples` to duckscript, with a visible carve-out

**Files:**
- Modify `Makefile.toml`
- Create `crates/airsl/examples/env-and-proc/unix-only`
- Create `crates/airsl/examples/denials-are-data/unix-only`

**Steps:**

1. Rewrite `[tasks.examples]` (`Makefile.toml:97-137`) from `script_runner = "@shell"` to
   `@duckscript`, preserving every behaviour the current script has: glob the example directories,
   skip any without a `main.rs` (cargo ignores those and so does this), `cargo run --example` each
   one, then `airsl check` and `airsl test` over `crates/airsl/examples/`. The comments at
   `:98-110` explain *why* each of those exists and survive the port unchanged.
2. **Keep discovery by glob.** The comment at `Makefile.toml:103-106` records that "an example
   that is added but not listed would silently never run, which is the failure mode a coverage
   suite can least afford". A hand-kept Windows skip list inside the task would re-create exactly
   that failure mode, one platform at a time, which is why the carve-out is not one.
3. Implement the carve-out as a **marker file in the example's own directory**: `unix-only`,
   containing the reason as text. The task still globs every directory; on Windows it reads the
   marker, skips that example, and **prints the skip and its reason into the log**. A skipped
   example is visible, not absent — the difference between a carve-out and a hole.
4. Write the two markers. Both examples spawn `sh` (`crates/airsl/examples/env-and-proc/main.rs`
   and `child.lua`; `crates/airsl/examples/denials-are-data/main.rs`), and no program exists on
   both platforms that would keep the byte-for-byte output promise at
   `crates/airsl/examples/README.md:73`. The other twelve of the fourteen run everywhere.
5. `airsl check` and `airsl test` still run over the **whole** examples tree on Windows, markers
   included: `check` compiles without running, so `child.lua`'s `sh` reference costs a parse, and
   the tree's only test file (`crates/airsl/examples/text-toolkit/toolkit_test.lua`) is portable.
   Do not narrow those two commands to the unmarked set.
6. A newly added example with no marker runs on Windows and fails loudly there if it cannot. That
   is the property the glob exists to protect; state it in the task's comment so a future
   contributor adds a marker with a reason rather than an exclusion.
7. Verify:
   ```
   $ cargo make examples
   ```
   Expected: on macOS, all fourteen run exactly as before — markers are inert off Windows. On the
   Windows leg, twelve run, and the log contains two skip lines each naming its example and its
   reason.

### Task 10 — Port `dod-crate`, and skip `publish-dry-run` on Windows

**Files:**
- Modify `Makefile.toml`

**Steps:**

1. Port `[tasks.dod-crate]` (`Makefile.toml:202-220`) to duckscript. It matters more than its size
   suggests: `CLAUDE.md:23` documents it as the per-crate iteration command, so without it a
   Windows contributor's only gate is the full workspace run. Keep all five steps in order —
   `fmt --check`, `clippy -D warnings`, `doc` with `RUSTDOCFLAGS="-D warnings"`, `test
   --all-targets`, `test --doc` — and keep the missing-argument branch printing the usage line to
   stderr and exiting non-zero. `RUSTDOCFLAGS` is set per-command in the shell version; in
   duckscript it is set on the environment before the `doc` step and must not leak into the two
   `test` steps.
2. Give `[tasks.publish-dry-run]` (`Makefile.toml:158-171`) a `windows_alias` pointing at a small
   task that prints why it is skipped and exits 0. It is a release-time task, releases are cut on
   unix, and §12 lists publishing a release advertising the new target as a non-goal — so this is
   a skip with a stated reason, not an unported task. The message says that, and says where
   releases are cut.
3. The five gate tasks (`Makefile.toml:47-91`) are plain `command`/`args` and are already
   portable; `deny`, `install`, `fmt` and `clippy-fix` likewise. Do not touch them. After this
   task no `script_runner = "@shell"` remains outside the skipped release task — confirm by grep.
4. Verify:
   ```
   $ cargo make dod-crate airsl && cargo make dod-crate && grep -n 'script_runner' Makefile.toml
   ```
   Expected: the first is green; the second prints the usage line and exits non-zero; the grep
   shows `@duckscript` for `examples` and `dod-crate`, with `@shell` only under
   `publish-dry-run`.

### Task 11 — Finalise the CI workflow

**Files:**
- Modify `.github/workflows/ci.yml`

**Steps:**

1. The matrix entry (`.github/workflows/ci.yml:37`), the `shell: bash` on the diagnostic step at
   `:46-51`, and the Developer Mode step all landed in plan 01. This task removes what was
   temporary: any `continue-on-error`, any inventory-capture step, and any commented-out
   scaffolding plan 01 left behind. The Windows leg must be able to fail the build — a leg that
   cannot go red is not a gate.
2. Rewrite the comment at `:31-33`. It currently reads "Unix only, deliberately. `lib.rs` refuses
   to build off unix and says why; adding windows-latest back here without doing that work first
   only re-reports it." That work is now done; the comment should say what the matrix is *for*
   (`mlua` is vendored, so every leg compiles Lua 5.4 from C, and the path/process/time modules
   differ per platform) and that Windows is `x86_64-pc-windows-msvc` only, per §12.
3. Leave `shared-key: gate-${{ matrix.os }}` at `:56` alone — it already keys the cache per-OS —
   and leave the `deny` job on `ubuntu-latest` only, since `deny.toml:13-17` views the whole graph
   regardless of target. Adding a Windows `deny` leg would report the same findings twice.
4. `cargo make examples` (`:74-75`) now runs on the Windows leg too, with the task 9 carve-out. No
   workflow change is needed for it; confirm the skip lines appear in that leg's log.
5. Verify:
   ```
   $ gh run watch    # after pushing
   ```
   Expected: three legs — `ubuntu-latest`, `macos-latest`, `windows-latest` — all green, no
   `continue-on-error` anywhere in the file, and the Windows log showing two example skips with
   reasons.

---

## Verification summary (plan-level)

- `cargo make dod` green on Linux, macOS **and** Windows. This is the chain's headline result and
  the first time it has been true.
- `cargo make examples` green on all three, with the two `sh`-based examples reported as skipped
  on Windows and the other twelve run.
- `INVENTORY.md` is fully marked: every entry either fixed (naming the plan that fixed it) or
  escalated as a spec finding. Nothing closed as skipped or flaky.
- No test is `#[ignore]`d, and no symlink test can pass without creating a symlink. Confirm by
  grep for `ignore` and by the `test_support.rs`-only result of grepping
  `os::unix::fs::symlink`.
- The unix test results are unchanged in count and in content by tasks 4–8 — this plan ports
  assertions, it does not weaken them. `script.rs:278` is the specific one to re-read: it must
  still be capable of failing.
- No new public error variant, and no new public item on `airsl`'s API surface. Confirm against
  `cargo public-api` or by reading the diff for `pub`.
- Spec §11's documentation edits are **not** in this plan's diff; they belong to plan 08.
