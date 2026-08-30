# Windows Failure Inventory

The product of plan 01. This file is **data, not design**: it records what actually happened when
the workspace built and tested on `x86_64-pc-windows-msvc`, so plans 02–08 work from observation
rather than from reading the code and guessing.

Nothing here is a fix.

**Source runs.** Two, both on GitHub Actions:

| Run | Commit | Branch | What it produced |
| --- | --- | --- | --- |
| `33265962712` | `ec89b52` | `worktree-windows-support` | the Build verdict and the Compile inventory |
| `33266258100` | `a2d41f0` | `spike/windows-test-inventory` (throwaway) | the Test inventory |

`ubuntu-latest`, `macos-latest` and `Supply chain` were green on both.

> **Revision note.** The first draft of this file mis-stated which failures the spec had already
> anticipated, in three places, and a review caught all three. Corrected below. The distribution
> across classes changed; the total, the 27 individual failures and every quoted assertion did not.

---

## Build

**Question:** does `mlua`'s `vendored` feature compile Lua 5.4 from C under MSVC? Nobody in this
project had ever observed it — the `compile_error!` in `crates/airsl/src/lib.rs` fired first.

**Answer: yes, and it links.** Both halves are observed, but in *different* runs — cite the right
one for each.

**Compiles** — run `33265962712`:

```
17:34:45  Compiling lua-src v550.1.1
17:34:46  Compiling mlua-sys v0.11.0
17:34:55  Checking  mlua v0.12.0
17:34:57  Checking  airsl v0.1.3 (D:\a\airsl\airsl\crates\airsl)
```

`mlua-sys`' build script ran and exited clean — the ~9s gap before `Checking mlua` is the C
compilation — and the run proceeded to type-check `airsl` itself.

**Links** — run `33266258100`. That run got past `clippy` to the `test` task, which is the first
time anything on Windows was actually *linked*:

```
Finished `test` profile ... in 35.03s
     Running unittests src\lib.rs (target\debug\deps\airsl-5aede28eb3dde530.exe)
```

An `.exe` that runs 477 tests is a binary statically linked against vendored Lua 5.4. Plan 01
task 3 step 5 states the expectation as *"links a static Lua 5.4"*; this run, not the first one,
is the evidence for it.

**Plan 01 task 4's stop-condition is therefore not triggered.** Every later plan's assumption that
the build succeeds is now observed rather than hoped for.

Two further build facts:

- **The whole dependency graph is Windows-clean.** Every dependency resolved and checked, including
  the ones that touch paths and processes — `globset`, `walkdir`, `tempfile`, `which`, `jiff`
  (via `jiff-tzdb-platform`), `same-file`, `winapi-util`. Nothing needs replacing or feature-gating.
  No MSVC-specific flag was needed in this repository; `find-msvc-tools v0.1.11` appears in the
  graph and located the toolchain on its own.
- **`cargo-make` itself works on Windows.** `cargo make dod` ran under `pwsh`, reported
  `cargo make 0.37.24`, and executed its tasks as ordinary `command`/`args` invocations. The
  gate's five steps carry no shell dependency.

A caution on reading the log for the "no system Lua" claim: `Compiling pkg-config v0.3.34` and
`luajit-src v210.7.2` both appear in the graph. That is the `pkg-config` *crate* being built as a
dependency, not the `pkg-config` *tool* being invoked to find a system Lua — but the log cannot
distinguish the two, so do not cite those lines as evidence either way. The positive evidence is
that `mlua-sys` compiled `lua-src`'s vendored C and linked it.

### Open tooling question — not yet observed

Three tasks use `script_runner = "@shell"` with POSIX sh bodies — `set -eu`, a `for` loop,
`basename`, `[ -f ]`, `set -x`, and an inline `RUSTDOCFLAGS="..."` assignment:

- `examples` (`Makefile.toml:111-137`) — **runs in CI**, the step after the gate
- `dod-crate` (`Makefile.toml:207-220`) — local development only
- `publish-dry-run` (`Makefile.toml:166`) — release only

cargo-make translates `@shell` to batch on Windows via `shell2batch`, which does not handle those
constructs. Neither Windows run reached the `examples` step — both failed at `cargo make dod`
first — so the prediction that it breaks is **still untested**. Record the real outcome once the
gate passes. If it does break, it is a **tooling** failure and says nothing about Windows support
of the library; it must not be filed under any test class below.

---

## Compile

Ten errors, all from run `33265962712`: **9 × `E0433`** (`could not find 'unix' in 'os'`) and
**1 × `E0599`**. rustc reported them as "2 previous errors" for `airsl (lib)` and "10 previous
errors" for `airsl (lib test)` — the lib pair is a subset of the test ten.

The old `lib.rs` comment predicted "nine resolution errors pointing at std". That was exactly
right: nine `E0433`s. It did not account for the `E0599` that follows the first one.

### `crates/airsl/src/modules/proc.rs` — the only non-test dependency

Both inside `is_executable` (`proc.rs:171-175`). This confirms the standing claim that
`proc::is_executable` is the single place library code reaches for unix.

```
error[E0433]: failed to resolve: could not find `unix` in `os`
   --> crates\airsl\src\modules\proc.rs:172:18
    |
172 |     use std::os::unix::fs::PermissionsExt as _;
    |                  ^^^^ could not find `unix` in `os`
```

```
error[E0599]: no method named `mode` found for struct `std::fs::Permissions` in the current scope
   --> crates\airsl\src\modules\proc.rs:174:64
    |
174 |         .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    |                                                                ^^^^ method not found in `std::fs::Permissions`
```

