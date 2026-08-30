---
status: done
created: 2026-08-29
---

# Intent: Windows as a first-class build target

## Problem

`airsl` refuses to build off unix. `crates/airsl/src/lib.rs:39-43` carries a
`compile_error!` naming two reasons: `airsstack.proc` decides executability from unix mode
bits, and the sandbox tests build symlinks through `std::os::unix::fs`.

That gate was never the product of a portability audit. It came from commit `5c3e8f0`, a
fix inside the repository-extraction work: a three-OS CI matrix failed on its first run
with three resolution errors, and rather than make a behavioural decision inside an
extraction commit, the matrix was cut to `ubuntu-latest` and `macos-latest` and the
constraint was written down. That was the right call at the time. It is not a finding
about what supporting Windows would actually cost.

The cost of the gate is reach. `airsl` is on crates.io at 0.1.3, published as an
embeddable runtime for host programs. Every Windows embedder is turned away at compile
time. Changing a platform contract is cheap before 1.0 and expensive after, once hosts
have built against the current one.

An audit of the crate against Windows semantics found that the compiler-visible surface
is the *smallest* part of the problem, and that the gate's two stated reasons are the two
cheapest items on the list. A compiler reports resolution failures; it has nothing to say
about a containment predicate that compiles cleanly and returns the wrong answer. Four
questions about what this crate's contracts *mean* on Windows are currently unanswered:

1. **What is an executable?** `proc.rs:171-175` reads mode bits. Windows decides by
   extension. Compounding it, `std`'s own spawn path resolves only `.exe` and never
   consults `PATHEXT` (`sys/process/windows.rs:464-511`), so `which` and `run` can be
   made to disagree about which programs exist.
