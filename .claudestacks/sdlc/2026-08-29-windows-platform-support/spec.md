---
status: approved
created: 2026-08-29
---

# Spec: Windows as a first-class build target

Removes the `compile_error!` at `crates/airsl/src/lib.rs:39-43` and makes
`x86_64-pc-windows-msvc` a supported target at the same tier as Linux and macOS. The work is
not a portability patch: it answers four questions about what this crate's capability
contracts *mean* on a second platform — what an executable is, what contains a path, what a
name's identity is, and what a script sees — and it closes one containment hole that only
exists on Windows. Every behavioural difference that survives is documented and tested rather
than discovered.

## 1. Design premises

These are the decisions the rest of the spec follows from. Each is a choice among real
alternatives, recorded with the reason it was taken.

**1.1 Paths a script sees are normalised; paths a script writes are accepted in either form.**
`airsl` returns `/`-separated paths with no `\\?\` prefix on every platform, and accepts `/`
or `\` on input. The alternative — returning native separators — makes an identical script
produce different bytes per platform, which contradicts the determinism convention recorded in
`CLAUDE.md:106-108` and breaks the byte-for-byte promise at
`crates/airsl/examples/README.md:73`.

**1.2 An executable on Windows is a `.exe`, and `airsl` resolves it itself.** `which` appends
`.exe` to a bare program name and matches only that; `%PATHEXT%` is not walked; `.bat` and
`.cmd` are never spawned. This preserves the module's headline property — "there is no shell,
no word splitting and no quoting bug to have" (`crates/airsl/src/modules/proc.rs:3-6`) —
because spawning a batch file routes argv through `cmd.exe`, whose parsing rules differ from
the MSVCRT rules `std` quotes with (the CVE-2024-24576 class). The accepted cost is real and
should not be understated: `npm`, `npx`, `yarn`, `tsc` and most Node/SDK shims ship as `.cmd`
on Windows and are therefore not reachable from `airsstack.proc`.

**1.3 Environment variable names fold on Windows; filesystem paths never do.** The Windows
environment block is case-insensitive unconditionally, so folding names there is sound. NTFS
is *not* unconditionally case-insensitive — per-directory case sensitivity has existed since
Windows 10 1803 and WSL sets it — so `airsl` never folds a path itself. §3.6 records where
this is and is not observable, which is narrower than it first appears.

**1.4 Internally, no path is ever verbatim.** `canonicalize()` returns `\\?\C:\…` on Windows;
it is stripped immediately on the way in. Verbatim spellings are refused on input. This costs
no long-path support: `std` re-adds the prefix internally when opening (`maybe_verbatim`,
`library/std/src/sys/path/windows.rs:86-89`), and the doc comment on the function it delegates
to records that non-verbatim paths are preferable for anything "given back to users or passed
to other application" (`get_long_path`, `:91-98`, quoted phrase at `:95`).

**1.5 Platform difference is structured and flavour-parameterised, not scattered.** Lexical
platform rules live in one module as pure functions with no filesystem access. Each takes an
explicit `PathFlavor` (`Posix` or `Windows`) rather than being `#[cfg]`-gated internally, with
thin public wrappers that select the compile-time flavour. This is what makes the Windows rules
unit-testable on Linux and macOS — a `#[cfg]`-gated rule can only ever be tested on the host it
was compiled for, which would leave the majority of this spec's new logic unexercised until CI
reached a Windows runner. The alternative of `#[cfg(windows)]` branches at each affected site
was rejected: it smears platform logic across eight files and makes every future path-returning
function a place to forget the conversion.

## 2. Module layout

One new folder module at the crate root, reachable from `modules::guard`, `sandbox::grants` and
`require_loader`:

```
crates/airsl/src/paths/
├── mod.rs          # table of contents and module doc only
├── rules.rs        # pure lexical rules, flavour-parameterised; no filesystem access
├── resolved.rs     # ResolvedPath: the outward `/` vocabulary, made structural
└── containment.rs  # the shared root-comparison predicate
```

`mod.rs` carries module docs plus `mod`/`pub(crate) use` and no implementation, per the
`mod-rs-export-only` rule. Each file ships a colocated `#[cfg(test)] mod tests`.

**What does not move.** `PathGuard` *and* `PathGuard::resolve` stay in
`crates/airsl/src/modules/guard.rs`. An earlier draft relocated `resolve` wholesale; that was
rejected as exceeding what the reasons demand, inside a platform port, on the crate's most
safety-critical function. Three things genuinely need a shared home:

1. **The lexical rules**, because premise 1.5 requires them filesystem-free and
   flavour-parameterised.
2. **The root-comparison predicate**, because containment is implemented **four** times today
   (§3.5), not once.
3. **The outward vocabulary**, because the boundaries that hand a path to Lua are too many to
   enumerate reliably (§4).

`require_loader`, `manifest` and `loaded` need only `reject_unrepresentable`, verbatim-stripping
and the comparison — not the partial-canonicalisation algorithm, since their targets must already
exist. So `resolve` has exactly one caller and stays where it is.

**`ResolvedPath` (`paths/resolved.rs`).** A newtype over the `PathBuf` that `PathGuard::resolve`
returns. Its invariant is exactly what its constructor establishes and no more: *absolute,
non-verbatim, symlink-resolved down to the deepest existing ancestor, and free of any `..` below
that point*. It deliberately does **not** mean "inside a granted root" — that comparison is
`paths::containment`'s, applied afterwards by `PathGuard`.

An earlier draft called this type `ContainedPath`, which asserted an invariant its constructor did
not carry; a later draft deleted it altogether, which removed the only structural guarantee that a
path reaching Lua carries the `/` vocabulary and forced §4 to enumerate every boundary by hand.
The type is reinstated with the honest name. Its accessors are:

```rust
/// The path to hand to the operating system.
pub(crate) fn as_path(&self) -> &Path;
/// The path to hand to a script: `/`-separated, never verbatim.
pub(crate) fn to_script_string(&self) -> String;
```

There is no `Display`, no `AsRef<str>`, and no public inner field, so a path cannot reach Lua or a
message without a caller naming which form it wanted.

`paths::rules` exposes, at minimum:

```rust
/// Which platform's spelling rules apply. Explicit so both are testable on either host.
pub(crate) enum PathFlavor { Posix, Windows }

/// Why a raw path string cannot be reasoned about at all.
pub(crate) enum Unrepresentable { Verbatim, DeviceNamespace, InteriorNul }

/// Refuses path spellings this runtime will not reason about.
pub(crate) fn reject_unrepresentable(raw: &str, flavor: PathFlavor) -> Result<(), Unrepresentable>;

/// The outward vocabulary: `/`-separated, never verbatim.
pub(crate) fn to_script_string(path: &Path, flavor: PathFlavor) -> String;

/// Removes a `\\?\` / `\\?\UNC\` prefix from a `canonicalize()` result (premise 1.4).
/// Every `canonicalize()` call site in the crate must apply it (§3.5), not only
/// `PathGuard::resolve`.
pub(crate) fn strip_verbatim(path: PathBuf, flavor: PathFlavor) -> PathBuf;

/// The file names to try on `PATH` for a bare program name, in order.
pub(crate) fn executable_candidates(program: &str, flavor: PathFlavor) -> Vec<String>;

/// Whether `program` names a path rather than a bare name.
pub(crate) fn has_separator(program: &str, flavor: PathFlavor) -> bool;
```

Each of these is the **flavour-taking core**, which is what the tests drive with both flavours.
Alongside each, `paths::rules` exposes a same-named one-argument wrapper in a `native` submodule
that supplies the compile-time flavour:

```rust
pub(crate) mod native {
    pub(crate) const FLAVOR: PathFlavor =
        if cfg!(windows) { PathFlavor::Windows } else { PathFlavor::Posix };

    pub(crate) fn has_separator(program: &str) -> bool {
        super::has_separator(program, FLAVOR)
    }
    // ...one per rule
}
```

Callers outside `paths` use `paths::rules::native::*`; only the tests name a flavour explicitly.

Every one of these is asymmetric by platform, and each asymmetry is load-bearing rather than
incidental:

| Function | `Posix` | `Windows` |
|---|---|---|
| `to_script_string` | unchanged | `\` → `/`, verbatim prefix stripped |
| `reject_unrepresentable` | interior NUL only | interior NUL, `\\?\`/`\\.\`, reserved device names |
| `executable_candidates` | `[program]` | `[program]` if it already ends `.exe` (case-insensitively), else `[program.exe]` |
| `has_separator` | `/` | `/` or `\` |

`to_script_string` converts `\` to `/` on Windows only because a unix filename may legally
contain a backslash — converting there would corrupt real names — while a Windows filename may
not contain `/`, so the conversion is unambiguous and reversible.

`reject_unrepresentable` is likewise Windows-only in substance. On Linux and macOS a file named
`NUL` or `con` is an ordinary file, so refusing it there would regress unix behaviour, which the
intent forbids. Under `Posix` the function rejects only an interior NUL byte, which no unix
syscall can express either. `InteriorNul` is therefore the one variant common to both flavours.

## 3. Path containment

### 3.1 Resolution

The existing algorithm at `crates/airsl/src/modules/guard.rs:145-191` is preserved: make the
path absolute, canonicalise the deepest ancestor that **exists**, re-append the non-existent
tail, and refuse a `..` below that point rather than resolving it lexically. Three changes:

1. **`reject_unrepresentable` runs first**, before `std::path::absolute`. This closes the
   verbatim hole described in §3.2. Because `resolve` is the single entry point every `fs`
   call funnels through, downstream code never re-asks — the parse-don't-validate property is
   carried by the entry point rather than by a wrapper type. Reserved device names (`CON`,
   `NUL`, `\\.\…`) are refused here too **on Windows only**; today they fall into the `_ =>`
   arm at `guard.rs:176-181` and produce the misleading message "no part of it exists".
2. **The verbatim prefix is stripped** from `canonicalize()` output, so nothing verbatim is
   ever stored or compared.
3. **The root comparison is delegated** to `paths::containment`, shared with `require_loader`.

### 3.2 The hole this closes

On Windows `std::path::absolute` returns verbatim paths **unchanged**
(`library/std/src/sys/path/windows.rs:194-203`), and `/` is not a separator inside a verbatim
path (`is_verbatim_sep`, `:56-58`). Consequently a path such as

```
\\?\C:\granted\a/../../Windows\System32\x
```

parses with `a/../../Windows` as a single `Component::Normal`. The `ParentDir` arm at
`guard.rs:168-173` — which that function's own doc comment calls "exactly the bypass this
function exists to prevent" — never fires. The resolution loop pops to `\\?\C:\granted`,
canonicalises it, re-appends the suffix, and `starts_with(root)` returns **true**: the guard
approves a path that leaves the root.

Filesystem operations still fail, because the kernel rejects a literal `/` in a verbatim name,
so `fs` is fail-closed today. The reachable consequence is that `fs.canonicalize` returns the
approved string to Lua (`crates/airsl/src/modules/fs.rs:237-243`), where a script also holding
a `proc` grant can hand it to a program that does re-normalise separators. Rejecting verbatim
input at the door removes the class rather than the instance.

### 3.3 Root resolution

`resolve_root` at `crates/airsl/src/sandbox/grants.rs:121-125` falls back to
`std::path::absolute` when `canonicalize` fails — the ordinary case for a write root that does
not exist yet. On Windows that yields a `Disk('C')` prefix while checked paths canonicalise to
`VerbatimDisk('C')`, and `Prefix` derives `PartialEq` (`library/std/src/path.rs:135`), so the
two never compare equal and the grant silently matches nothing. Premise 1.4 fixes this without
special-casing: with the verbatim prefix stripped on the way in, both sides are `Disk('C')`.

### 3.4 A platform difference that is accepted

On Windows `std::path::absolute` is `GetFullPathNameW`
(`library/std/src/sys/path/windows.rs:190-215`), which collapses `..` lexically before the
guard sees it, so `resolve`'s `ParentDir` arm is reachable on unix only. This resembles the
unsound lexical collapse the guard's doc comment warns against, but it is not: Win32 normalises
`..` out of the path string before the object manager resolves it, so the check and the
subsequent open agree. Soundness holds on both platforms by *different* arguments.

The observable consequence is real and cannot be engineered away: with `link` a symlink or
junction to `elsewhere`, `granted/link/../secret` opens `elsewhere/secret` on unix and
`granted\secret` on Windows. It is documented as a platform-conditional invariant and pinned by
a test asserting the guard's verdict matches what the operating system actually opens.

### 3.5 Removing the duplicate rule

Containment is implemented four times in this crate, and a fifth site canonicalises a root
without comparing it. All five must apply `strip_verbatim`; the four that compare must share one
predicate:

| Site | Shape | Returns a `canonicalize()` output? |
|---|---|---|
| `crates/airsl/src/modules/guard.rs:145-191` | partial canonicalisation, then compare | yes, as `ResolvedPath` |
| `crates/airsl/src/require_loader.rs:202-230` | `canonicalize` + `starts_with` | **yes** (`:223`) |
| `crates/airsl/src/extension/manifest.rs:280-291` (`validate_entry`) | `canonicalize` + `starts_with` | no (returns `relative`) |
| `crates/airsl/src/extension/loaded.rs:140-152` (`recheck_entry`) | `canonicalize` + `starts_with` | **yes** (`:152`) |
| `crates/airsl/src/sandbox/grants.rs:121-125` (`resolve_root`) | `canonicalize`, no comparison | **yes** |

That is a duplicate-concept violation of the modularity rule today and, once Windows is
supported, four places to fix and three to forget. The three simple copies delegate their
comparison to `paths::containment` while keeping their own distinct responsibilities —
`require_loader::resolve` iterating `target.candidates()`, `validate_entry` rejecting `..`/`.`
components and checking `is_file`, `recheck_entry` re-verifying at load time.

**Three sites return a `canonicalize()` output, and each violates premise 1.4 without
stripping.** `recheck_entry` returns the extension's entry path; `require_loader::resolve`
returns a path that becomes both the module-cache key (`require_loader.rs:147`) and the chunk
name a Lua traceback shows (`:197`); `resolve_root` returns the grant root that §3.3's fix
depends on comparing as `Disk('C')` rather than `VerbatimDisk('C')`. This is why `strip_verbatim`
lives in `paths::rules` rather than inside `PathGuard::resolve`.

### 3.6 Where case actually matters

Premise 1.3 says `airsl` never folds a path. The scope of that decision is narrower than it
sounds, and the reason matters for §9's tests.

For paths that **exist**, Windows `fs::canonicalize` returns the on-disk casing. Both the root
and the checked path go through it (`guard.rs:145-191`, `grants.rs:121-125`), so both come back
in the same casing and the comparison already behaves case-insensitively — the fold happens in
the operating system, not in `airsl`.

Case therefore only varies where canonicalisation does not reach: a root that does not exist yet
and falls back to `std::path::absolute` (§3.3), and the non-existent tail re-appended after the
deepest existing ancestor. In exactly those cases the comparison is a byte comparison, which is
*narrower* than a case-insensitive volume and so fails closed. Folding there instead would be
wider than a case-sensitive directory allows — an escape — which is why the decision is to fold
nothing.

## 4. Path vocabulary and determinism

**The rule.** No path reaches a script, a Lua traceback, or an error message in native spelling.
Everything crossing that boundary is `/`-separated and never verbatim.

**The compiler narrows the search; it does not close it.** Three drafts tried to enumerate the
boundaries by reading; the first missed six and the second missed four more. The type system does
better, but its guarantee has to be stated precisely or it misleads a plan author:

1. `PathGuard::read`/`write` return `ResolvedPath` (§2), which has **no `Display`, no
   `AsRef<str>`, no `to_string_lossy`, and no public inner field**. Its only string accessor is
   `to_script_string`.
2. `rustc` therefore produces the list of the guard's **consumers** — every site that takes a
   value out of `read`/`write` — on the first build, exhaustively.
3. That is *not* the same as the list of **renderings**. A helper taking `&Path` — `fs::io`
   (`crates/airsl/src/modules/fs.rs:52-54`), `fs::walk` (`:397`), `PathGuard::deny`
   (`crates/airsl/src/modules/guard.rs:105`) — is satisfied by inserting `.as_path()` at the
   call, after which the `display()` inside compiles unchanged and stays natively spelled. And
   `as_path()` returns `&Path`, which carries both `display()` and `to_string_lossy()`, so the
   newtype does not durably prevent a future site either.

**So the mechanism is: `rustc` lists the guard's consumers, and the plans triage every rendering
downstream of each.** Inserting `.as_path()` to clear a diagnostic is explicitly *not* a fix, and
a plan that does so without converting the rendering below it has not done the work. Two helpers
are changed to take `&ResolvedPath` rather than `&Path` — `fs::io` and `PathGuard::deny` — so that
for the two highest-traffic rendering sites the diagnostic lands on the rendering itself rather
than on the handoff above it.

**The residue: paths that never pass through the guard.** The compiler cannot reach these at all.
This is the list plans must handle explicitly:

| Site | Reaches a script as |
|---|---|
| `crates/airsl/src/modules/fs.rs:409` | `fs.walk` entries, relative to the walk root |
| `crates/airsl/src/modules/glob.rs:118` | `glob.walk` entries, relative to the walk root |
| `crates/airsl/src/modules/fs.rs:355`, `:374` | `fs.tempdir`, `fs.tempfile` — rooted at `std::env::temp_dir()` |
| `crates/airsl/src/modules/proc.rs:167` | `proc.which` |
| `crates/airsl/src/modules/guard.rs:114` | the granted-root list in a refusal — `&[PathBuf]` off `FsGrant`, never guard-derived |
| `crates/airsl/src/modules/ext.rs:138-143` | `ext.granted()`'s fs roots, via `sorted_roots` |
| `crates/airsl/src/types/chunk_name.rs:60-66` | every file-loaded script's traceback name |
| `crates/airsl/src/extension/loaded.rs:117-121` | the extension chunk name |
| `crates/airsl/src/require_loader.rs:197` | the `require` chunk name |
| `crates/airsl/src/modules/path.rs` | `join`, `dirname`, `basename`, `stem`, `ext`, `normalize`, `absolute`, `relative_to` — pure string math, no guard involved |

`guard.rs:114` is worth singling out: the grant roots it renders come from `resolve_root`
(§3.5), so they are already verbatim-stripped, but they still need the `/` conversion — and
because they arrive as a plain `&[PathBuf]`, no diagnostic will ever point at them.

Two of these carry more than a spelling change. `ext.rs:138-143` sorts the *rendered strings*, and
its comment at `:133-135` records that the output is machine-read by a script and sorted for
determinism — so a backslash rendering changes ordering, not only spelling. And
`chunk_name.rs:60-66` and `require_loader.rs:197` are the two sources of Lua traceback names; if
only one is converted they diverge, and a traceback would spell the same file two ways depending
on how it was loaded.

### 4.0 `is_absolute` and `absolute`

`Path::new("/a").is_absolute()` is `false` on Windows, and `std::path::absolute("/a/b")` prepends
the current drive. The `/` vocabulary does not change this: **absoluteness is a property of the
platform's path grammar, not of the separator**, and `path.is_absolute` continues to report what
the platform believes. Claiming `/a` is absolute on Windows would make `path.absolute` incoherent
with it — `absolute("/a")` must still produce `C:/a` — and would mislead a script into treating a
drive-relative path as fully qualified.

This is the one place the outward vocabulary is uniform but the *semantics* are not, and it is
documented as such. Five tests in `crates/airsl/src/modules/path.rs` carry unix-shaped
expectations and take platform-conditional ones: `:425-434`, `:437-439`, `:442-447`, `:449-451`,
`:454-459`. Those five are the exception to the claim below.

Because the outward vocabulary is `/` on every platform, the remaining 22 of `path.rs`'s 27 tests
keep their current expectations unchanged.

### 4.1 `normalize`

`crates/airsl/src/modules/path.rs:191-197` hard-codes a forward slash — `leading.join("/")` —
and concatenates it with `out.to_string_lossy()`, which is backslash-separated on Windows,
producing mixed output such as `../..\b\c`. It is rebuilt rather than patched: assemble into a
`PathBuf` and render once through `to_script_string`, with no string concatenation.

The same rewrite fixes drive-relative input, which today produces garbage
(`normalize("C:a/../..")` yields `"../C:"`), via an explicit rule: **a `..` that would climb
above a root — `RootDir` or a Windows `Prefix` — is absorbed**, matching how every operating
system treats `/..`, while a `..` above a *relative* start still survives as a leading `..`.

**This rule changes one unix-observable output, deliberately.** Today `normalize("/..")` returns
`"..//"`, because `out.pop()` on `RootDir` returns false, the `..` lands in `leading`, and
`:197` splices the two. Under the new rule it returns `"/"`. The intent lists "changing unix
semantics for their own sake" as a non-goal; this is not that — it is a defect the Windows work
surfaces, and leaving `"..//"` in place while fixing the Windows twin would encode the bug as
intended behaviour. The affected expectations are named in the plans rather than left for a plan
author to discover.

### 4.2 Glob patterns

`globset`'s `backslash_escape` option defaults to `!is_separator('\\')`, which is **false** on
Windows, and `crates/airsl/src/modules/glob.rs:62-72` never overrides it. A pattern meaning
"literal asterisk" on unix would become a wildcard on Windows — a *widening*, which
`glob.rs:53-56` states is the one direction a matcher must never be wrong in. The option is
pinned explicitly to `true` rather than inherited from the platform. Patterns are always written
in the `/` vocabulary; `globset` normalises candidates itself.

### 4.3 What is not a determinism problem

`walkdir`'s `sort_by_file_name` is an `OsStr` byte comparison (`glob.rs:106`, `fs.rs:399`), so a
case-insensitive filesystem does not perturb sort order.

`Command`'s Windows environment map is keyed by a type whose ordering calls
`CompareStringOrdinal` (`library/std/src/sys/process/windows.rs:66-68`). `std` reaches for that
OS call precisely *because* it is the platform's own stable answer; what its comment warns may
change between Windows versions is the underlying case-folding uppercase mapping (`:57-64`).
Either way this is not a determinism exposure here: §6 makes at most one overlay entry exist per
folded name, so `Command`'s map cannot collapse two of ours, and the child's environment block
ordering is not script-observable.

## 5. Executability and subprocesses

### 5.1 One resolution path, owned by `airsl`

`crates/airsl/src/modules/proc.rs:170-175` uses `std::os::unix::fs::PermissionsExt` and
`mode() & 0o111`. It is replaced by a two-part check: the lexical half
(`paths::rules::executable_candidates`, §2) and the filesystem half — existence, and on unix the
mode bits — which stays in `proc.rs`.

`which` joins each `PATH` entry with each candidate name in order, and returns the first that
exists and is executable. On Windows `which("git")` therefore probes `git.exe`, not `git`.

**On Windows only**, `run` pre-resolves a bare program name through that same `which` and spawns
the resolved absolute path:

```rust
// A bare name is resolved by airsl. A name containing a separator passes through; that is
// reachable only under `trusted`, since a grant refuses `/bin/echo`.
#[cfg(windows)]
let program = if paths::rules::native::has_separator(&program) {
    program
} else {
    which(&program).ok_or_else(|| /* the refusal of §8 */)?
};
let mut child = std::process::Command::new(&program);
```

**Unix is untouched**: it keeps `Command::new(&program)` (`crates/airsl/src/modules/proc.rs:109`)
and lets `execvp` search. An earlier draft applied the pre-resolution on both platforms, which
would have changed unix behaviour three ways — an unset `PATH` would stop falling back to
`_CS_PATH`; a `PATH` entry with some execute bit but none for the calling user would be selected
rather than skipped in favour of a later entry; and a missing program's error would move from
spawn-time `Error::Io` to pre-resolution. The intent forbids changing unix semantics for their own
sake, and none of those three buy anything on unix, so the `#[cfg]` is the fix rather than a
carve-out in §12.