### Test-only — eight `std::os::unix::fs::symlink` sites

All `E0433`, all inside `#[cfg(test)] mod tests`, all the same shape:

| File | Line | Call |
| --- | --- | --- |
| `crates/airsl/src/require_loader.rs` | 333 | `symlink(outside.path().join("secrets.lua"), dir.path().join("s.lua"))` |
| `crates/airsl/src/require_loader.rs` | 348 | `symlink(decoy.join("m.lua"), root.join("m.lua"))` |
| `crates/airsl/src/extension/loaded.rs` | 515 | `symlink(&outside_file, dir.path().join("main.lua"))` |
| `crates/airsl/src/extension/manifest.rs` | 822 | `symlink(&target, dir.path().join("main.lua"))` |
| `crates/airsl/src/extension/negotiate.rs` | 515 | `symlink(outside.path(), dir.path().join("link"))` |
| `crates/airsl/src/modules/guard.rs` | 245 | `symlink(outside.path().join("secret"), root.join("link"))` |
| `crates/airsl/src/modules/guard.rs` | 265 | `symlink(outside_root.join("sub"), root.join("link"))` |
| `crates/airsl/src/sandbox/grants.rs` | 311 | `symlink(base.join("real"), base.join("link"))` |

**The spec had all eight already, and its file/directory split checks out.** `spec.md:559-569`
enumerates exactly these sites and assigns each to `symlink_dir` or `symlink_file`; plan 07 task 5
carries the same list. Two spot-checks against the real test bodies confirm the split:

- `sandbox/grants.rs:311` — target is `base.join("real")`, created by `std::fs::create_dir`
  (`grants.rs:310`). Directory. Spec says `symlink_dir`. Correct.
- `modules/guard.rs:245` — target is `outside.path().join("secret")`, created by `std::fs::write`
  (`guard.rs:242`). File. Spec says `symlink_file`. Correct.

The table records each target expression so the remaining six can be checked the same way when
plan 07 does the work. Getting the split wrong yields a link that resolves but is not traversable,
and Windows does not repair a mismatch after creation.

---

## Tests

Run `33266258100`, commit `a2d41f0` on the throwaway branch `spike/windows-test-inventory`.

### What ran, and what did not

```
[cargo-make] Running Task: fmt-check      PASSED
[cargo-make] Running Task: clippy         PASSED
[cargo-make] Running Task: doc            PASSED
[cargo-make] Running Task: test
     Running unittests src\lib.rs (target\debug\deps\airsl-5aede28eb3dde530.exe)
test result: FAILED. 450 passed; 27 failed; 0 ignored; 0 measured; 0 filtered out
```

**`fmt-check`, `clippy` with `-D warnings`, and rustdoc with `RUSTDOCFLAGS="-D warnings"` all pass
on Windows.** There is not one lint or doc warning to fix. Every failure below is behavioural.

`cargo test` stops at the first failing target, so **only the `airsl` lib unittests ran**. These
never executed and remain entirely unmeasured:

- `airsl-cli`'s 82 tests
- every example and bench target
- all doctests (the separate `--doc` run never started)

450 + 27 = 477 = 485 − the 8 tests the spike gated. The arithmetic accounts for every test.

### Coverage limits — read before using this list

Three blind spots, all structural:

1. **The 8 symlink tests were skipped, not run.** This inventory says nothing about whether symlink
   containment holds on Windows. That is plan 03's question and it remains open.
2. **`is_executable` was a stub returning `meta.is_file()`.** Any `proc` result below reflects the
   stub, not a design.
3. **The Developer Mode registry step is unexercised.** It succeeded on the runner, but because the
   spike gated every symlink test, nothing ever attempted an unprivileged symlink. Whether that
   step actually grants what plan 07 needs is as unmeasured as blind spot 1.

### Reconciliation — all 27, each counted once

| Class | Failures | Anticipated by |
| --- | --- | --- |
| 1 — escape-processing string literals | 2 | spec §9, by name and line |
| 2 — errors that cannot occur on Windows | 1 | spec §9 / §3.4 |
| 3 — natively-rendered paths | 9 | spec §9 / §4 |
| V — verbatim containment | 1 | spec §3.2 |
| A — unix-absolute path literals | 13 | §4.0 covers 3 of the 13; **10 are new** |
| C — `proc`, measured through a stub | 1 | characterises nothing |
| **Total** | **27** | |

Two tests — `path::absolute_normalises_what_it_produces` and
`path::absolute_makes_a_relative_path_absolute` — have *both* a class-3 and a class-A cause. They
are counted once, under class 3, and appear in class A's table marked as such. Class A therefore
lists 15 rows for 13 unique failures.

**Split by whether the spec saw it coming: 16 anticipated, 10 new, 1 unmeasured.**

- Anticipated (16) = class 1 (2) + class 2 (1) + class 3 (9) + V (1) + the three §4.0-named
  `path.rs` tests inside class A (3).
- **New (10)** = class A's five manifest-validator failures + its five `sandbox::grants` failures.
- Unmeasured (1) = class C.

The genuinely new finding is narrower and sharper than "unix-absolute path literals" as a whole:
**the manifest validator's absoluteness check, and the grant matcher.** The spec touches neither.

---

### Class 3 — natively-rendered paths (spec §9) — 9 failures

