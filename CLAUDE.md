# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`airsl` embeds Lua 5.4 in Rust and lets the **host** decide what a script may reach. Two published
crates in one workspace: `crates/airsl` (the library) and `crates/airsl-cli` (the `airsl` binary —
`run`, `test`, `check`, `doctor`, `ext doctor`/`ext fire`, `lua setup`). A third member,
`crates/docs-check`, is unpublished and holds only tests that keep the prose true to the code.

Build requirement that is not negotiable: **a C compiler** (`mlua`'s `vendored` feature compiles
Lua 5.4 from C source and links it statically — no system Lua, no `pkg-config`). On Linux and macOS
that is whatever `cc` the platform provides; on `x86_64-pc-windows-msvc` it is MSVC Build Tools,
preinstalled on `windows-latest`.

## Commands

The gate is [cargo-make](https://github.com/sagiegurari/cargo-make)
(`cargo install --locked cargo-make`). CI runs the same command a developer runs.

```bash
cargo make dod          # the gate: fmt-check, clippy, rustdoc, tests, doctests — all warnings-as-errors
cargo make dod-crate airsl   # the same five steps scoped to one crate, while iterating
cargo make fmt          # the only task that rewrites the tree
cargo make clippy-fix   # apply machine-applicable clippy suggestions
cargo make deny         # advisories, licenses, duplicates, sources (not part of dod, by design)
cargo make install      # install the airsl binary from this checkout
cargo make --list-all-steps
```

Anything short of `cargo make dod` is iteration, not a green result. `-D warnings` in the clippy
step promotes plain rustc warnings too, which is why there is no separate `cargo build` step, and
doctests need their own run because `--all-targets` excludes them.

The tests step also runs `crates/docs-check`, the unpublished member that guards the prose. It
checks these four things and nothing else:

- Each example README's `## Output` block equals what the example prints. Where Windows prints
  something else (a path that starts with a drive), the first block under a `### Windows`
  sub-heading inside `## Output` is what Windows must print instead. An example with no block
  must carry a `no-output` marker file holding the reason, and be named in `EXPECTED_NO_OUTPUT` in
  `crates/docs-check/src/examples.rs`; an example with a `unix-only` marker file must be named in
  `EXPECTED_UNIX_ONLY` there. The `examples` task of `Makefile.toml` may compare output with the
  book appendix only, never with an example's README, which this check owns.
- Every `.rs` path in a code span of `CLAUDE.md` or a README exists, and none carries a line-number
  suffix (cite the symbol in prose instead). A path resolves against the citing document's own
  crate; `CLAUDE.md` and the root README resolve from the repository root, so a path into a crate
  there starts with `crates/<name>/`.
- In `CLAUDE.md` alone, a module path in a code span (`require::loader`,
  `extension::manifest::validate_entry`) names a module that exists in some workspace crate, and
  the item after it, if any, is declared in that module's source (a `mod`, `fn`, `struct`, `enum`,
  `union`, `trait`, `type`, `const`, `static` or `macro_rules!` item, or a `pub use` that
  re-exports it), with comments and strings ignored, so prose that merely mentions the word does
  not count. Associated items of an `impl` block and items inside a `#[cfg(test)]` module are not
  read, so a method is cited through its type (`extension::loaded::Approved::recheck_entry`). A lone
  snake-case name with an underscore appears somewhere in the `.rs` and `Cargo.toml` files outside
  `crates/docs-check`, unless it is listed with its reason in `ANTI_EXAMPLES` in
  `crates/docs-check/src/citations/module.rs` (for a name this file cites as what not to write).
  Paths into `std`, a dependency or a workspace crate (`airsl::mlua`) and type paths
  (`Policy::trusted()`) are not read.
- Every book `{{#include}}`, `{{#rustdoc_include}}` or `{{#playground}}` directive of a chapter
  points at a file that exists.

It does not check that a cited symbol is in the cited file, that the book's prose is true, or
anything else written in backticks. `cargo make dod-crate docs-check` runs it alone.

Single test / narrower runs:

```bash
cargo test -p airsl modules::hash          # one module's tests
cargo test -p airsl -- confined_restricts_the_surface --exact
cargo test -p airsl-cli --all-targets
cargo bench -p airsl                       # full timing pass; any other invocation smoke-runs it
```