Three properties follow on Windows. `which` and `run` agree **by construction** rather than by
mirroring `std`'s rules — which matters, because `std`'s `resolve_exe` appends `.exe`
(`library/std/src/sys/process/windows.rs:512`) only when the name contains no `.` at all
(`has_extension`, `:506`), so `run{'foo.bar'}` would otherwise skip the append and diverge from
`which`. Batch files are never spawned. And the promise at `proc.rs:160-161` — "deliberately no
fallback to the current directory" — becomes `airsl`'s own guarantee on Windows rather than a
borrowed one; pre-resolving also bypasses `std`'s `search_paths` step 2, which searches the
directory of the calling executable before `PATH`
(`library/std/src/sys/process/windows.rs:545-551`).

### 5.2 `ProcGrant` is unchanged, and grants stay spelled the same on both platforms

Matching stays an exact, case-sensitive comparison on the program name as written
(`crates/airsl/src/sandbox/grants.rs:218-221`). Because §5.1 puts the `.exe` suffix inside
`which`'s candidate list rather than requiring the caller to spell it, a grant of `git` permits
`run{'git'}` on Windows exactly as on unix, and one script plus one grant works on both
platforms unchanged. This is the reason the suffix belongs in resolution and not in the grant.

The comparison being case-sensitive means `run{'GIT'}` under a `git` grant is refused on Windows
even though the filesystem would resolve it. That is the narrowest available comparison, so it
fails closed, and it keeps one grant semantic rather than two.