Anticipated. A `\` came back where the assertion wanted `/`.

| Test | left (Windows) | right (expected) |
| --- | --- | --- |
| `modules::fs::walk_returns_sorted_paths_relative_to_the_root` (`fs.rs:651`) | `a.txt,sub,sub\\b.txt` | `a.txt,sub,sub/b.txt` |
| `modules::glob::walk_returns_sorted_matches_relative_to_the_root` (`glob.rs:268`) | `Cargo.toml,sub\\Cargo.toml` | `Cargo.toml,sub/Cargo.toml` |
| `modules::path::join_builds_a_path_from_its_parts` (`path.rs:279`) | `a\\b\\c` | `a/b/c` |
| `modules::path::normalize_resolves_dot_and_dotdot` (`path.rs:352`) | `\\a\\c` | `/a/c` |
| `modules::path::normalize_does_not_consult_the_filesystem` (`path.rs:374`) | `\\nonexistent\\b` | `/nonexistent/b` |
| `modules::path::relative_to_strips_the_base` (`path.rs:382`) | `crates\\airsl\\src` | `crates/airsl/src` |
| `modules::path::relative_to_normalises_both_sides_first` (`path.rs:390`) | `a\\b` | `a/b` |
| `modules::path::absolute_normalises_what_it_produces` (`path.rs:450`) | `D:\\a\\c` | `/a/c` |
| `modules::path::absolute_makes_a_relative_path_absolute` (`path.rs:444`) | panicked with `D:\a\airsl\airsl\crates\airsl\a` | — |

The last **two** rows carry a `D:`. `std::path::absolute` does not merely change separators; it
prefixes the current drive. Those two are the tests with both a class-3 and a class-A cause.

### Class 2 — errors that cannot occur on Windows (spec §9) — 1 failure

`modules::guard::a_dotdot_below_a_directory_that_does_not_exist_is_refused_rather_than_guessed`
(`guard.rs:304`):

```
called `Result::unwrap_err()` on an `Ok` value:
    "\\\\?\\C:\\Users\\runneradmin\\AppData\\Local\\Temp\\.tmpxkPwIV\\ok.txt"
```

Spec §3.4 predicted this precisely: `GetFullPathNameW` collapses the `..` before the guard's
`ParentDir` arm can refuse it, so the call succeeds where unix refuses. The `Ok` value also carries
a raw `\\?\` prefix out to a caller — spec §3.2 and §4, visible in the same line.

### Class 1 — escape-processing string literals (spec §9) — 2 failures

**Anticipated by the spec, by name, line and mechanism.** `spec.md:605-612` says a temp path on a
`windows-latest` runner contains `\U`, an invalid TOML escape, so the manifest fails to parse — and
names `extension/host.rs:467-471` and `extension/loaded.rs:375-379` as the TOML basic strings at
fault. Those are the `format!` bodies of exactly the two tests that failed:

| Test | site the spec named |
| --- | --- |
| `extension::host::loading_the_same_name_twice_is_a_duplicate_and_runs_no_code` (`host.rs:494`) | `host.rs:467-471` |
| `extension::loaded::a_required_capability_outside_the_ceiling_is_denied_before_any_engine_exists` (`loaded.rs:403`) | `loaded.rs:375-379` |

```
cannot parse manifest `C:\Users\RUNNER~1\AppData\Local\Temp\.tmpgo0IpZ\extension.toml`:
TOML parse error at line 7, column 18
  |
7 | fs.write = ["C:\Users\RUNNER~1\AppData\Local\Temp\.tmpgo0IpZ"]
  |                  ^