Exercising the binary during development:

```bash
cargo run -p airsl-cli -- doctor --policy confined
cargo run -p airsl-cli -- run --allow-read . script.lua arg1
cargo run -p airsl-cli -- test  path/        # runs *_test.lua / test_*.lua
cargo run -p airsl-cli -- check path/        # compiles every .lua without running it
```

## Architecture

Three independent layers; collapsing them is the most common mistake. The book's
`book/src/ch09-01-three-layers.md` is the long form.

1. **The VM** — `mlua` with `lua54,vendored,serialize,send`. Lua 5.4 specifically, because only
   5.3+ distinguishes integers from floats, which byte-stable JSON depends on.
2. **Policy** (`src/sandbox/`) — three separate questions, never one switch: `LanguageSurface`
   (which of *Lua's own* libraries a script sees), `GrantSet`/`FsGrant`/`EnvGrant`/`ProcGrant`
   (what host modules may touch), `ResourceLimits` (memory + instruction ceilings, armed on the
   state before any module installs). `Policy::trusted() / confined() / pure()` are the presets.
3. **The capability surface** (`src/modules/`) — twelve `HostModule` implementations installed as
   subtables of a single Lua global, per-engine and defaulting to `airsstack`.

`src/extension/` sits on top of those three layers: it reads a manifest, bounds the request with a
host-supplied `Ceiling`, asks an `Approver`, and only then builds an ordinary `Engine` under the
negotiated `Policy`. `ExtensionHost::load` (`crates/airsl/src/extension/host.rs`) is the entry
point a host calls; `Extension::approve` followed by `Approved::start`
(`crates/airsl/src/extension/loaded.rs`) is the type-state that proves a denial ran before any
extension code does.

Key seams:

- `Engine::dispatch` (`crates/airsl/src/engine.rs`) is how a host invokes the handlers a script
  registered with `ext.on`. Handlers live in a registry table written by
  `crates/airsl/src/modules/ext.rs`; the engine records the evaluating `ThreadId` so a handler that
  re-enters its own engine gets `Error::Reentrant`, never a deadlock.
- `Engine::builder()` is a **type-state builder** (`crates/airsl/src/builder.rs`,
  `Missing`/`Present`): there is no `build()` until `policy()` has been called, so a sandbox cannot
  be forgotten.
- `HostModule::install(&mlua::Lua, &mlua::Table, &InstallContext)` is the extension seam, and
  `describe` beside it lists the module's Lua surface for editors and the drift check
  (`luals::verify`). `mlua` is re-exported as `airsl::mlua` deliberately — it is part of the
  contract, and a separately declared version produces type errors that never name the real cause.
  `InstallContext` carries the *engine's* policy, so what a module enforces and what `airsl doctor`
  reports cannot diverge.
- `modules::stdlib()` (`crates/airsl/src/modules/stdlib.rs`) is the single list of built-ins; the
  engine, the doctor output, and tests all read it. A new module is registered there.
- The filesystem containment rule is written in two places, each the only place for its half.
  `crates/airsl/src/modules/guard.rs` (`PathGuard::resolve`) turns a path a script wrote into the
  absolute, symlink-free path an operation would actually touch.
  `crates/airsl/src/paths/containment.rs` holds the root comparison itself, shared by
  `crates/airsl/src/sandbox/grants.rs`'s `FsGrant::allows_read`/`allows_write` (what `PathGuard`
  consults) and, outside `fs`, by `require::loader`, `extension::manifest::validate_entry` and
  `extension::loaded::Approved::recheck_entry`. Every `fs` call, plus `hash.hash_file` and
  `glob.walk`, funnels through `PathGuard`.
- `require("<root>")` is answered by `require::root` before any other resolution, and through
  `package.loaded` on `Full`.
- `Engine` is `Send + Sync` and holds a lock spanning a whole evaluation
  (`crates/airsl/src/engine.rs`) — `mlua`'s per-operation locking was memory-safe but not correct
  across the four steps of an eval. A shared engine avoids rebuilding state; it never buys
  parallelism.
- Engine reuse is ~30× cheaper than rebuilding, so it is an API-shape decision. Per-evaluation:
  instruction counter, `arg` table, `require` root. Persisted across evaluations: the `require`
  module cache (keyed by canonical path), Lua globals a script wrote.

