# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`airsl` embeds Lua 5.4 in Rust and lets the **host** decide what a script may reach. Two crates in
one workspace: `crates/airsl` (the library) and `crates/airsl-cli` (the `airsl` binary — `run`,
`test`, `check`, `doctor`).

Build requirements that are not negotiable: **unix only** (`lib.rs` has a `compile_error!` off unix,
because `modules::proc` decides executability from mode bits) and **a C compiler** (`mlua`'s
`vendored` feature compiles Lua 5.4 from C source and links it statically — no system Lua, no
`pkg-config`).

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

Three independent layers; collapsing them is the most common mistake. `crates/airsl/docs/architecture.md`
is the long form.

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
negotiated `Policy`. `ExtensionHost::load` (`extension/host.rs`) is the entry point a host calls;
`Extension::approve` followed by `Approved::start` (`extension/loaded.rs`) is the type-state that
proves a denial ran before any extension code does.

Key seams:

- `Engine::dispatch` (`engine.rs`) is how a host invokes the handlers a script registered with
  `ext.on`. Handlers live in a registry table written by `modules/ext.rs`; the engine records the
  evaluating `ThreadId` so a handler that re-enters its own engine gets `Error::Reentrant`, never a
  deadlock.
- `Engine::builder()` is a **type-state builder** (`builder.rs`, `Missing`/`Present`): there is no
  `build()` until `policy()` has been called, so a sandbox cannot be forgotten.
- `HostModule::install(&mlua::Lua, &mlua::Table, &InstallContext)` is the extension seam. `mlua` is
  re-exported as `airsl::mlua` deliberately — it is part of the contract, and a separately declared
  version produces type errors that never name the real cause. `InstallContext` carries the *engine's*
  policy, so what a module enforces and what `airsl doctor` reports cannot diverge.
- `modules::stdlib()` (`src/modules/stdlib.rs`) is the single list of built-ins; the engine, the
  doctor output, and tests all read it. A new module is registered there.
- `modules/guard.rs` (`PathGuard`) is the *only* place the filesystem containment rule is written.
  Every `fs` call, plus `hash.hash_file` and `glob.walk`, funnels through it.
- `Engine` is `Send + Sync` and holds a lock spanning a whole evaluation (`engine.rs`) — `mlua`'s
  per-operation locking was memory-safe but not correct across the four steps of an eval. A shared
  engine avoids rebuilding state; it never buys parallelism.
- Engine reuse is ~30× cheaper than rebuilding, so it is an API-shape decision. Per-evaluation:
  instruction counter, `arg` table, `require` root. Persisted across evaluations: the `require`
  module cache (keyed by canonical path), Lua globals a script wrote.

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

`crates/airsl/docs/` follows [Diátaxis](https://diataxis.fr/): tutorial, how-to, architecture,
sandbox, stdlib, extensions. Reference is the rustdoc, not a file there.

Evidence rules those documents follow, and that edits to them must keep: a claim about code that
exists carries a `file:line`; a claim about code that does not exist says so explicitly. The status
table in `docs/README.md` marks each area **implemented** or **proposed** — the manifest parser
(`extension/manifest.rs`), ceiling (`extension/ceiling.rs`), negotiation (`extension/negotiate.rs`),
approver (`extension/approver.rs`), event dispatch (`modules/ext.rs`, `Engine::dispatch`), and the
extension host / loader (`ExtensionHost`, `ExtensionHost::load`, `extension/host.rs`) are all
implemented; only the `airsl ext` CLI (`doctor`, `fire`) is proposed and unbuilt. Quoted
measurements are a snapshot from `cargo bench -p airsl` on one machine, not a guarantee.

Commits follow Conventional Commits with a scope naming the affected area (`fix(ci):`,
`chore(workspace):`, `docs:`).
