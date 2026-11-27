# Changelog

Notable changes to both crates in this workspace, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and both crates follow
[SemVer](https://semver.org/) — with the caveat that for a `0.y.z` version the *middle* number is
the slot Cargo treats as breaking, so a compatible change ships as a `z` bump.

`airsl`, `airsl-cli` and the native module's LuaRocks packages share one version. Every release is a
lockstep release of both crates, tagged `airsl-v<version>` and `airsl-cli-v<version>` on the same
commit, and there is no library-only release. Older entries keep their per-crate headings, from when
each crate had a number of its own and one commit could ship them under two different numbers.

## Unreleased

### Breaking

- **`airsl` gains Cargo features.** With `default-features = false`, a dependent now has to enable
  `lua54` or `luajit` itself, because no Lua is selected without one, and must also enable
  `vendored`, or `mlua` looks for a system Lua through pkg-config. `Engine` is `Send + Sync` only
  with the default `send` feature.

## airsl 0.2.0 — airsl-cli 0.2.0 — 2026-11-25

A breaking release: `Policy::pure()` now keeps the promise its name makes, which stops scripts that
worked on 0.1.6, and the documented meaning of `GrantSet::declared()` changes. Both crates take a
middle-number bump, per the rule above. It closes
[#11](https://github.com/airsstack/airsl/issues/11),
[#12](https://github.com/airsstack/airsl/issues/12),
[#13](https://github.com/airsstack/airsl/issues/13) and
[#14](https://github.com/airsstack/airsl/issues/14). What `pure` guarantees, and what it still
leaves to the VM, is in
[What airsl Can and Cannot Promise](https://airsstack.github.io/airsl/ch09-03-what-it-promises.html).

### Breaking

- **`pure` reaches no host.** A `pure` script no longer has `print`, and the host modules refuse
  what reached the host: `stdio` and `hook` (stdin, stdout and stderr), `time.now` and
  `time.monotonic` (the clock), `time.parse` of text that names an IANA time zone such as `UTC`
  or `America/New_York` (a numeric offset such as `+0400` still parses), and `path.absolute` of a
  relative path (the working directory). A refusal names the access that would allow it. Each of
  these worked under `pure` on 0.1.6. To grant one back, pass `--allow-access NAME` on the command
  line, or build the policy on `GrantSet::sealed().with_access(HostAccess::…)` in Rust.
- **A relative path needs `cwd` under `pure`.** `fs`, `hash.hash_file` and `glob.walk` refuse a
  relative path such as `p.txt` unless `cwd` is granted, even when filesystem roots are, because a
  relative path is read against the working directory. An absolute path inside a granted root is
  unaffected. On 0.1.6, `pure` with `--allow-read .` accepted relative paths; now it also needs
  `--allow-access cwd`. The same holds for any policy built on `GrantSet::sealed()` that does not
  add `cwd`; `confined` holds `cwd` and `trusted` holds everything, so neither is affected.
- **`math.randomseed()` with no argument is refused under `pure`**, because it reads the clock.
  `math.randomseed(n)` still works.
- **`math.random` repeats under `pure`.** Every fresh engine starts from the same seed, so the same
  script gives the same numbers on every run. A reused engine continues the sequence, as it
  keeps its Lua state.
- **`require` is a refusing function under `pure`**, where it was `nil`. Calling it fails with
  `require denied`, so a script that tested `require == nil` to detect the sandbox now sees a
  function. Granting `require` (`--allow-access require`) gives back the loader confined to the
  script's folder.
- **`GrantSet::declared()` now holds all seven host-access members**, so
  `GrantSet::declared().is_empty()` is `false` and `GrantSet::declared().to_string()` is
  `access stdin,stdout,stderr,clock,cwd,require,zones` where it was `none`. A host that built a
  `pure`-based policy on `declared()` or `default()` reopens every member; build on
  `GrantSet::sealed()` instead. See
  [Granting from Rust](https://airsstack.github.io/airsl/ch06-03-granting.html).

### Added

- **`HostAccess`**, the seven capabilities that need no path, variable or program: `stdin`,
  `stdout`, `stderr`, `clock`, `cwd`, `require` and `zones`. A host gives a `pure`-based policy any
  of them back, `require` included, and each host module checks its member before acting.
- **`GrantSet::sealed()`**, which holds no member and is what `pure` is built on, and
  **`GrantSet::with_access`**, **`allows`** and **`access`** to add, test and list members.
- **`Error::RequireDenied`**, returned by the refusing `require`.
- **`--allow-access NAME`** on `run`, `test`, `ext doctor` and `ext fire`, repeatable, naming a
  member to give back to `pure`. It changes nothing under `confined`, which holds every member, and
  `trusted` ignores it, as it ignores every grant flag. `ext doctor` and `ext fire` always use the
  `confined` ceiling, so there the flag cannot change it.
- **`airsl doctor` takes the grant flags**, so it reports the policy a script would get from the
  same command line, for example `airsl doctor --policy pure --allow-access stderr`.
- **Appendix F, "Lua Functions by Surface"**, in the book: every base global and library name
  each surface loads, and which preset uses which.

### Changed

- **A refusal names the path as the script wrote it.** With no filesystem roots granted, `fs`,
  `hash.hash_file` and `glob.walk` refuse before touching the filesystem, so `/etc/hosts` is
  reported as `/etc/hosts` rather than as the symlink-resolved `/private/etc/hosts` it becomes on
  macOS, and `p.lua` rather than the path joined to the working directory. A refusal no longer
  reveals where the process runs.
- **A path that could not be resolved is now `Error::Denied`** when no filesystem roots are
  granted, where it was `Error::UncheckablePath`: when the working directory could not be read,
  when the path climbs through a directory that does not exist, and when no part of it exists. A
  caller that matched the variant or the text for those inputs should match `Error::Denied`. The
  cases `UncheckablePath` still names, a NUL byte, a verbatim path and a device path, are
  unchanged.
- **The `grants:` lines of `airsl doctor` and `airsl ext doctor` list host access.** Every
  `confined` line now starts with `access stdin,stdout,stderr,clock,cwd,require,zones`, ahead of
  any allowlist entries. Where it read `none` it reads that list alone, and where it read
  `exec git` it now reads
  `access stdin,stdout,stderr,clock,cwd,require,zones; exec git`. A program that parses these
  lines must expect the prefix. `pure` still reads `none` until a member or an entry is granted.
- **`ext.granted()` gains an `access` key**, a list of the granted members in declaration order,
  empty when none is granted. A script that iterates the table's keys sees one more. An extension
  holds the ceiling's host access and cannot request more.
- **`print`, `require` and the random seeding are unchanged under `confined` and `trusted`**, which
  behave there as on 0.1.6.

## airsl 0.1.6 — airsl-cli 0.1.6 — 2026-11-24

Compatible changes, so both crates take a `z` bump.

### Added

- **`airsl::VERSION`**, the version of the `airsl` crate a program was built against. An embedder
  can record which runtime ran a script, or compare it with an installed `airsl` binary, without
  reading its own `Cargo.lock`.

### Changed

- **Current majors of `sha2`, `sha1` and `toml`.** `sha2` and `sha1` move to 0.11 and `toml` to 1,
  so a crate that depends on `airsl` and on the current releases of these crates no longer
  compiles two copies of `sha2`, `digest` and `toml`. Hash output is unchanged. A manifest parse
  error still carries the parser's message, which now comes from `toml` 1.

## airsl 0.1.5 — airsl-cli 0.1.5 — 2026-11-23

Compatible changes, so both crates take a `z` bump.

### Fixed

- **A module loaded by `require` now receives its name and path in `...`.** A module can read them
  with `local name, path = ...` and require its siblings by a name relative to its own. A module
  that returns nothing now yields `true` on its first `require`, as it already did on later ones.

### Changed

- **Documentation is now [The airsl Book](https://airsstack.github.io/airsl/).** It replaces
  `crates/airsl/docs/`, which is removed. Both crates' READMEs are rewritten as short introductions
  that link into it, and both crates name it as their `homepage`.
- **Shorter install commands.** `curl -LsSf https://airsstack.github.io/airsl/install.sh | sh` and
  its PowerShell counterpart install the latest release, or the one `AIRSL_VERSION` names.

## airsl-cli 0.1.4 — 2026-10-22

How the binary reaches a machine changes; what it does once there does not. `airsl` has no change
and stays at 0.1.4.

### Added

- **Prebuilt binaries.** Each `airsl-cli` release is built for macOS (Apple Silicon and Intel),
  Linux (x86_64 and ARM64, each as a glibc build and a fully static musl build) and Windows
  (x86_64), and published as a GitHub Release with a SHA-256 checksum and a GitHub artifact
  attestation per archive. A shell installer (macOS, Linux) and a PowerShell installer (Windows)
  pick the archive for the machine and install `airsl` into `~/.local/bin`. Every archive is
  smoke-tested on its own platform before the release is published. Running `airsl` no longer
  needs a Rust toolchain or a C compiler; `cargo install --locked airsl-cli` works as before.

## airsl 0.1.4 — airsl-cli 0.1.3 — 2026-08-30

A third platform, and no public item changed shape to get there: one type is added, `Error` is
`#[non_exhaustive]`, and every existing signature is untouched. A `z` bump in both crates, per the
rule above — the two behaviour differences below are corrections to contracts the code already
claimed, and neither stops anything compiling.

### Added

- **Windows support.** `airsl` and `airsl-cli` build and run on `x86_64-pc-windows-msvc`, under the
  same policy model as Linux and macOS — the same script, the same grant, the same host modules,
  on every target. A path a script sees is always `/`-separated regardless of the platform
  underneath, and `airsstack.proc` resolves and runs only `.exe` programs there, so a grant of
  `npm` does not reach the `.cmd` shim `npm` installs as.

- **`EnvName`**, the identity two environment variable names are compared and hashed under. The
  only new public item in this release. It exists because the allowlist and the environment
  overlay both need the same answer to "are these two names the same variable", and that answer is
  not a constant: the Windows environment block folds ASCII case unconditionally, a unix one does
  not. `EnvGrant` is built on it, so `EnvGrant::none().read(["Path"])` and `.read(["PATH"])` are
  the same grant on Windows and two different ones on unix — the platform's own answer either way.

### Changed — behaviour worth checking before upgrading

These two are the only differences an existing unix user's `cargo update` would notice; the path
rules behind them are now shared code rather than written once per target.

- `path.normalize("/..")` returns `"/"`. It previously returned `"..//"`.
- A path spelling containing an interior NUL byte is now refused where the sandbox first resolves
  it, as `Error::UncheckablePath`, rather than reaching the operating system and failing later as
  `Error::Io`.

### Fixed

- `airsl ext doctor` rendered the `requested:` entry path with the platform's own separator while
  every grant line a few rows below it used `/`, so one report spelled two paths two ways. Windows
  only — on unix the two spellings were already the same string.

`airsl-cli`'s dependency floor moves to `airsl` 0.1.4, so `cargo install --locked` picks up this
release rather than pinning the previous one.

## airsl 0.1.3 — airsl-cli 0.1.2 — 2026-08-23

One feature, additive end to end: no public item changed shape, `Error` is `#[non_exhaustive]`
so its new variants break no match, and the only thing an existing script sees is a new
`airsstack.ext` subtable. A `z` bump in both crates, per the rule above.

### Added

- **The extension system.** A host program loads third-party Lua extensions that declare the
  capabilities they need in an `extension.toml` manifest; the host bounds those requests with its
  own `Ceiling`, an `Approver` decides within that bound, and the result is an ordinary `Policy`
  enforced by the engine that already exists.

  - `airsstack.ext` (`on`, `granted`) and `Engine::dispatch` — a script registers a handler for a
    host-declared event and the host calls it repeatedly on one persistent engine, rather than the
    script running once to completion.
  - The manifest parser (`extension::manifest`) — `extension.toml`'s `[extension]`,
    `[capabilities]`, `[capabilities.optional]` and `[limits]` blocks, with `$VAR` expansion from
    host-supplied variables.
  - `Ceiling`, negotiation (`extension::negotiate`) and the `Approver` trait, with
    `ManifestApprover` and `DenyAll` shipped — a manifest's request is intersected with the host's
    ceiling, and a required capability outside it fails the load before the approver is even asked.
  - `ExtensionHost` and `Extension` (`extension::host`, `extension::loaded`) — `load` and
    `load_dir` turn a directory (or a whole tree of them) into running, negotiated extensions
    without short-circuiting on the first failure, and `broadcast` fans one event out to every
    loaded extension, isolating each one's outcome into its own `Dispatch`.
  - `airsl ext doctor` and `airsl ext fire` — inspect what a ceiling would grant, reduce and deny
    for a manifest without running its entry script, or load the extension for real and dispatch
    one event with the payload on stdin and byte-stable JSON on stdout.

## airsl 0.1.2 — airsl-cli 0.1.1 — 2026-08-23

Four fixes, every one of them found by writing an example against the real API rather than by
reading it.

### Fixed

- **`require` was `nil` under `Policy::trusted()`.** `trusted` loads `package`, and
  `luaopen_package` installs `require` alongside it — which the engine then cleared before every
  evaluation, leaving a `full` script holding `package.path` and `package.loaded` with no `require`
  to use them with. The cause was the shape of the question rather than a missing branch: the
  predicate asked whether a surface should get the *confined* loader, and `Full` and `Minimal` both
  answer no for opposite reasons, so they shared the arm that cleared the global.
  `RequireDisposition` now names the three answers there are, matched exhaustively, so a fourth
  surface cannot acquire one by default.

- **`time.monotonic` was not monotonic.** It read `SystemTime::now()` — the wall clock, the same
  source `now` reads — while the comment above it promised the reading was "unaffected by the clock
  being adjusted underneath a running script". A backwards NTP step or `settimeofday` between two
  readings made the later one smaller, so a script measuring a duration got a negative one and a
  script computing a deadline got one that never elapsed. It now reads `std::time::Instant` from a
  process-wide origin, which needs no dependency: no datetime crate could supply this, because a
  calendar point is defined by the wall clock and so `jiff` and `chrono` alike bottom out in
  `SystemTime::now`.

- **A refusal's pronoun had no antecedent.** ``fs.read denied: `/etc/hostname` is outside them — no
  read roots are granted`` referred to a noun the sentence never contained. Both branches now open
  the same way: ``is outside the granted read roots: none are granted``. Which allowlist a refusal
  was measured against is carried as a type rather than a string compared to select one, so a
  message cannot name a direction the check did not use.

### Added

- **`Script::with_name`**, for naming a chunk loaded from a file. `from_file` takes the chunk name
  from the path as given, which is right for a developer running a script they pointed at and wrong
  for a hook whose diagnostics leave the machine — a chunk name reaches every traceback, and a
  hook's stderr routinely lands in someone else's log. Reading the file and going through
  `from_source` was the only prior workaround, and it silently discards the inferred root, so a
  script renamed that way loses `require`.

- **Thirteen runnable examples** under `crates/airsl/examples/`, each a directory with its own
  README whose `## Output` block is real captured stdout. `cargo make examples` runs all of them
  end to end, compiles every shipped `.lua` with `airsl check`, and runs `airsl test` over the tree
  — closing a gap where the CLI's own test runner was a documented entry point that no gate
  exercised.

### Changed — behaviour worth checking before upgrading

- Under `Policy::trusted()`, `require` is a function. A script that wrote `if require then` to
  detect its surface takes the other branch now.
- `time.monotonic` returns seconds since an unspecified origin — around zero, not around 1.7e9. Any
  caller reading it as a timestamp was relying on the one thing its own documentation said was not
  there.

Both are patch releases regardless: nothing stops compiling, and the documented contract is
unchanged in each case — the code started honouring it.

`airsl-cli` carries no source changes. Its dependency floor moves to `airsl` 0.1.2, so
`cargo install --locked` picks up that release rather than pinning 0.1.1.

## airsl 0.1.1 — airsl-cli 0.1.0 — 2026-08-22

### Changed

- Documentation and comment wording only in `airsl`; no behaviour change. Vocabulary carried over
  from the monorepo this crate was extracted from — "the plugin suite this crate was built for" and
  similar — now describes a standalone runtime.

### Added

- First release of `airsl-cli`, published from the same commit as `airsl` 0.1.1: `run`, `test`,
  `check` and `doctor`.

## airsl 0.1.0 — 2026-08-22

First release. An embeddable Lua 5.4 runtime where the host decides what a script may reach: three
independent policy axes (language surface, grants, resource ceilings), eleven host modules under a
configurable root table, and a type-state builder with no `build()` until a policy has been chosen.

Unix only, and a C compiler is required — `mlua`'s `vendored` feature compiles Lua 5.4 from C
source and links it statically, so there is no system Lua and no `pkg-config` involved.