invalid unicode 8-digit hex code
```

The spec also prescribes the fix — normalise to `/` before interpolation, which premise §1.1's
input vocabulary accepts. No decision is needed; plan 07 implements what is already specified.

**One residual question the spec does not answer.** Its class 1 is scoped to *tests* interpolating
temp paths. The same mechanism hits a real extension author who writes
`fs.write = ["C:\Users\me\app"]` in their own `extension.toml`, and `\U` in `C:\Users` makes that
the common spelling rather than an edge case. Nothing in the spec says whether that is supported,
refused, or documented, and "invalid unicode 8-digit hex code" will not lead anyone to their
backslash. That is a modest docs-and-error-message point for plan 08 — not a design gap, and much
narrower than this file's first draft claimed.

---

## Class A — unix-absolute path literals — 13 failures (15 rows; 2 shared with class 3)

A hardcoded `/repo`, `/a/b`, `/home/x/app/journal` or `/` is **not absolute on Windows**: it has a
root but no drive prefix, so `Path::is_absolute` is false. Two distinct consequences follow.

**Spec coverage is partial.** `spec.md:348-350` names five `path.rs` tests that carry unix-shaped
expectations and must take platform-conditional ones — `:425-434`, `:437-439`, `:442-447`,
`:449-451`, `:454-459` — and its prose already states that `std::path::absolute("/a/b")` prepends
the current drive. All five failed, exactly as written. Two of them are counted under class 3
above; the other three are in A2 below. **What the spec does not cover is the other ten.**

### A1 — the manifest validator rejects them — 5 failures, all new

`capabilities.fs.read` requires an absolute path after expansion, and a unix-spelled one is not:

| Test | reason |
| --- | --- |
| `extension::host::load_dir_loads_in_directory_order_and_reports_failures` (`host.rs:535`) | ``invalid manifest field `capabilities.fs.read`: `/` is not absolute after expansion`` |
| `extension::manifest::a_capability_in_both_blocks_is_dropped_from_optional_fs_paths_too` (`manifest.rs:811`) | ``ManifestInvalid { field: "capabilities.fs.read", reason: "`/home/x/app/journal` is not absolute after expansion" }`` |
| `extension::manifest::the_full_example_validates_with_expanded_absolute_paths` (`manifest.rs:576`) | same, `/home/x/app/journal` |
| `extension::manifest::a_tempdir_root_under_var_still_canonicalises_to_match_the_entry` (`manifest.rs:842`) | `assertion failed: Manifest::from_dir(dir.path(), &vars()).is_ok()` |
| `extension::negotiate::a_required_read_outside_the_ceiling_is_denied_naming_the_roots` (`negotiate.rs:344`) | same, `/` |

The `manifest.rs:842` row is worth spelling out, because the first draft of this file filed it
under TOML escaping and that was wrong. It writes the *static* `FULL` manifest
(`manifest.rs:497-517`) via `write_manifest`, with `vars()` = `APP_HOME=/home/x/app`
(`manifest.rs:488-489`). No Windows path is ever interpolated into it and there is no `\U`
anywhere. Its cause is identical to `the_full_example_validates_with_expanded_absolute_paths`:
`/home/x/app/journal` is not absolute on Windows.

### A2 — `std::path::absolute` silently prefixes the current drive — 8 failures, 5 new

`/repo` becomes `D:\repo` (the CI workspace is on `D:`), so grant roots and checked paths stop
matching:

| Test | detail | new? |
| --- | --- | --- |
| `sandbox::grants::a_read_root_covers_itself_and_its_descendants` (`grants.rs:258`) | `assertion failed: grant.allows_read(Path::new("/repo"))` | **new** |
| `sandbox::grants::read_and_write_are_independent_authorities` (`grants.rs:280`) | `assertion failed: grant.allows_read(Path::new("/repo/src"))` | **new** |
| `sandbox::grants::a_write_root_outside_every_read_root_is_not_readable` (`grants.rs:290`) | `assertion failed: grant.allows_write(Path::new("/var/state/a"))` | **new** |
| `sandbox::grants::a_root_that_does_not_exist_yet_is_made_absolute_but_kept` (`grants.rs:344`) | left `["D:\\definitely\\not\\here"]`, right `["/definitely/not/here"]` | **new** |
| `sandbox::grants::several_roots_are_all_honoured` (`grants.rs:353`) | `assertion failed: grant.allows_read(Path::new("/a/x"))` | **new** |
| `modules::path::is_absolute_distinguishes_the_two_kinds_of_path` (`path.rs:426`) | left `false`, right `true` | spec §4.0 `:425-434` |
| `modules::path::absolute_leaves_an_absolute_path_alone` (`path.rs:438`) | left `D:\\a\\b`, right `/a/b` — and it did **not** leave it alone | spec §4.0 `:437-439` |
| `modules::path::absolute_does_not_require_the_path_to_exist` (`path.rs:455`) | left `D:\\nonexistent\\deep\\file.lua`, right `/nonexistent/deep/file.lua` | spec §4.0 `:454-459` |
| `modules::path::absolute_normalises_what_it_produces`, `absolute_makes_a_relative_path_absolute` | counted under class 3 | spec §4.0 `:449-451`, `:442-447` |

**The ten new failures are the finding.** The spec worked out what `path.absolute` should do; it
did not work out that the same fact reaches the *manifest validator* and the *grant matcher*, which
are authority boundaries rather than rendering. Plans 04 and 06 must treat "what does a unix-spelled
absolute path mean in a manifest, and in a grant root" as a decision.

## Class C — `proc`, measured through a stub — 1 failure

`modules::proc::which_finds_a_granted_program_on_the_path` (`proc.rs:289`):

```
called `Result::unwrap()` on an `Err` value: Lua { chunk: "test",
  source: FromLuaConversionError { from: "nil", to: "String",
                                   message: Some("expected string or number") } }