`crates/docs-check` sits outside all of this. It has no runtime code and nothing depends on it; it
is a workspace member of its own, never published and never in a release archive, because its
checks read the whole repository (this file, every README, the book, the examples) and so belong
to no one crate. The tests step runs it with everything else.

## Conventions

- **Every module is a capability.** `path` needs no authority, `fs` does, so they are separate
  modules rather than one convenient namespace. A change that makes a module need a grant it did not
  need before is a design decision, not a detail.
- **Enforcement lives in Rust, never in Lua.** The grant is checked inside the host function before
  the operation. Lua holds strings, never handles.
- **A module is always present, even ungranted.** It refuses each call with a message naming what
  *was* granted. Absence would make `if airsstack.fs then` mean "am I allowed" instead of "does this
  runtime have it".
- **Determinism is a correctness property.** Sorted keys, sorted directory listings, C-locale byte
  ordering; `os.setlocale` is withheld below `trusted` because `strcoll` would silently change every
  subsequent `table.sort`.
- **A resource breach is not a script failure.** `Error::MemoryLimit` / `Error::InstructionLimit`
  are classified structurally (the engine's counter and the VM error chain — never message text),
  and the CLI reports a breach even under `--fail-open`.

### Code style the lints enforce

Workspace lints: `unsafe_code = forbid`, `clippy::pedantic` + `nursery` warn, `unwrap_used` and
`panic` deny, `missing_docs`/`missing_errors_doc`/`missing_panics_doc` warn. Consequences:

- No `unwrap`/`expect`/`panic!` in library code. Test modules opt out at the top with
  `#![expect(clippy::unwrap_used, reason = "...")]` — every `expect` attribute carries a `reason`.
- Every public item is documented, and every fallible function has an `# Errors` section naming the
  variant it returns.
- **Module doc comments follow a fixed shape**: what it is, *why it exists as its own module*, then
  a `Responsibilities:` list and a `Non-responsibilities:` line. Match this when adding a file.
- Tests live inline in `#[cfg(test)] mod tests` (there are no `tests/` directories), and test names
  are sentences describing the behaviour — `confined_restricts_the_surface_and_imposes_both_ceilings`,
  not `test_confined`.
- Comments explain *why*, at the level of the decision, not what the line does. The existing
  comments in `Cargo.toml`, `Makefile.toml` and `.github/workflows/ci.yml` are the standard.

### Documentation

User documentation is **The airsl Book**, an mdBook in `book/` published to
<https://airsstack.github.io/airsl/> by `.github/workflows/pages.yml`, alongside the `install/`
scripts. `cargo make book` serves it locally; `book/book/` is build output and is ignored. Reference
is the rustdoc, not the book.

The book is written for a reader using `airsl`, not for one maintaining it:

- Plain, short sentences that tell the reader what to do and what happens. It describes what the
  code does now; no `file:line` citations, no implemented/proposed status, no history of how a
  decision was reached, no internal vocabulary.
- Each chapter carries its own small, self-contained example. Every Lua snippet, CLI transcript and
  Rust example is run against the current checkout before it lands, and output is pasted, not
  written from memory.
- A fact lives in one chapter; other chapters link to it rather than restate it.

The three READMEs (root, `crates/airsl`, `crates/airsl-cli`) are short front doors into the book.
Crate READMEs render on crates.io, so they link to the book with absolute
`https://airsstack.github.io/airsl/…` URLs, and their Rust examples are complete programs rather
than rustdoc snippets with hidden `#` lines. They are not doctested, so compile any example you
change in them.

Quoted measurements are a snapshot from `cargo bench -p airsl` on one machine, not a guarantee.

Commits follow Conventional Commits with a scope naming the affected area (`fix(ci):`,
`chore(workspace):`, `docs:`).

## Working in this repository with Claude

- The main thread is for discussion with the author and progress reports only. Every job goes to a
  subagent, however small: reading or searching files, probes, builds, gate runs, edits, status
  flips, and verification. Batch independent jobs into parallel subagents.
- The main thread reports only the outcome of each job, never the job's raw output.
- Keep the author's machine responsive: cargo runs with `CARGO_BUILD_JOBS=2`, builds run one at a
  time, at most one Docker container runs at once, and no emulated-architecture builds run
  locally (use a CI runner for another architecture).
