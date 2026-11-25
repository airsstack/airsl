---
status: done
created: 2026-11-24
---

# Intent: The `pure` preset keeps the promise its name makes

## Problem

Tracked as [airsstack/airsl#11](https://github.com/airsstack/airsl/issues/11),
[#12](https://github.com/airsstack/airsl/issues/12),
[#13](https://github.com/airsstack/airsl/issues/13) and
[#14](https://github.com/airsstack/airsl/issues/14).

`Policy::pure()` is the preset for config files, expressions and generated snippets. The name and
the book (`ch03-05-presets.md`) promise a pure computation: "no I/O at all", "cannot even read the
clock", and "given the same input, a pure script always produces the same output". None of the
three holds today.

`pure` is enforced on two of airsl's three layers only. It selects the `Minimal` language surface
and grants nothing, but `stdlib()` installs all twelve host modules under every policy, and four of
them act without consulting any grant. On top of that, two Lua built-ins on the `Minimal` surface
reach the outside or vary per run.

Reproduced on airsl 0.1.5 (checkout `62d2572`), three runs of `airsl run --policy pure`; raw output
in `probes/pure-reach.out`, `probes/globals.out`, `probes/warn.out`:

| Reached under `pure` | Result | Varies per run | Issue |
|---|---|---|---|
| `print` | writes the host's stdout | no | #13 |
| `airsstack.stdio.*` | reads stdin, writes stdout/stderr, no grant | no | #11 |
| `airsstack.hook.*` | reads stdin, writes stdout, no grant | no | #11 |
| `airsstack.time.now` / `monotonic` | reads the clock | yes | #11 |
| `airsstack.path.absolute` | returns the process cwd | per host | #14 |
| `math.random` | `859977` / `176763` / `840843` | yes | none yet |
| `pairs` over string keys | a different order each run | yes | none yet |
| `tostring({})` | a memory address | yes | none yet |

Already refused, no change needed: `hash.hash_file`, `glob.walk`, `env.get`, `fs.*`, `proc.*` (no
grant). `warn` is loaded but writes nothing (`probes/warn.out`).

The opposite gap is #12. A host cannot give a `pure` script a way to load its own sibling files.
`require` exists only on the `Restricted` surface (`confined`), and the `Minimal` surface never
has it, even when the script has a folder. Reproduced on airsl 0.1.6 (checkout `f281887`); raw
output in `probes/require.out`:

| Policy | `type(require)` | `require "require-sib"` |
|---|---|---|
| `pure` | `nil` | `attempt to call a nil value` |
| `confined` | `function` | `42` |

Switching to `confined` to get `require` is not a way out. That surface also loads `os.time` and
`os.clock` (`probes/globals.out`), which are the clock reads `pure` must not have. A multi-file
script under `pure` is impossible today.

Why it matters: the embedders that use `pure` (agentstacks tool-extension adapters, buildl build
files) each hand-build a module set and strip `print` themselves to get what the name promises.
buildl also re-implemented its own root-bound `require`, and agentstacks needs the same for
multi-file adapters. Every embedder repeats that work and can get it wrong, and an embedder that
trusts the name gets stray output on a stdout it uses for its own protocol.

Why it was not caught: the tests check `pure` field by field (surface, limits). No test runs a
script under `pure` and asserts what it can and cannot reach.

Users also have no single place that says which Lua functions each surface loads. Appendix C lists
libraries, not functions, and says nothing about `print` or `math.random`.

## Affected systems

- `crates/airsl/src/sandbox/`: `language_surface.rs` (what `Minimal` withholds), `grant_set.rs` /
  `grants.rs` (any new grant), `policy.rs` (the presets' grant sets).
- `crates/airsl/src/builder.rs`: globals withheld after the state is built.
- `crates/airsl/src/require_loader.rs` (`RequireDisposition::decide`, which today gives `Minimal`
  no `require`) and `crates/airsl/src/engine.rs` (where that decision is applied per script).
- `crates/airsl/src/modules/`: `stdio.rs`, `hook.rs`, `time.rs`, `path.rs`.
- `crates/airsl-cli`: `doctor` output, and any CLI flag that grants what a host module now checks.
- The airsl Book: `ch03-05-presets.md`, `appendix-03-presets.md`, `ch09-03-what-it-promises.md`,
  the chapters for `stdio`, `hook`, `time`, `path` (`ch05-*`), and the chapters that describe
  `require` (`ch02-00-first-project.md`, `ch06-02-scripts-and-values.md`).
- Release: `airsl` and `airsl-cli`, 0.1.6 at the time of writing; this ships as 0.2.0.
- Downstream: agentstacks (`Policy::pure()` with `json`, `ext` and its own module) and buildl
  (`Minimal` surface, curated modules, `print` removed by hand, its own root-bound `require`).
  Both bump to 0.2 in their own repos.

## Desired outcome

- A script under `pure` cannot reach stdin, stdout, stderr, the clock or the process cwd, through
  a Lua built-in or a host module.
- Every source of run-to-run variation that airsl controls is gone under `pure`: the same script and
  input give the same output, apart from what the Lua VM itself randomizes (`pairs` order over
  non-sequence keys, the addresses `tostring` prints), which the book names as not guaranteed.
- `trusted` and `confined` behave as they do today. A script that runs under them on 0.1.6 runs the
  same on 0.2.0.
- A host can still give a `pure`-based policy one of these capabilities explicitly, and
  `airsl doctor` shows what it was given.
- A host can also give a `pure`-based policy a `require` that loads only from the script's own
  folder, without widening the language surface and without the clock coming along. `airsl doctor`
  shows it was given. Plain `pure` still has no `require`.
- A refusal names what was missing, as every other refusal does.
- A test runs a script under `pure` and asserts every route above is closed. A test runs a
  multi-file script under `pure` with `require` granted, and asserts it cannot load a file outside
  its folder.
- The book states exactly what `pure` guarantees, and gains a reference that lists, function by
  function, which Lua built-ins and standard-library functions each surface loads and which preset
  uses which surface. Every table in it is pasted from a run against the release, not written by
  hand.
- `airsl` and `airsl-cli` 0.2.0 are published together.

Candidate direction (for `design`, not settled): withhold `print` and fix or withhold
`math.random` on the `Minimal` surface; add grants for stdio, the clock and the cwd that
`trusted` and `confined` hold and `pure` does not, checked inside the host modules. Make the
confined `require` a host grant on the policy as well, rather than something only the `Restricted`
surface carries.

## Constraints

- **The three layers stay separate** (`CLAUDE.md`, `book/src/ch09-01-three-layers.md`). The
  surface decides which Lua built-ins exist; grants decide what host modules may do; the module
  set stays independent of the policy.
- **A module is always present, even ungranted**, and refuses each call naming what was granted.
  Under `pure`, `airsstack.stdio` is still a table.
- **Nothing is ambient**: a capability a `pure` script can use is one the host granted and
  `doctor` can show. That includes `require`. #12 suggests an opt-in on the builder or on the
  `Script`, but that would sit outside the policy `doctor` reports, and this constraint rules it out.
- **Enforcement lives in Rust**, inside the host function, before the operation.
- A resource breach stays classified structurally, unaffected by this change.
- Breaking for scripts is accepted: a script that used `print`, `math.random`, `stdio`, `hook`,
  `time.now`/`monotonic` or `path.absolute` under `pure` is refused on 0.2.0. Rust API changes stay
  additive where possible (`GrantSet` and `Error` are `#[non_exhaustive]`).
- `airsl` and `airsl-cli` publish together (org rule: the binary's lockfile pins the library).
- Every claim about mlua or Lua behaviour is proven with a probe against mlua 0.12 (the version in
  `Cargo.lock`), the raw output saved in this chain.

## Non-goals

- Changing `trusted` or `confined` behaviour.
- Making `pairs` order or `tostring` addresses deterministic. Both come from the Lua VM's hash seed
  and allocator; they are documented, not fixed.
- A policy-dependent module set (`stdlib_for(&Policy)`), which would merge the layers.
- Bumping agentstacks or buildl. That happens in their repos after 0.2.0 is published.