```

`which` returned `nil`. Consistent with spec §5 — the fixture names a program without `.exe` and
`PATHEXT` is unimplemented — but the `is_executable` stub was in the path, so read this as "`proc`
is unmeasured on Windows" rather than as a characterised failure.

It has a second cause that will outlive the first: the test also asserts `found.ends_with("/sh")`
(`proc.rs:279`), a class-3 separator mismatch that fails on Windows even once `which` resolves
correctly. Fixing `which` alone will not make this test pass.

## Class V — the verbatim containment bug, observed — 1 failure

`extension::negotiate::a_nonexistent_path_inside_the_ceiling_is_still_granted` (`negotiate.rs:483`)
denied a path that is plainly inside its root:

```
[Denial {
   capability: FsRead("\\\\?\\C:\\Users\\runneradmin\\AppData\\Local\\Temp\\.tmp8WiTqP/does-not-exist-yet"),
   detail: "outside the granted read roots: \\\\?\\C:\\Users\\runneradmin\\AppData\\Local\\Temp\\.tmp8WiTqP"
}]
```

The root is `\\?\C:\...\.tmp8WiTqP`; the path is that same root joined with a **forward slash**.
Rust's `std::path` treats only `\` as a separator inside a verbatim (`\\?\`) path, so
`.tmp8WiTqP/does-not-exist-yet` parses as a single `Component::Normal` that cannot equal
`.tmp8WiTqP`, and the component-wise `starts_with` fails.

**Where the `/` comes from matters for the fix.** It is not a `Path::join`. The manifest literal
`fs.read = ['$HOME/does-not-exist-yet']` is expanded *textually* (`negotiate.rs:481`), so the
forward slash is already inside the string before any `Path` sees it. Plan 03's fix has to catch
this at expansion time, not only at join time.

This is spec §3.2's hole, confirmed in the wild — and it shows the flaw cuts **both** ways. §3.2
describes a false *approval* (a `..` hidden inside a verbatim component, escaping the root); this
is a false *denial* of a legitimate path. Plan 03's fix — reject unrepresentable spellings, strip
the verbatim prefix, one containment predicate — addresses both, and this failure is the regression
test it should carry.

---

## What plans 02–08 should change because of this

- **Plans 04 and 06:** the ten new class-A failures. Decide what a unix-spelled absolute path means
  in a **manifest** and in a **grant root** — spec §4.0 settled `path.absolute` and nothing else.
- **Plan 03:** unchanged in priority. The verbatim hole was observed live, but symlink containment
  is still entirely unmeasured, and so is the Developer Mode step meant to enable testing it.
- **Plan 07:** budget for `airsl-cli`'s 82 tests, the doctests and the examples, none of which have
  ever run on Windows. The 27 here are a floor, not a total. Class 1's fix is already specified;
  class C needs two fixes, not one.
- **Plan 08:** the residual class-1 question — what an extension author on Windows should write in
  `extension.toml`, and an error message that names the cause.

---

## Disposition — every recorded failure, and where it was answered

Added after plans 02-07. **Read the method note first: none of these is marked from a Windows run.**
No Windows runner is reachable from the development host, and until plan 07's task 5 the crate's
test target did not compile off unix at all (`cargo check --target x86_64-pc-windows-gnu -p airsl
--all-targets` failed with seven `E0433`). Each row below is therefore marked from **reading the
current tree**, cited by test name, plus the cross-target type-check that task 5 brought to zero
errors. The CI leg is what turns these from *answered* into *observed*, and nothing here should be
read as the second thing.

| Class | Count | State | Where |
| --- | --- | --- | --- |
| 1 — escape-processing string literals | 2 | fixed | plan 07 task 4. Nine escape-processing sites converted, not the two measured here — the sweep found eleven interpolation sites where the plan named six. |
| 2 — errors that cannot occur on Windows | 1 | fixed | plan 03. `a_dotdot_below_a_directory_that_does_not_exist_is_refused_rather_than_guessed` carries both platform arms with the reasoning written out. Plan 07 task 6 confirmed rather than redid it. |
| 3 — natively-rendered paths | 9 | fixed | plans 04 and 07. Plan 04 re-spelled the renderings; plan 07 task 6 rebuilt the assertions that were still constructed with `.display()`. |
| V — verbatim containment | 1 | fixed | plan 03 closed the false denial. The test's own input, built by `canonicalize()`, was class 4 and went to plan 07 task 5. |
| C — `proc` measured through a stub | 1 | fixed | plan 05. `which_finds_a_granted_program_on_the_path` now has both arms, and `which_renders_the_found_path_without_backslashes` sits beside it. |
| A — unix-absolute path literals | 13 | fixed, with a judgement recorded | plan 07 task 8, and see below. |
| 4 — verbatim `canonicalize()` test input | not measured here | fixed | plan 07 task 5. Caused by plan 03's own door check, so it postdates this file's measurement entirely. |

Nothing is closed as flaky, skipped or ignored. No test was deleted and none is `#[ignore]`d.

### Class A carried a decision, not just a fix

This file assigned class A's ten new failures to plans 04 and 06. **Neither took them**: plan 04's
`is_absolute`/`absolute` work is `modules/path.rs`'s own functions, and plan 06 does not mention the
subject. They were answered in plan 07 task 8, and the answer is that **the production behaviour is
correct and does not change**. `extension/manifest.rs` refuses a path that is not absolute after
expansion, and a driveless `/home/x/app` names a different file depending on the current drive, so
refusing it is the fail-closed answer for a capability request. `sandbox/grants.rs` falls through to
`std::path::absolute`, which resolves against the current drive — documented behaviour, and what the
operating system would do. All thirteen were fixtures asking for "some absolute path" and spelling
it in a way that is only absolute on one platform. They now build it through a helper.

### What this file still does not cover, and why that matters more than the 27

Its own coverage note already says the 27 are a floor. Two of its blind spots turned out to hide
real defects, both found only in review of the plan-07 diff:

- **`airsl-cli` has class-A sites of its own.** `cargo test` stops at the first failing target, so
  only `airsl`'s library tests ever ran; the 82 CLI tests are unmeasured. Four fixtures in
  `airsl-cli/src/ext_doctor.rs` hardcode a driveless absolute path and would fail on Windows. They
  were invisible here and invisible to the task written from here.
- **The examples are unmeasured too.** `crates/airsl/examples/extension-host/extensions/broken/extension.toml`
  requests `fs.read = ["/"]`, so on Windows that example's failure changes from a ceiling denial to
  an invalid manifest — falsifying a byte-for-byte output promise while the exit code stays 0.

The lesson is the one this file already states and that four rounds of work then failed to apply:
**a list measured from a run that stopped early is a floor for the targets it reached and says
nothing at all about the ones it did not.**

---

## Docs sweep

Run from the workspace root, verbatim:

```
grep -rniE 'unix|POSIX|/tmp|/etc|/bin|mode bits|CLOCK_|O_CREAT|O_EXCL|execvp|symlink|Linux|macOS' \
    --include='*.rs' --include='*.md' --include='*.toml' --include='*.yml' --include='*.lua' \
    crates/ *.md .github/ Makefile.toml
```

Measured on the tree with plans 02–10 landed and before this closing pass: **294 hits across 54
files** — up from 261/48 recorded at the start of plan 07, because plans 02–07 added `paths/`,
`test_support.rs`, and the Windows arms the earlier count predates. After the one fix this pass made
(below), the count is **293 across 54 files** — verifiable by re-running the command above and
piping through `wc -l` and `cut -d: -f1 | sort -u | wc -l`.

### Arithmetic

Per-file hit counts on the closing tree (`cut -d: -f1 sweep.txt | sort | uniq -c`), which sum to 293:

```
 42 crates/airsl/src/paths/rules.rs
 30 crates/airsl/src/modules/proc.rs
 27 crates/airsl/src/test_support.rs
 25 crates/airsl/src/modules/path.rs
 23 crates/airsl/src/modules/guard.rs
 17 crates/airsl/src/modules/fs.rs
  9 crates/airsl/src/sandbox/grants.rs
  7 crates/airsl/docs/how-to.md
  6 crates/airsl/src/modules/env.rs
  6 crates/airsl-cli/src/test_support.rs
  5 crates/airsl/src/extension/negotiate.rs
  5 crates/airsl/docs/sandbox.md
  5 crates/airsl-cli/src/ext_doctor.rs
  4 Makefile.toml
  4 crates/airsl/src/types/env_name.rs
  4 crates/airsl/src/modules/time.rs
  4 crates/airsl-cli/README.md
  4 CHANGELOG.md
  4 .github/workflows/ci.yml
  3 crates/airsl/src/sandbox/grant_set.rs
  3 crates/airsl/src/require_loader.rs
  3 crates/airsl/src/extension/manifest.rs
  3 crates/airsl/src/extension/loaded.rs
  3 crates/airsl/README.md
  3 crates/airsl/examples/text-toolkit/toolkit.lua
  3 crates/airsl/docs/README.md
  2 crates/airsl/src/types/require_target.rs
  2 crates/airsl/src/paths/resolved.rs
  2 crates/airsl/src/modules/glob.rs
  2 crates/airsl/src/modules/ext.rs
  2 crates/airsl/examples/text-toolkit/toolkit_test.lua
  2 crates/airsl/examples/README.md
  2 crates/airsl/examples/multi-file-project/README.md
  2 crates/airsl/examples/filesystem-grants/README.md
  2 crates/airsl/examples/filesystem-grants/main.rs
  2 crates/airsl/examples/env-and-proc/README.md
  2 crates/airsl/examples/env-and-proc/main.rs
  2 crates/airsl/examples/denials-are-data/README.md
  2 CLAUDE.md
  1 README.md
  1 crates/airsl/src/paths/containment.rs
  1 crates/airsl/src/modules/hash.rs
  1 crates/airsl/src/lib.rs
  1 crates/airsl/examples/text-toolkit/README.md
  1 crates/airsl/examples/multi-file-project/main.rs
  1 crates/airsl/examples/extension-host/README.md
  1 crates/airsl/examples/extension-host/main.rs
  1 crates/airsl/examples/env-and-proc/child.lua
  1 crates/airsl/examples/denials-are-data/denials.lua
  1 crates/airsl/docs/tutorial.md
  1 crates/airsl/docs/stdlib.md
  1 crates/airsl/docs/architecture.md
  1 crates/airsl-cli/src/test_runner.rs
  1 crates/airsl-cli/src/cli.rs
```

54 files, sum of the column above is 293. Every one of these 293 hits is accounted for below; none
is silently skipped.

### Buckets

**restate — 0 remaining, 1 survivor found and fixed by this pass.** Tasks 2–10 cleared every restate
site named in the plan. One did not get cleared: `crates/airsl/examples/filesystem-grants/README.md`
was outside plan 08's own file list (Task 5 restated `docs/stdlib.md` and `docs/how-to.md`'s
`create_exclusive`/`atomic_write` prose but not this per-example README, which made the identical
promise). Two lines there named POSIX mechanisms that do not hold on Windows:

- `:59` said `create_exclusive`'s exclusivity "is `O_CREAT|O_EXCL`" — restated to name the property
  (rests on `create_new`, wins/loses rather than raises) instead of the flag pair.
- `:63` said `atomic_write` avoids `/tmp` "because a rename across filesystems is not atomic" —
  restated to name both platforms' temp directories and both reasons a cross-volume rename is not
  atomic, matching the wording already in `src/modules/fs.rs:449-459`'s doc comment.