### 5.3 Exit status

On Windows `ExitStatus::code()` always returns `Some`, so `unwrap_or(-1)` at
`crates/airsl/src/modules/proc.rs:131` never fires and the comment above it describes a state
that cannot occur. `-1` also stops being a sentinel there — `ExitProcess(0xFFFFFFFF)` produces
it legitimately. No new field is added; the comment is corrected, the documentation records that
Windows `status` is the raw 32-bit exit code narrowed to `i32`, and `result.status ~= 0` remains
the portable way to ask whether the program worked.

## 6. Environment name identity

`types::EnvName` is added to `crates/airsl/src/types/`, the existing home for validated domain
strings alongside `ChunkName`, `EventName`, `ExtensionName`, `ModuleName`, `RequireTarget` and
`RootTable`. It becomes the key type for both `EnvGrant`'s set
(`crates/airsl/src/sandbox/grants.rs:143`) and `Overlay`'s map
(`crates/airsl/src/modules/env.rs:29-32`).

**`EnvName` is total, not validating.** It wraps a `String` and supplies `Eq`, `Ord` and `Hash`
that fold ASCII case on Windows and compare exactly on unix; it rejects nothing and has no
fallible constructor. This is forced by the surrounding API: `EnvGrant::read` is infallible and
`#[must_use]` (`grants.rs:155-164`), so a rejecting constructor would have nowhere to report a
failure, and `panic!` is denied by the workspace lints. A name that cannot exist in a real
environment — one containing `=` or an interior NUL — simply never matches anything, which is
the fail-closed outcome. `EnvName` is the deliberate exception to the guideline's
validate-at-construction rule, and its doc comment says so.

`EnvGrant`'s public API does not change. Its `names` field is already private, and `read`,
`allows`, `names` and `is_empty` keep their signatures; only `allows`'s behaviour on Windows
differs.

### 6.1 The bug this fixes

`Overlay::get` (`crates/airsl/src/modules/env.rs:49-56`) checks a case-**sensitive** `BTreeMap`
and then falls through to `std::env::var`, which on Windows is case-**insensitive**. A script
granted `Path` that calls `env.set("Path", …)` writes overlay key `Path`; `proc::which` looks up
`PATH`, misses, and resolves against the *host's* PATH, while the child process — whose
`Command` environment map is case-insensitive — receives the script's. `which` and `run` resolve
against different PATHs. Folding the key removes the divergence structurally.

### 6.2 `env.all()`

Two further decisions. Returned keys preserve **host casing** (`Path` on Windows, `PATH` on
Linux) with fold-aware dedup, so a host `Path` and an overlay `PATH` collapse to one entry; the
host environment genuinely differs between platforms and `airsl` does not paper over it. And
names beginning with `=` are filtered, because `std::env::vars()` surfaces Windows' per-drive
current-directory pseudo-variables (`=C:`, `=ExitCode`), which are process-private state rather
than environment.