2. **What contains a path?** `PathGuard::resolve` (`guard.rs:145-191`) is the single place
   the containment rule is written. On Windows it has a hole: `std::path::absolute` returns
   verbatim (`\\?\`) paths unchanged (`sys/path/windows.rs:194-203`), and `/` is not a
   separator inside them (`is_verbatim_sep`, `:56-58`) — so `..` hides inside a single
   `Component::Normal`, the `ParentDir` arm the guard's own doc comment calls "exactly the
   bypass this function exists to prevent" never fires, and the guard returns *approved*
   for a path that leaves the root. `fs` operations still fail (the kernel rejects `/` in a
   verbatim name), but `fs.canonicalize` hands the approved string back to Lua
   (`fs.rs:237-243`), where a script holding a `proc` grant can pass it to a program that
   does re-normalise separators. Separately, a grant on a root that does not yet exist
   matches nothing at all, because `resolve_root`'s fallback (`grants.rs:121-125`) yields a
   `Disk` prefix while checked paths canonicalise to `VerbatimDisk`, and `Prefix` derives
   `PartialEq` (`std`'s `path.rs:135`).
3. **What is a name's identity?** Windows environment variables and filenames are
   case-insensitive; four grant surfaces compare them case-sensitively — `EnvGrant::allows`
   and `ProcGrant::allows` (`grants.rs:167-170`, `:218-221`), `contains_any`
   (`grants.rs:132-134`), and `fs.create_exclusive`'s uniqueness property
   (`fs.rs:152-179`). The sharpest consequence is a split-brain `PATH`: `Overlay::get`
   checks a case-sensitive map and then falls through to the case-insensitive
   `std::env::var` (`env.rs:54-55`), so `which` and `run` can resolve against different
   `PATH`s.
4. **What does a script see?** Determinism is a stated correctness property of this crate,
   not a nicety. Three mechanisms break it on Windows: separators and `\\?\` prefixes leak
   into every path-shaped string returned to Lua (`fs.rs:409`, `glob.rs:118`, `fs.rs:240`,
   and `path.rs:191-197`, which hardcodes `/` and concatenates it with backslash-separated
   output); `globset`'s `backslash_escape` defaults to *false* on Windows and `glob.rs:62-72`
   never overrides it, so a pattern meaning "literal asterisk" on unix becomes a wildcard —
   widening, the one direction `glob.rs:53-56` says a matcher must never be wrong in; and
   `Command`'s environment ordering runs through `CompareStringOrdinal`, which `std`
   describes as a mapping unique to Windows that "can potentially change between Windows
   versions" (`sys/process/windows.rs:57-64`) — the same class of hazard `os.setlocale` is
   withheld below `trusted` to avoid.

None of these four are visible to a compiler. All four are decisions about what a
capability means on a second platform, which is why the original commit was right that
this is not a portability patch.

One thing is unverified and gates every estimate: because the `compile_error!` fires
first, **no one has ever observed whether this workspace's vendored Lua compiles on
Windows at all.**

## Affected systems

Library (`crates/airsl`), in rough order of decision weight:

- `src/modules/guard.rs` — the single containment rule; the verbatim hole.
- `src/sandbox/grants.rs` — `resolve_root`, `contains_any`, `EnvGrant`, `ProcGrant`.
- `src/modules/proc.rs` — `is_executable`, `which`; the only non-test `std::os::unix` use
  in the workspace.
- `src/modules/env.rs` — overlay case identity, `all()` key casing.
- `src/modules/path.rs` — `normalize`'s hardcoded separator; all 27 tests assume `/`.
- `src/modules/glob.rs`, `src/modules/fs.rs` — returned-path vocabulary; `atomic_write`'s
  guarantee under `NamedTempFile::persist`; `create_exclusive`.
- `src/require_loader.rs` — a second, independent copy of the containment rule
  (`:203-230`).
- `src/lib.rs` — the gate itself.
- Eight test symlink sites across `guard.rs`, `grants.rs`, `require_loader.rs`,
  `negotiate.rs`, `manifest.rs`, `loaded.rs` — three are directory links, five are file
  links needing privileges CI does not grant by default.

Workspace surface:

- `crates/airsl-cli` — path-separator assertions (`check.rs:199`, `test_runner.rs:247`) and
  string-escaping bugs where temp paths are interpolated into TOML basic strings and Lua
  single-quoted strings (`ext_fire.rs:318-326`, `check.rs:160`).
- `Makefile.toml` — `examples`, `publish-dry-run`, `dod-crate` are POSIX shell
  (`:111`, `:166`, `:207`); the five gate tasks are already portable.
- `.github/workflows/ci.yml:37` — the matrix, and the comment at `:31-33` explaining why
  Windows was removed.
- `crates/airsl/docs/`, three `README.md`s, `CLAUDE.md` — the unix-only promise is stated
  in each as a documented guarantee.
- `crates/airsl/examples/` — `env-and-proc` and `denials-are-data` spawn `sh`; the examples
  README promises byte-for-byte reproducible output.

Not affected: every dependency. All thirteen are cross-platform, and `windows-sys`,
`windows-link` and `winapi-util` are already in the lockfile.

## Desired outcome

`x86_64-pc-windows-msvc` is a supported target at the same tier as Linux and macOS.

- `cargo make dod` runs green on `windows-latest` as a third matrix leg, under the same
  warnings-as-errors gate.
- Every test passes, ported rather than skipped. In particular the symlink-containment
  tests — the ones that establish the property this crate exists to provide — actually run
  on Windows. A Windows CI leg that skips them proves nothing on the platform where
  containment is hardest to get right.
- Every module works, including `proc` and `fs`. No module is degraded or absent on
  Windows.
- Each of the four contract questions above has a written, tested answer, and where a
  behaviour legitimately differs between platforms, the difference is documented at the
  same evidence standard the rest of `docs/` holds — a `file:line` for a claim about code
  that exists.
- The `compile_error!` and every documented unix-only promise are removed or restated.

## Constraints

- **`cargo make dod` is the gate.** Anything less is iteration. `-D warnings` promotes
  plain rustc warnings, and `unsafe_code = forbid` holds — no `unsafe` for Windows APIs.
- **The three-layer separation stands.** Language surface, grants, and resource limits stay
  three questions, not one switch. A Windows difference must not collapse them.
- **Enforcement stays in Rust, never in Lua**, and a module stays present-but-refusing when
  ungranted rather than absent — `if airsstack.fs then` must keep meaning "does this runtime
  have it", not "am I allowed", on every platform.
- **Determinism is a correctness property, not a preference.** Whatever a script observes
  must not vary with host casing, path spelling, or an OS collation table.
- **A change that makes a module need a grant it did not need before is a design decision**,
  not a detail — this applies equally to a change that makes a grant mean something
  different on one platform.
- Containment behaviour must be *tested* on Windows, not asserted. This follows from the
  parity bar and is the constraint most likely to be quietly traded away.
- Build requirements: `mlua`'s `vendored` feature compiles Lua 5.4 from C, so a C compiler
  remains required — on this target, MSVC Build Tools. `windows-latest` ships them.
- Unix behaviour must not regress. Where a single fix serves both platforms it is
  preferred, but Linux and macOS keep their current semantics.
- Verify the vendored Lua build on Windows before committing to any estimate — every
  figure downstream assumes it succeeds, and that assumption has never been tested.

## Non-goals

- Any target other than `x86_64-pc-windows-msvc`. `x86_64-pc-windows-gnu` (MinGW/MSYS2),
  `aarch64-pc-windows-msvc`, `i686-*`, and UWP are explicitly out.
- WSL. It is already covered as a Linux target and is not Windows support.
- Adding a shell, `cmd.exe`, or PowerShell surface to `airsstack.proc`. "There is no string
  form of `run`" is a property to preserve on Windows, not to relax.
- Windows-specific capability modules — ACLs, the registry, services, event log, drive
  enumeration. This chain ports the existing twelve modules; it adds none.
- Changing unix semantics for their own sake. Cross-platform reconciliation is in scope;
  unix-side redesign is not.
- Publishing a release. Getting the target green is this chain; cutting a version that
  advertises it is separate.
