---
status: approved
created: 2026-08-30
depends-on: [02]
---

# Proc Executability Implementation Plan

**Goal:** Make `airsstack.proc` resolve and spawn Windows executables without admitting a shell.

**Architecture:** `is_executable` is split into a lexical half and a filesystem half. The lexical
half — *which names to try* — moves out of `proc.rs` entirely into
`paths::rules::native::executable_candidates` (plan 02), where it is a pure function testable for
both flavours on any host. The filesystem half — existence, and on unix the mode bits — stays in
`proc.rs`, because it is the one part that genuinely needs the disk. `which` then joins each `PATH`
entry with each candidate, and on Windows *only*, `run` pre-resolves a bare program name through
that same `which` before spawning, so the two functions agree by construction. Unix keeps
`Command::new(&program)` and `execvp`'s own search, unchanged. `ProcGrant` is not touched at all:
the `.exe` lives in the candidate list, never in the grant.

**Tech Stack:** Rust 1.94, `std::process::Command`, `std::env::split_paths`,
`crates/airsl/src/paths/rules.rs` from plan 02. No new dependencies.

**Content authority:** spec §5 (§5.1, §5.2, §5.3), premise §1.2, and the `run` diagnostic in §8.
The Windows fixture choices are §9's "Subprocess fixtures" paragraph.

---

## File structure

```
crates/airsl/src/modules/proc.rs   — modify  executability, `which`, `run`, the §8 refusal, tests
crates/airsl/src/sandbox/grants.rs — modify  one added test only; ProcGrant itself is unchanged
```

