# Changelog

Notable changes to both crates in this workspace, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and both crates follow
[SemVer](https://semver.org/) — with the caveat that for a `0.1.z` version the *middle* number is
the slot Cargo treats as breaking, so a compatible change ships as a `z` bump.

The two crates are versioned independently and have released together so far, so each entry names
both. Every release is tagged per crate — `airsl-v0.1.2`, `airsl-cli-v0.1.1` — because one commit
has shipped two crates under two different numbers, which a single `vX.Y.Z` tag cannot name.

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