## 7. Filesystem guarantees that change

Three guarantees weaken on Windows. Each is documented at the point it is stated, not left to be
discovered:

- **`fs.atomic_write`** (`crates/airsl/src/modules/fs.rs:417-434`) remains atomic on success, but
  `tempfile::NamedTempFile::persist` can fail with a sharing violation where unix would succeed,
  if another process holds the target open. The rationale comment at `:417-418` ("`/tmp` is
  routinely a different filesystem") is unix reasoning and is rewritten.
- **`fs.stat`**'s `readonly` field (`crates/airsl/src/modules/fs.rs:229`) reports
  `FILE_ATTRIBUTE_READONLY` on Windows, not mode bits.
- **`fs.create_exclusive`** (`crates/airsl/src/modules/fs.rs:152-179`) keeps its exclusivity
  property, since it rests on `create_new`, but on a case-insensitive volume the name space is
  coarser: `CLAIM` and `claim` are one file.

## 8. Error handling

No new public error variants.

**`Error::UncheckablePath` is reused, and its documentation is restated.** Its current doc
(`crates/airsl/src/error.rs:261-265`) says the path "could not be resolved to something a grant
can be checked against" and that it is separate from `Error::Denied` because "the policy did not
refuse this, the path could not be given a meaning to refuse". A verbatim or device-namespace
spelling strains that sentence: such a path *can* be given a meaning, and `airsl` chooses to
refuse the spelling. The variant is still the right home — the refusal is a property of the
runtime's path vocabulary, not of any grant, so `Denied` would be worse — but the doc must widen
to cover "a spelling this runtime will not reason about". `error.rs:261-265` is listed in §11
for that reason.

**A program that cannot be resolved** produces the refusal unix already produces for a missing
program. One addition: `.bat` and `.cmd` are probed on `PATH` **for the diagnostic only**, never
for resolution and never for spawning, so a failed `run{'npm'}` on Windows can say that
`npm.cmd` was found and that `airsl` runs only `.exe`. This is the one place a fixed extension
list beyond `.exe` is consulted; premise 1.2's prohibition is on walking `%PATHEXT%` to *decide
what to run*, and the distinction is stated in the code comment as well as here. Without it the
message could only fire for a file literally named `npm`, which is not the case that motivates
it. This follows the actionable-refusal standard set out at
`crates/airsl/src/modules/guard.rs:95-104`.

## 9. Testing strategy

Every logic-bearing file ships colocated `#[cfg(test)] mod tests`, per the unit-test mandate.
Test names remain sentences describing the behaviour.

**Testable on every platform, both flavours.** All of `paths::rules` — verbatim and device-name
rejection, `to_script_string`, `executable_candidates`, `has_separator` — takes an explicit
`PathFlavor` (premise 1.5), so the *Windows* branch of every lexical rule is exercised on Linux
and macOS. This is the point of the parameterisation: it moves the bulk of the new logic off the
Windows-runner dependency entirely.

**Requires a Windows runner.** Filesystem-truth containment: symlink and junction escapes, the
§3.4 invariant that the guard's verdict matches what the OS actually opens, and the §3.3 root
comparison.

**Symlinks.** The intent forbids silent skipping, since these are the tests that establish the
containment property. All eight sites call `std::os::unix::fs::symlink` today and keep doing so
on unix; the port is **not** a substitution. Each goes through one shared test helper — a
`#[cfg]`-branched `link_file` / `link_dir` pair in a test-support module — so the unix path is
unchanged and the Windows arm is written once rather than eight times. On Windows the helper
picks `std::os::windows::fs::symlink_dir` or `symlink_file` according to the target: the three
directory cases (`crates/airsl/src/modules/guard.rs:265`,
`crates/airsl/src/sandbox/grants.rs:311`, `crates/airsl/src/extension/negotiate.rs:515`) take
`symlink_dir`; the five file cases (`crates/airsl/src/modules/guard.rs:245`,
`crates/airsl/src/require_loader.rs:333` and `:348`,
`crates/airsl/src/extension/manifest.rs:822`, `crates/airsl/src/extension/loaded.rs:515`) take
`symlink_file`. Getting that split wrong yields a link that resolves but is not traversable.

An earlier draft used directory *junctions* for the three directory cases on the theory that they
need no privilege. That is doubly wrong: `std` exposes no junction-creation API, and creating one
by hand needs `DeviceIoControl` with `FSCTL_SET_REPARSE_POINT`, which
`#![forbid(unsafe_code)]` (`crates/airsl/src/lib.rs:31`) forbids crate-wide including test
modules — and it is unnecessary anyway, because `std` passes
`SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE` (`library/std/src/sys/fs/windows.rs:1432`), so
plain `symlink_dir` succeeds unprivileged once Developer Mode is on. The CI job enables it:

```
reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock" /t REG_DWORD /f ^
  /v AllowDevelopmentWithoutDevLicense /d 1
```

A test whose symlink creation is denied **fails loudly**, naming Developer Mode, so a maintainer
running locally is told what to enable rather than getting a false green.

**Case-sensitivity tests are scoped to where case can vary.** Per §3.6, a path and root that both
exist come back from `canonicalize` in on-disk casing, so a "differs only by case" test would not
reproduce on a default NTFS volume. The regression tests target the two cases that do vary: a
write root that does not exist yet (§3.3) and a non-existent tail below an existing ancestor. On
the environment side, a test pins that an `EnvGrant` on `PATH` permits reading `Path` on Windows
and does not on unix.

**Subprocess fixtures.** Windows `proc` tests use `where.exe` — a real executable present on
every installation, with no shell involved: `where where` for stdout, `where nonexistent-xyz`
for a non-zero status and for stderr. The unix fixtures (`sh`, `echo`, `false`) stay on unix.
`arguments_are_passed_without_a_shell` (`crates/airsl/src/modules/proc.rs:238-249`) gets a
Windows payload exercising `&`, `|`, `^` and `%FOO%` rather than the unix
`'a b; rm -rf *'`, which proves nothing about the property on that platform.

**Existing tests that break, and how the list is obtained.** These are found by *running* the
suite on `windows-latest`, not by reading it — the same reason §4 delegates its inventory to the
compiler. §10's task 2 is that run, and its failure list is the work list. Three classes are known
in advance and seed it; none is claimed to be complete.

*Class 1 — temp paths interpolated into escape-processing string literals.* A temp path on a
`windows-latest` runner contains `\U`, an invalid TOML escape, so the manifest fails to parse.
Known sites: `crates/airsl-cli/src/ext_fire.rs:318-326` and `crates/airsl-cli/src/check.rs:160`;
and in the library's own tests, `crates/airsl/src/extension/host.rs:467-471` and
`crates/airsl/src/extension/loaded.rs:375-379` (TOML basic strings), with `host.rs:476` and
`loaded.rs:384` (Lua single-quoted strings). Fixed by normalising to `/` before interpolation,
which premise 1.1's input vocabulary accepts.

*Class 2 — tests asserting an error that cannot occur on Windows.* §3.4's `GetFullPathNameW`
collapses `..` before the guard sees it, so the `ParentDir` refusal is unreachable there. Known
sites: `crates/airsl/src/modules/guard.rs:294-304` and `:254-273`. These take
platform-conditional expectations rather than being deleted — the unix assertion is still the one
that matters on unix.

*Class 3 — tests asserting a natively-rendered path in a message or chunk name*, which §4
re-spells. Known sites: `crates/airsl/src/modules/guard.rs:326-341`,
`crates/airsl/src/script.rs:219` and `:278`.

`crates/airsl-cli/src/check.rs:199` and `crates/airsl-cli/src/test_runner.rs:247` assert
`/`-joined relative paths and **are not fixed by §4**. Their expectations are built inside the
test bodies themselves (`check.rs:188-196`, `test_runner.rs:240-243`) by
`strip_prefix(…).to_string_lossy()` over `PathBuf`s from the CLI's own `discover`
(`check.rs:91-97`, `test_runner.rs:89-95`), which never crosses an `airsl` module boundary — and
`paths::rules` is `pub(crate)`, so `airsl-cli` cannot reach it. Both tests normalise the
separator in their own test bodies. `airsl`'s public API is deliberately not widened for this:
the CLI renders those paths only for its own assertions, and exporting a path-spelling helper to
fix two test expectations would put a permanent item on the public surface to serve test code.

## 10. Build, CI, and tooling

**Two green-field tasks come first, before any design work is implemented.**

*Task 1 — verify the vendored Lua build.* Because the `compile_error!` fires before anything
else, no one has ever observed whether this workspace's vendored Lua compiles on Windows.
Relaxing the gate and running a build on `windows-latest` is the first thing execution does;
every estimate downstream assumes it succeeds.

*Task 2 — run the full suite on `windows-latest` and record every failure.* This is the task §9's
inventory mechanism depends on: the failure list, not a reading of the test files, is what the
subsequent plans work from. It runs with the gate relaxed and nothing else fixed, so its output is
the honest starting inventory.

**Matrix.** `.github/workflows/ci.yml:37` gains `windows-latest`, and the comment at `:31-33`
explaining the removal is rewritten. `shared-key: gate-${{ matrix.os }}` at `:56` already keys
the cache per-OS. The multi-line diagnostic step at `:46-51` gets `shell: bash` so its semantics
match the other legs. The `deny` job stays `ubuntu-latest` only, since `deny.toml:13-17`
deliberately views the whole graph regardless of target.

**`cargo make`.** The five gate tasks (`Makefile.toml:47-91`) are plain `command`/`args` and are
already portable. Two POSIX-shell tasks are ported to duckscript so they run on every platform:
`examples` (`Makefile.toml:111-137`) and `dod-crate` (`:202-215`), the latter because CLAUDE.md
documents it as the per-crate iteration command and a Windows contributor needs it. The third,
`publish-dry-run` (`:166`), gets a `windows_alias` that skips it with a message: it is a
release-time task and releases are cut on unix.

**Examples carve-out, and how it stays honest.** `crates/airsl/examples/env-and-proc` and
`crates/airsl/examples/denials-are-data` spawn `sh`. No program exists on both platforms that
would let them keep the byte-for-byte output promise, so they stay unix-only; the other twelve
run everywhere.

The mechanism matters, because the `examples` task discovers by directory glob deliberately —
its comment at `Makefile.toml:103-106` records that "an example that is added but not listed
would silently never run, which is the failure mode a coverage suite can least afford", and a
hand-kept skip list inside the task would re-create exactly that. Instead, an example opts out by
carrying a marker file in its own directory (`unix-only`, containing the reason). The task still
discovers every directory by glob; on Windows it runs each one that has no marker and *reports*
the ones it skipped, so a skipped example is visible in the log rather than absent from it. A
newly added example with no marker runs on Windows and fails there if it cannot — loudly, which
is the property the glob exists to protect.

**Toolchain.** `rust-toolchain.toml:9` pins channel `1.94` with `profile = "minimal"`, which
resolves to `x86_64-pc-windows-msvc` on `windows-latest`. MSVC Build Tools are preinstalled on
that runner, so no extra step is required.

## 11. Documentation

Every claim below is a documented promise that becomes false and must be restated. Edits keep the
evidence standard the rest of `crates/airsl/docs/` holds: a claim about code that exists carries
a `file:line`.

**How the list is obtained.** As with §4 and §9, reading for these proved unreliable — a
`CLOCK_MONOTONIC` claim and two `O_CREAT|O_EXCL` claims survived two drafts. The plans run a
pattern sweep over `crates/`, `docs/`, `*.md` and doc comments for unix-specific vocabulary —
`unix`, `POSIX`, `/tmp`, `/etc`, `/bin`, `mode bits`, `CLOCK_`, `O_CREAT`, `O_EXCL`, `execvp`,
`symlink`, `Linux`, `macOS` — and triage every hit. The list below seeds that sweep; it is not
asserted to be its complete output.

Found by that sweep and not previously listed:

- `crates/airsl/docs/stdlib.md:129` and `crates/airsl/docs/how-to.md:53` — both state
  `fs.create_exclusive` "is `O_CREAT|O_EXCL`", which names a POSIX flag pair with no Windows
  equivalent. §7 records what the guarantee becomes there.
- `crates/airsl/docs/how-to.md:33` — "`/tmp` is not used" as the `atomic_write` rationale. §7
  rewrites only the `fs.rs:417-418` twin; this one says the same thing in the user-facing docs.
- `crates/airsl/docs/stdlib.md:105` and `crates/airsl/src/modules/time.rs:93` — `monotonic`
  "reads `CLOCK_MONOTONIC`", which on Windows is `QueryPerformanceCounter`. The behaviour is
  correct on both; only the stated mechanism is unix-specific.

- `crates/airsl/src/lib.rs:33-43` — the `compile_error!` and its comment are removed.
- `CLAUDE.md:11-14` — the "unix only" build requirement.
- `CLAUDE.md:87` — the line naming `guard.rs` as the only place the containment rule is written.
  `PathGuard::resolve` stays there (§2), but the root comparison moves to `paths::containment`,
  so the sentence needs to name both.
- `crates/airsl/src/error.rs:261-265` — `Error::UncheckablePath`'s doc, per §8.
- `README.md:31-33`, `crates/airsl/README.md:21-23`, `crates/airsl-cli/README.md:12-14` — the
  "Linux and macOS only" paragraphs.
- `crates/airsl/docs/sandbox.md:227-230` — `proc` grant granularity, which needs the `.exe` rule
  and the case-sensitivity note from §5.2.
- `crates/airsl/docs/how-to.md:94` and `:175`, `crates/airsl-cli/README.md:105` — unix path and
  program examples in denial output.
- `crates/airsl/docs/README.md` — the status table gains the Windows rows.
- `crates/airsl/docs/stdlib.md:32-33` — the determinism statement, which §4 refines rather than
  weakens.
- `crates/airsl/examples/README.md:73` and `:82-83` — the byte-for-byte promise and "only `sh`
  is ever executed", per the §10 carve-out.
- `crates/airsl/src/modules/time.rs:3-5` and root `Cargo.toml:34-35` — the `/etc/localtime`
  rationale for choosing `jiff`. The choice stays correct; the reason needs a Windows clause,
  since `jiff` bundles tzdb there.
- `crates/airsl/docs/architecture.md:27` — cites `Cargo.toml:44` for `mlua`, which is stale; it
  is root `Cargo.toml:19`. Corrected while the surrounding C-compiler paragraph is updated.
- `crates/airsl-cli/README.md:286-292` — the `#!/bin/sh` hook launcher needs a `.cmd` sibling.
- `CHANGELOG.md` — a new entry recording the platform change.

## 12. Non-goals

Carried from the intent:

- Any target other than `x86_64-pc-windows-msvc`. `x86_64-pc-windows-gnu` (MinGW/MSYS2),
  `aarch64-pc-windows-msvc`, `i686-*` and UWP are out.
- WSL, which is already covered as a Linux target and is not Windows support.
- Any shell, `cmd.exe` or PowerShell surface on `airsstack.proc`, including a grant that would
  opt into batch files.
- Windows-specific capability modules — ACLs, registry, services, event log, drive enumeration.
  This chain ports the existing twelve modules and adds none.
- Changing unix semantics for their own sake. Cross-platform reconciliation is in scope; unix-side
  redesign is not. `run`'s pre-resolution is `#[cfg(windows)]` for this reason (§5.1), and
  `reject_unrepresentable`'s device-name refusal is Windows-only (§2).

  This spec makes **two** unix-observable behavioural changes, both argued rather than incidental:

  1. §4.1's `normalize("/..")`, which returns `"..//"` today and `"/"` afterwards.
  2. A path containing an interior NUL. Today the guard approves it — `std::path::absolute` does
     not reject it on unix — and it fails later as `Error::Io`. Under §2's `InteriorNul` rule it
     is refused at the guard as `Error::UncheckablePath`. The outcome is unchanged (the call
     fails either way) and no unix syscall could ever have expressed such a path; what changes is
     that the refusal is earlier and names the reason. Scoping `InteriorNul` to Windows was
     considered and rejected: it would leave unix with the worse diagnostic to protect a
     behaviour nothing depends on.

Decided at spec time rather than carried, and therefore wanting explicit sign-off:

- **`%PATHEXT%` walking for resolution**, per premise 1.2. `.bat`/`.cmd` are probed only for the
  §8 diagnostic.
- **Long-path (`>MAX_PATH`) support as an explicit feature.** It is inherited from `std` per
  premise 1.4, neither engineered nor tested here.
- **A Windows-appropriate `proc` example**, deferred by the §10 carve-out. The two `sh`-based
  examples stay unix-only and nothing replaces them on Windows.
- **A `.cmd` hook launcher**, beyond the documentation change in §11.
- **Publishing a release** advertising the new target.