Both lines also carried stale `file:line` citations into `src/modules/fs.rs` and `src/sandbox/grants.rs`
(the whole README's citation set had drifted — see "Citation re-verification" below); all nine
citations in that file were corrected in the same pass.

**correct — 0.** No sweep hit in this closing pass was a drifted `file:line` sitting inside the
unix-vocabulary sweep pattern itself (`Cargo.toml:19` and other `.toml`/`.md` citation drift is
tracked separately below, under citation re-verification, because the sweep regex does not match a
bare `Cargo.toml:19` — it has no unix/POSIX/symlink/etc. vocabulary on that line).

**no change — 292 (293 minus the one fixed `filesystem-grants/README.md` hit at `:63`, which now
correctly describes both platforms and stays in the sweep as a `no change` hit going forward).**
Grouped by file, all homogeneous within each file:

| File | Hits | What they are |
| --- | ---: | --- |
| `crates/airsl/src/paths/rules.rs` | 42 | `PathFlavor::Posix` variant/arms, `#[cfg]`-free test names (`_under_posix`, `_on_windows`), doc comments explaining the unix/Windows split this module exists to hold |
| `crates/airsl/src/modules/proc.rs` | 30 | `#[cfg(unix)]` test arms, mode-bit executability doc comments, `execvp` mentioned as what unix's shell does and this crate deliberately does not replicate |
| `crates/airsl/src/test_support.rs` | 27 | the shared symlink-fixture helper — explicitly named a `no change` site by the task brief |
| `crates/airsl/src/modules/path.rs` | 25 | `#[cfg(unix)]` test arms and comments marking `#[cfg(windows)]` siblings as unverified on this host |
| `crates/airsl/src/modules/guard.rs` | 23 | `#[cfg(unix)]` symlink-containment test arms, `/etc/hostname` used as an arbitrary outside-the-grant path literal in test fixtures (parses fine in the crate's own `/`-vocabulary on every platform) |
| `crates/airsl/src/modules/fs.rs` | 17 | the `atomic_write` doc comment naming `/tmp` and `%TEMP%` as the two platforms' temp dirs (already correctly dual-platform), `#[allow]`-free unix epoch comment, `/etc/hostname` test literals |
| `crates/airsl/src/sandbox/grants.rs` | 9 | `#[cfg(unix)]` arms, macOS `/tmp` → `/private/tmp` symlink test comments |
| `crates/airsl/docs/how-to.md` | 7 | the `atomic_write`/`create_exclusive`/denial-example prose Task 5–6 already restated for both platforms; each unix mention sits beside its Windows equivalent |
| `crates/airsl/src/modules/env.rs` | 6 | `#[cfg(unix)]` case-identity test arms |
| `crates/airsl-cli/src/test_support.rs` | 6 | the CLI's own copy of the unix-spelled-literal-to-platform-literal test helper — same bucket as the library's `test_support.rs` |
| `crates/airsl/src/extension/negotiate.rs` | 5 | macOS `/var` → `/private/var` symlink test comments |
| `crates/airsl/docs/sandbox.md` | 5 | symlink containment prose (platform-neutral concept) and the `proc` grant paragraph Task 6 restated with the Windows `.exe`/case-sensitivity addition |
| `crates/airsl-cli/src/ext_doctor.rs` | 5 | unix-spelled-literal test helper comments, macOS `/var` symlink comments |
| `Makefile.toml` | 4 | the `unix-only` marker-file mechanism doc comment, and the `publish-dry-run` task's unix-only release-cutting comment |
| `crates/airsl/src/types/env_name.rs` | 4 | `#[cfg(unix)]` arm and doc comments explaining the Windows-fold-vs-unix-exact rule this type exists to hold |
| `crates/airsl/src/modules/time.rs` | 4 | the module doc comment naming `/etc/localtime` as unix's mechanism beside Windows' bundled tzdb (already dual-platform, per Task 5) |
| `crates/airsl-cli/README.md` | 4 | the restated supported-targets paragraph, the `sh` hook launcher (deliberately unix-only, Windows sentence added beside it), the single denial example |
| `CHANGELOG.md` | 4 | the `## Unreleased` entry's "Linux and macOS" phrase (describing what stays true) and the historical `airsl 0.1.0` entry's "Unix only" line, correctly left as a record of what was true at that release |
| `.github/workflows/ci.yml` | 4 | the matrix's `macos-latest`/`ubuntu-latest` legs and the Developer Mode step's symlink-privilege comment |
| `crates/airsl/src/sandbox/grant_set.rs` | 3 | `#[cfg(unix)]`-adjacent doc example and test comments |
| `crates/airsl/src/require_loader.rs` | 3 | symlink-escape test name and unix-identity-rendering comment |
| `crates/airsl/src/extension/manifest.rs` | 3 | symlink test fixtures, macOS `/var` comment |
| `crates/airsl/src/extension/loaded.rs` | 3 | symlink test name/comment, unix-identity-rendering comment |
| `crates/airsl/README.md` | 3 | the restated supported-targets paragraph and two symlink-containment mentions |
| `crates/airsl/examples/text-toolkit/toolkit.lua` | 3 | `/etc/shadow`/`/etc/hosts` literals used as arbitrary path-shaped strings in a log-parsing example |
| `crates/airsl/docs/README.md` | 3 | the "Python, Node and POSIX sh" description of what `airsl` replaces (explicitly not a platform claim per the plan), the restated cross-platform build statement, the explicit not-implemented row naming the unix-only `env-and-proc` example |
| `crates/airsl/src/types/require_target.rs` | 2 | symlink doc comment, unix-spelled literal in a test fixture list |
| `crates/airsl/src/paths/resolved.rs` | 2 | doc comments on what canonicalisation does and does not resolve (symlinks) |
| `crates/airsl/src/modules/glob.rs` | 2 | backslash-escape unix/Windows comment, `/etc` test literal |
| `crates/airsl/src/modules/ext.rs` | 2 | a test comment about exercising the Windows rendering rule on a macOS/Linux host |
| `crates/airsl/examples/text-toolkit/toolkit_test.lua` | 2 | same `/etc/...` literal usage as the example script it tests |
| `crates/airsl/examples/README.md` | 2 | the byte-for-byte reproducibility promise (now naming all three platforms) and the `sh`-only carve-out naming Windows explicitly |
| `crates/airsl/examples/multi-file-project/README.md` | 2 | symlink-containment prose |
| `crates/airsl/examples/filesystem-grants/README.md` | 2 | symlink-containment prose (`:47`) and the now-corrected `atomic_write` temp-dir sentence (`:63`, both platforms named) |
| `crates/airsl/examples/filesystem-grants/main.rs` | 2 | macOS `/tmp` symlink comment |
| `crates/airsl/examples/env-and-proc/README.md` | 2 | `sh` resolving to `/bin/sh` on macOS vs `/usr/bin/sh` on usrmerge Linux — a unix-only example's own internals |
| `crates/airsl/examples/env-and-proc/main.rs` | 2 | unix-only-example doc comment and grant-matching comment |
| `crates/airsl/examples/denials-are-data/README.md` | 2 | `/` resolving identically on Linux and macOS — a unix-only example's own internals |
| `CLAUDE.md` | 2 | the widened C-compiler build requirement (names Linux/macOS as what `cc` already covers) and the restated two-halves containment sentence |
| `README.md` | 1 | the restated supported-targets paragraph |
| `crates/airsl/src/paths/containment.rs` | 1 | doc comment on what inputs are assumed already symlink-resolved |
| `crates/airsl/src/modules/hash.rs` | 1 | `/etc/hostname` test literal |
| `crates/airsl/src/lib.rs` | 1 | the crate doc's restated supported-targets sentence |
| `crates/airsl/examples/text-toolkit/README.md` | 1 | `/etc/hosts` literal used in an `is_absolute` example |
| `crates/airsl/examples/multi-file-project/main.rs` | 1 | symlink-containment comment |
| `crates/airsl/examples/extension-host/README.md` | 1 | "unix shows `/`" naming this platform's rendering beside the Windows equivalent |
| `crates/airsl/examples/extension-host/main.rs` | 1 | `#[cfg(unix)]` |
| `crates/airsl/examples/env-and-proc/child.lua` | 1 | unix-only-example internals |
| `crates/airsl/examples/denials-are-data/denials.lua` | 1 | unix-only-example internals |
| `crates/airsl/docs/tutorial.md` | 1 | a unix-spelled `--allow-read /etc` example, illustrative only |
| `crates/airsl/docs/stdlib.md` | 1 | "a unix run and a Windows one" — naming both platforms |
| `crates/airsl/docs/architecture.md` | 1 | symlink-containment mention |
| `crates/airsl-cli/src/test_runner.rs` | 1 | `/etc/hostname` test literal |
| `crates/airsl-cli/src/cli.rs` | 1 | unix-spelled-literal test helper comment |

**covered elsewhere — 0.** Every plan in this chain (02–10) has landed; there is no remaining plan
for a future edit to defer to.

### Sweep blind spot

Five sites carry **no** hit under this pattern and are reachable only from the plan's seed list, not
from grep — confirmed against the current tree, each already correctly restated by Tasks 2–10:

- `CLAUDE.md:87` — the containment sentence's opening line names no unix vocabulary itself; the
  vocabulary sits two lines later (`:89`, `symlink-free`), which the sweep does catch.
- `crates/airsl/src/error.rs:261-265` — `Error::UncheckablePath`'s doc, restated to "a spelling this
  runtime will not reason about", names no platform word at all.
- `crates/airsl/docs/stdlib.md:32-33` — the determinism paragraph's opening two lines name no unix
  word; the platform-specific mechanism is a few lines further down, which the sweep does catch
  (`:35`, `to_script_string`).
- `crates/airsl/examples/README.md:82-83` — the "Only `sh` is ever executed" paragraph's own
  citation line and the sentence naming Windows explicitly carry no sweep vocabulary.
- `crates/airsl/docs/architecture.md:27` — the `Cargo.toml:19` mlua paragraph names no unix word on
  that exact line; `mode bits`/`unix` do not appear until later in the surrounding prose.

A grep-only sweep would have missed all five; each was verified by reading, per the plan's own
two-halves method (mechanical sweep plus the seed list).

### Citation re-verification (Task 11)

Re-ran the sweep and confirmed every one of the 293 remaining hits above maps to `no change` — no
untriaged hit and no surviving `restate` entry.

Re-verified every `file:line` citation reachable from the Task 11 grep
(`crates/airsl/docs/*.md`, `crates/airsl/examples/README.md`, `CLAUDE.md`, `README.md`,
`crates/airsl/README.md`, `crates/airsl-cli/README.md`) against the current tree, starting from the
existing citation audit at
`.airsstack/cc/plugins/claudestacks/handoff/20260830-165350-25df/15-orchestrator-citation-audit.md`.
Two of its "worth correcting" rows had already been fixed by the time this pass ran
(`docs/stdlib.md`'s `time.rs` citation now lands on the `monotonic` binding); the rest were fixed in
this pass:

- `docs/extensions.md:169,203` — `src/modules/ext.rs:104` → `:89` (the `on` binding, not the
  `.map_err(fail)?;` inside its closure).
- `docs/extensions.md:204` — `src/modules/ext.rs:116` → `:111` (the `granted` binding).
- `docs/stdlib.md:177` — `src/modules/ext.rs:149` (a blank line) → `:154` (`fn granted_table`).
- `examples/README.md:111` — `src/script.rs:118` (a doc-comment fence) → `:119`
  (`pub fn with_name`).

The two borderline rows the audit named — `docs/README.md:44`/`extensions.md:201` and
`examples/README.md:101`, both landing on a `#[must_use]` one line above the item's signature — were
left, per the audit's own guidance.

**Not corrected, and not to be re-litigated:** `crates/airsl/docs/sandbox.md:133-135` cite
`src/state.rs` and `src/debug.rs`. Those are **mlua's own sources** (the embedded VM's crate, not
this one), not files that exist anywhere in this crate. A mechanical citation check will flag them
as unresolvable every time it runs; that is expected and is not a defect in the documentation.

**Beyond the Task 11 grep's scope:** while restating `examples/filesystem-grants/README.md`'s
`atomic_write`/`create_exclusive` prose (above), its own nine `file:line` citations into
`src/modules/fs.rs`, `src/sandbox/grants.rs` and `src/modules/guard.rs` were found drifted too (code
moved under it the same way it moved under the docs the Task 11 grep does cover) and were corrected
in the same edit. The other per-example `README.md` files under `crates/airsl/examples/*/` were not
built or edited by plan 08 and were not audited here — a full citation sweep of that directory is
outside this pass's scope and is recorded as a gap for a future citation audit to pick up, not as a
finding this pass answered.