`crates/airsl/src/modules/proc.rs:172` is the **only** non-test `std::os::unix` use in the
workspace — verified by `grep -rn "std::os::unix" crates --include="*.rs"`, whose other nine hits
are all `symlink` calls inside `#[cfg(test)] mod tests` (the symlink-helper plan's work) plus the comment at
`crates/airsl/src/lib.rs:34`. After Task 1 no production code in the workspace names a unix-only
`std` module outside a `#[cfg(unix)]` item.

Every task below is verified on this macOS host with `cargo test -p airsl modules::proc`, which
proves the unix arms and that the Windows arms compile is *not* provable here — a `#[cfg(windows)]`
item is not even parsed for name resolution on a unix host. The Windows arms are verified by the CI
leg plan 01 added; each task names which of its assertions that applies to, rather than claiming a
local green covers them.

### Task 1 — Split executability into a lexical half and a filesystem half

**Files:**
- Modify `crates/airsl/src/modules/proc.rs`

**Steps:**

1. Write the failing test first, in `proc.rs`'s `mod tests`. It is a filesystem test, not a Lua
   one, so it drives `is_executable` directly:
   - a temp file with mode `0o644` is not executable **on unix**, and *is* on Windows — the whole
     point of the split, and the one assertion that must be platform-conditional rather than shared
   - a temp file with mode `0o755` is executable on unix
   - a directory is never executable on either platform, since `meta.is_file()` gates both arms
   - a path that does not exist is never executable
2. Replace `crates/airsl/src/modules/proc.rs:170-175` with a shape that has no `#[cfg]` inside an
   expression — two `#[cfg]`-selected one-line helpers, so each platform's rule reads as a complete
   sentence:
   ```rust
   fn is_executable(path: &std::path::Path) -> bool {
       std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && has_execute_permission(&meta))
   }
   ```
   with `#[cfg(unix)] fn has_execute_permission` carrying the existing
   `std::os::unix::fs::PermissionsExt` + `mode() & 0o111 != 0`, and `#[cfg(windows)]` returning
   `true`.
3. The doc comment on `has_execute_permission`'s Windows arm carries the *why*, because `true`
   looks like a stub and is not one: Windows has no execute bit, so the question "may this be
   executed" has already been answered by the lexical half — a file named `git.exe` on `PATH` is
   the answer, and existence is all that is left to check. Refusing to say that in a comment is how
   a later reader "fixes" it into something wrong.
4. Note in the same comment that the mode-bit check on unix is load-bearing for *ordering*, not
   only for the verdict: a readable non-executable file early on `PATH` must be **skipped** so a
   later entry can win, which is why the check cannot be collapsed to existence on both platforms.
5. Verify:
   ```
   $ cargo test -p airsl modules::proc
   ```
   Expected: green on macOS, including the new `0o644` unix assertion. The Windows arm of that same
   test is exercised only on the CI Windows leg.

### Task 2 — Make `which` join every `PATH` entry with every candidate

**Files:**
- Modify `crates/airsl/src/modules/proc.rs`

**Steps:**

1. Write failing tests first. Two are runnable here, one is not:
   - `which_finds_a_granted_program_on_the_path` (existing, `:275-280`) keeps working on unix with
     `sh`
   - a new test: on Windows, a grant of `where` makes `airsstack.proc.which('where')` return a path
     whose lowercased form ends with `where.exe`. Assert on the **lowercased tail** and not on the
     separator, so this assertion is independent of whether the path-vocabulary plan has already converted
     `proc.rs:167`'s rendering to the `/` vocabulary
   - a new test: `which('airsl-not-installed')` is `nil` on both platforms, which on Windows now
     also means `airsl-not-installed.exe` was probed and missed
2. Rewrite `which` (`crates/airsl/src/modules/proc.rs:162-168`) over
   `paths::rules::native::executable_candidates(program)`, keeping the existing
   `env::overlay().get("PATH")` source and the existing "no fallback to the current directory"
   doc comment at `:158-161` intact:
   ```rust
   let candidates = crate::paths::rules::native::executable_candidates(program);
   std::env::split_paths(&path)
       .find_map(|directory| {
           candidates
               .iter()
               .map(|name| directory.join(name))
               .find(|candidate| is_executable(candidate))
       })
       .map(|found| found.to_string_lossy().into_owned())
   ```
3. The iteration is **directory-major**: every candidate is tried within one `PATH` entry before
   moving to the next. Write that down as a comment with its reason — candidate-major would let a
   later directory's match beat an earlier directory's, which inverts what `PATH` order means. The
   decision is unobservable today, since `executable_candidates` returns exactly one name for both
   flavours, but the return type is a `Vec` and the ordering has to be fixed by intent rather than
   by whichever loop nesting was typed first.
4. Verify:
   ```
   $ cargo test -p airsl modules::proc::tests::which
   ```
   Expected: the unix `which` tests green on macOS. The `where.exe` assertion runs on CI's Windows
   leg.

### Task 3 — Pre-resolve a bare program name on Windows only

**Files:**
- Modify `crates/airsl/src/modules/proc.rs`

**Steps:**

1. Write the failing tests first, both Windows-only:
   - under a `where` grant, `run{'where', 'where'}` succeeds — the bare-name branch, resolved to an
     absolute `where.exe` before spawning
   - under `Policy::trusted()`, `run{'C:\\Windows\\System32\\where.exe', 'where'}` succeeds — the
     separator branch, passed through unresolved. This is the only way that branch is reachable,
     because a `ProcGrant` refuses a path (`crates/airsl/src/sandbox/grants.rs:221`, pinned by
     `grants.rs:388-394`), which is exactly what the comment in the code must say
2. Insert the pre-resolution immediately above `crates/airsl/src/modules/proc.rs:109`, shadowing
   `program`, in the form spec §5.1 gives:
   ```rust
   #[cfg(windows)]
   let program = if crate::paths::rules::native::has_separator(&program) {
       program
   } else {
       which(&program).ok_or_else(|| mlua::Error::from(unresolvable(&program)))?
   };
   ```
   `crates/airsl/src/modules/proc.rs:109` itself — `std::process::Command::new(&program)` — is
   **not edited**. On unix the `#[cfg]` item does not exist and the line binds the same `program` it
   binds today.
3. Write the comment explaining why the `#[cfg]` is the fix rather than a carve-out. Pre-resolving
   on unix would change three behaviours, none of which buys anything there:
   - an unset `PATH` would stop falling back to `_CS_PATH`, which `execvp` does and `which` does
     not (`which` returns `None` the moment `env::overlay().get("PATH")` is `None`,
     `proc.rs:163`)
   - a `PATH` entry carrying *some* execute bit but none for the calling user would be selected by
     `mode() & 0o111 != 0` rather than skipped by the kernel in favour of a later entry
   - a missing program's error would move from spawn-time `Error::Io` (`proc.rs:120-124`) to
     pre-resolution, changing when a script sees it
4. Record the one consequence this shadowing does have on Windows, rather than letting it be
   discovered: `Error::Io`'s `path` field at `proc.rs:122` now names the **resolved absolute path**
   rather than the name as written. That is an improvement in a case that is close to unreachable —
   pre-resolution has already proved the file exists, so a spawn-time `NotFound` on Windows means
   the file was removed between the two calls — and it is stated here so nobody later reads it as
   an accident.
5. Verify:
   ```
   $ cargo make dod-crate airsl
   ```
   Expected: green on macOS with zero warnings. The `#[cfg(windows)]` block is unparsed here, so
   the first real check of this task is the CI Windows leg — say so in the execution log rather
   than reporting a local green as coverage.

### Task 4 — Refuse an unresolvable program, and name a `.cmd` shim when there is one

**Files:**
- Modify `crates/airsl/src/modules/proc.rs`

**Steps:**

1. Write the failing tests first, both Windows-only:
   - under a `where` grant, `run{'airsl-not-installed'}` fails with a message naming the program
     and `PATH`, matching the shape unix already produces for a missing program
   - with a `<temp>\npm.cmd` created and prepended to `PATH` through `env.set`, a granted
     `run{'npm'}` fails with a message that names `npm.cmd` *and* says `airsl` runs only `.exe`.
     This is the test that proves the probe fires for the case that motivates it — a Node shim —
     rather than only for a file literally named `npm`
2. Add a `#[cfg(windows)] fn unresolvable(program: &str) -> Error` returning
   `Error::Io { operation: "run", path: program.to_owned(), source }` with `source` a
   `std::io::Error::new(std::io::ErrorKind::NotFound, detail)`. **No new `Error` variant** (spec
   §8): unix already reports a missing program as `Error::Io`, and the two platforms reporting the
   same shape is worth more than a variant that names the platform.
3. Build `detail` from a `.bat`/`.cmd` probe over `PATH`, and comment the probe at the point it
   happens with the distinction premise 1.2 actually draws: these extensions are consulted **for
   the diagnostic only**, never for resolution and never for spawning. The prohibition is on
   walking `%PATHEXT%` to decide *what to run*; naming what was found is a different act. Spell out
   the consequence of omitting it — the message could then only fire for a file literally named
   `npm`, which is not the case anyone hits.
4. The refusal is actionable in the sense `crates/airsl/src/modules/guard.rs:95-104` sets: it names
   what was looked for (`npm.exe`), what was found instead (`npm.cmd`), and why the second is not
   run (spawning a batch file routes argv through `cmd.exe`, whose parsing rules differ from the
   MSVCRT rules `std` quotes with). A reader must not have to consult the spec to learn why their
   `npm` call failed.
5. Keep the probe list a two-element `const` local to that function, so a future reader cannot
   mistake it for a resolution table.
6. Verify:
   ```
   $ cargo make dod-crate airsl
   ```
   Expected: green on macOS; the whole task is `#[cfg(windows)]` and is exercised on the CI Windows
   leg only.

### Task 5 — Pin that one grant spelling works on both platforms

**Files:**
- Modify `crates/airsl/src/modules/proc.rs`
- Modify `crates/airsl/src/sandbox/grants.rs`

**Steps:**

1. `crates/airsl/src/sandbox/grants.rs:193-235` — `ProcGrant`, its `allow`, its `allows` at
   `:219-223`, and its `executables` — is **unchanged**. Nothing in this task edits it. The grant
   stays an exact, case-sensitive comparison on the program name as written.
2. Add a test to `grants.rs`'s `mod tests`, next to
   `a_proc_grant_does_not_admit_a_path_to_a_granted_name` (`grants.rs:387-394`), pinning that the
   `.exe` suffix is **not** part of the grant vocabulary: `ProcGrant::none().allow(["git"])` allows
   `git` and does **not** allow `git.exe` or `GIT`. It runs on every platform, because it is a
   statement about the grant type rather than about the filesystem. Its comment records why the
   suffix is absent: it lives in `which`'s candidate list, so one script plus one grant works on
   both platforms unchanged, and the case-sensitivity is the narrowest available comparison and
   therefore fails closed.
3. Add the end-to-end twin in `proc.rs`'s `mod tests` — the property this whole plan exists to
   keep. Name it for the behaviour, e.g.
   `a_grant_spelled_without_exe_permits_running_the_program_on_both_platforms`, and give it one
   body per platform with the *same grant string shape*: a grant of `where` permits
   `run{'where', 'where'}` on Windows; a grant of `echo` permits `run{'echo', 'ok'}` on unix.
   Neither grant names an extension. Assert the successful status on both.
4. Verify:
   ```
   $ cargo test -p airsl sandbox::grants && cargo test -p airsl modules::proc
   ```
   Expected: green on macOS. The `grants.rs` assertion is genuinely cross-platform and needs no
   Windows runner; only the `proc.rs` twin's Windows body does.

### Task 6 — Correct the exit-status comment and document what `status` means

**Files:**
- Modify `crates/airsl/src/modules/proc.rs`

**Steps:**

1. The comment at `crates/airsl/src/modules/proc.rs:129-130` — "A signalled process has no exit
   code. Reporting -1 keeps the field a number…" — describes a state that **cannot occur on
   Windows**, where `ExitStatus::code()` always returns `Some`. Rewrite it to say which platform
   each half applies to: `code()` is `None` only where a signal can end a process, which is unix;
   on Windows `unwrap_or(-1)` at `:131` never fires, and `-1` there is a *legitimate* exit code
   (`ExitProcess(0xFFFFFFFF)` narrowed to `i32`) rather than this fallback's sentinel.
2. **No new field** and no change to the expression at `:131`. A `signalled` flag would make the
   portable question — `result.status ~= 0` — stop being the one way to ask whether the program
   worked, in order to disambiguate a value that only unix can produce and only for a signal.
3. Extend the module doc at `crates/airsl/src/modules/proc.rs:1-11` with a short paragraph stating
   what `status` is on each platform: the exit code on unix with `-1` standing in for a signal, and
   the raw 32-bit exit code narrowed to `i32` on Windows. It belongs in the module doc because it
   is a property of the module's contract, not of one function's implementation.
4. Do **not** edit `crates/airsl/docs/stdlib.md:93` or
   `crates/airsl/docs/sandbox.md:227-230` here. Both are spec §11's list and belong to the
   documentation plan; changing them from two plans is how one of the two edits gets lost.
5. Verify:
   ```
   $ RUSTDOCFLAGS="-D warnings" cargo doc -p airsl --all-features --no-deps
   ```
   Expected: green with zero rustdoc warnings; the new module-doc paragraph renders.

### Task 7 — Give the platform-dependent tests platform-conditional bodies

**Files:**
- Modify `crates/airsl/src/modules/proc.rs`

`proc.rs` has **13** `#[test]` functions, not 14 — counted with
`grep -c '#\[test\]' crates/airsl/src/modules/proc.rs`. Each was read; this is the full triage.

| # | Test | Line | Verdict |
|---|---|---|---|
| 1 | `the_module_is_named_proc` | `:202-205` | **unchanged** — no fixture at all |
| 2 | `run_captures_stdout_and_the_exit_status` | `:207-217` | **split** — `echo` is a `cmd` builtin, not an `.exe` |
| 3 | `run_captures_a_non_zero_status_without_raising` | `:219-225` | **split** — no `false.exe` |
| 4 | `run_captures_stderr_separately` | `:227-236` | **split** — spawns `sh` |
| 5 | `arguments_are_passed_without_a_shell` | `:238-249` | **split** — §9 names this one specifically |
| 6 | `an_ungranted_program_is_refused` | `:251-256` | **unchanged** — refused before any resolution; `curl` is never spawned |
| 7 | `a_path_to_a_granted_program_is_still_refused` | `:258-266` | **unchanged** — `/bin/echo` is a string the grant refuses; nothing touches the filesystem |
| 8 | `an_empty_argv_is_refused_with_a_reason` | `:268-273` | **unchanged** — `argv` refuses before `Command` exists |
| 9 | `which_finds_a_granted_program_on_the_path` | `:275-280` | **split** — asserts `ends_with("/sh")` (Task 2) |
| 10 | `which_returns_nil_for_something_that_is_not_installed` | `:282-291` | **unchanged** — and on Windows now also proves the `.exe` candidate missed |
| 11 | `which_is_refused_for_an_ungranted_program` | `:293-299` | **unchanged** — the grant check at `:141` precedes `which` |
| 12 | `a_policy_granting_nothing_refuses_every_program` | `:301-309` | **unchanged** — names `echo` but never spawns it |
| 13 | `a_trusted_policy_runs_anything` | `:311-317` | **split** — spawns `echo` |

Five split, eight unchanged. Tests 6, 7 and 12 are worth noting explicitly: they *name* unix
programs and are still portable, because the refusal they assert happens strictly before
resolution. Rewriting them for Windows would add platform branching that proves nothing.

**Steps:**

1. Take the five splits with `#[cfg(unix)]` / `#[cfg(windows)]` bodies inside one test function
   each, rather than two functions with platform-suffixed names — the behaviour under test is the
   same sentence on both platforms, and duplicating the name would suggest otherwise. The unix
   bodies are **byte-identical to what is there today**; this task must not tighten or relax a unix
   assertion.
2. Windows fixtures use `where.exe`, per spec §9: a real executable on every installation with no
   shell involved.
   - #2: `run{'where', 'where'}` — status `0`, stdout non-empty and lowercasing to something
     containing `where.exe`
   - #3: `run{'where', 'airsl-nonexistent-xyz'}` — status non-zero, and the call **returns a
     result rather than raising**, which is the property the test is named for
   - #4: the same call's `stderr` is non-empty while its `stdout` is empty. Assert non-emptiness,
     not the message text: `where`'s `INFO:` line is localised, and pinning English would make the
     suite fail on a non-English runner for a reason unrelated to the property
   - #13: `run{'where', 'where'}` under `Policy::trusted()`, status `0`
3. #5, `arguments_are_passed_without_a_shell`, needs a program that echoes an argument back, which
   `where` does not. Use `find.exe` (`C:\Windows\System32\find.exe`, present on every
   installation): write a temp file whose single line is the payload, then
   `run{'find', '<payload>', '<file>'}` and assert the payload appears in stdout. The payload is
   `a & b | c ^ d %FOO%`, covering all four metacharacters §9 names — if any shell were in the
   path, `&` would terminate the command, `|` would open a pipe, `^` would escape the next
   character and `%FOO%` would expand, and the literal line would not be found.
   - Interpolating the temp path into the Lua source is spec §9's **Class 1** trap: a
     `windows-latest` temp path contains `\U`, which is an invalid escape in a Lua single-quoted
     string exactly as it is in TOML. Convert the path to `/` before interpolating — premise 1.1
     accepts `/` on input — and put that reason in a comment, since the failure it prevents is a
     parse error whose message names Lua rather than the path
   - If `find.exe` turns out to parse its own command line rather than taking MSVCRT argv, fall
     back to `where /R <dir> <name>` over a file *named* with the payload, dropping `|` from it —
     `|` is illegal in an NTFS filename. Record the fallback's reduced coverage in the test comment
     if it is used; do not silently drop a metacharacter
4. Verify:
   ```
   $ cargo test -p airsl modules::proc
   ```
   Expected: 13 tests plus the ones added in Tasks 1–5, all green on macOS, with every unix
   assertion identical to before this plan. A diff of the unix bodies against `HEAD` should show
   only the added `#[cfg(unix)]` attributes.

### Task 8 — Run the gate

**Steps:**

1. Confirm the workspace has no remaining unconditional unix-only `std` use in production code:
   ```
   $ grep -rn "std::os::unix" crates --include="*.rs"
   ```
   Expected: every hit is inside a `#[cfg(test)] mod tests` (the symlink-helper plan) or under a
   `#[cfg(unix)]` item. `crates/airsl/src/modules/proc.rs:172` no longer appears as an
   unconditional use.
2. Verify:
   ```
   $ cargo make dod
   ```
   Expected: green on macOS — zero warnings from clippy, rustdoc, tests and doctests.

---

## Verification summary (plan-level)

- `cargo make dod` green on macOS and, via the CI legs, on Linux and Windows.
- Unix behaviour is provably unchanged: no line of the unix `run` path is edited,
  `crates/airsl/src/modules/proc.rs:109` still reads `std::process::Command::new(&program)`, and
  every unix test body is byte-identical apart from an added `#[cfg(unix)]`.
- `crates/airsl/src/sandbox/grants.rs`'s `ProcGrant` is unchanged; the only edit to that file is an
  added test.
- A grant of `git` permits `run{'git'}` on both platforms, pinned by a test on each — the property
  that keeps one script plus one grant portable.
- No `.bat` or `.cmd` is ever spawned or resolved. The only code that names those extensions is the
  §8 diagnostic, and it is commented as such at the point it runs.
- No new `Error` variant, no new `run` result field.
- `proc.rs:129-130`'s comment no longer describes a state Windows cannot reach.
