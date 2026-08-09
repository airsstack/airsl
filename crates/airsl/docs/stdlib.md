# Host standard library

**Status: `json` is implemented. Everything else on this page is proposed.**

Everything a script can reach arrives under one Lua global, `airsstack`, as subtables installed from
Rust. This document is the roster, the reasoning, and the rules every module follows.

## Why a host stdlib at all

The answer that sounds right and is wrong: "because the sandbox removes Lua's own libraries." Under
`--unrestricted`, `io.open`, `os.getenv`, `io.popen` and `require` all work today, so a script can
already do these things.

The real reason is that **Lua's own standard library is thin, and the parts it does have are the
wrong shape for a host that cares about determinism.** Lua has no JSON, no regular expressions (its
patterns have no alternation and no `\b`), no hashing, no directory listing, no `stat`, no path
manipulation, and no way to run a process without going through a shell. A script written against
raw Lua ends up shelling out to `python3` — which is the exact situation this crate exists to
remove.

So the modules are not sandbox workarounds. They are better APIs, and being grantable is a second
benefit rather than the motivation.

## Principles

**Every module is a capability.** This is why `path` and `fs` are separate: path manipulation is
pure string math and needs no authority, file access needs a grant. A function that would force a
grant onto a previously-pure module is a design change, not a detail.

**Deterministic by default.** Sorted JSON keys, sorted directory listings, C-locale byte ordering,
stable iteration. Non-determinism here surfaces as spurious diffs and irreproducible builds rather
than as errors, which makes it expensive to find later.

**No shell, ever.** `proc.run` takes an argv array. There is no string form, so quoting bugs are
unrepresentable rather than merely discouraged. `io.popen` takes a shell string, and that is the
single strongest reason to prefer `proc` over it even under `Trusted`.

**Errors are catchable Lua errors,** not sentinel return values, so `pcall` is the one handling
story a script author has to learn.

**Grants are checked in Rust, before the operation.** See [sandbox.md](sandbox.md).

## What ships: `airsstack.json`

`encode`, `encode_pretty`, `decode`. Two known gaps, both worth fixing before other consumers build
on it:

**Object key order is Lua hash order and varies between runs.** `convert.rs:34` streams an
`mlua::Value` straight into `serde_json`, so nothing sorts. Four consecutive runs of one script
encoding the same table:

```
{"kappa":1,"alpha":1,"beta":1,"gamma":1,"zeta":1,"omega":1,"mid":1,"delta":1}
{"omega":1,"gamma":1,"zeta":1,"alpha":1,"delta":1,"mid":1,"kappa":1,"beta":1}
{"delta":1,"beta":1,"zeta":1,"alpha":1,"omega":1,"mid":1,"gamma":1,"kappa":1}
{"zeta":1,"kappa":1,"delta":1,"beta":1,"alpha":1,"omega":1,"mid":1,"gamma":1}
```

Any consumer writing an index, a lockfile or a cached artifact needs a sorted-key mode.

**JSON `null` does not round-trip.** `convert.rs:56-58` documents it: `null` decodes to Lua `nil`,
which is indistinguishable from an absent key, so `{"a": null}` and `{}` decode identically. A null
sentinel value fixes it. This matters more for extensions than for scripts, because extensions
exchange JSON with the host.

## Tier 1 — the modules the plugin corpus needs

Validated against the 29 production scripts in `plugins/`, which between them exercise filesystem
walking, subprocess capture, environment lookup, regex, glob matching, hashing, time formatting and
JSON round-trips against real data. That corpus is the acceptance test for this tier, not its
specification: each module is designed for the general case.

| Module | Surface | Grant | Backing crate |
|---|---|---|---|
| `path` | `join`, `dirname`, `basename`, `stem`, `ext`, `normalize`, `relative_to`, `is_absolute`, `absolute` | none | std |
| `fs` | `read`, `read_lines`, `write`, `append`, `exists`, `is_file`, `is_dir`, `stat`, `list`, `walk`, `mkdir`, `remove`, `remove_dir`, `copy`, `rename`, `canonicalize`, `tempfile`, `tempdir`, `atomic_write`, `create_exclusive`, `same_content` | read roots, write roots | `walkdir`, `tempfile` |
| `env` | `get`, `all`, `set` | name allowlist | std |
| `proc` | `run(argv) -> {stdout, stderr, status}`, `which` | executable allowlist | std |
| `regex` | `compile`, `is_match`, `find`, `find_all`, `captures`, `replace`, `replace_all`, `split` | none | `regex` |
| `hash` | `sha1`, `sha256`, `hash_file`, hex encoding | none | `sha2`, plus a SHA-1 crate |
| `time` | `now`, `monotonic`, `format`, `parse` | none | `jiff` |
| `glob` | `match(pattern, path)`, `walk(root, pattern)` | inherits `fs` | `globset` |

Six of those eight backing crates — `regex`, `globset`, `walkdir`, `sha2`, `jiff`, `tempfile` — are
already declared in `Cargo.toml:48-58` and currently unused. The roster is largely what the commit
that introduced this crate anticipated.

### Four requirements that are easy to miss

**`env` needs an allowlist, not just a read grant.** The environment routinely carries credentials.
An extension granted "read env" should see the names it declared, not everything the host process
inherited.

**`fs.create_exclusive` is a concurrency primitive, not a convenience.** It is `O_CREAT|O_EXCL` — an
atomic claim. `plugins/airsstack/hooks/enforce.py:321-335` relies on it for a sentinel claim, and its
comment records that the previous read-then-append design let 3 of 4 concurrent hooks all fire.
Without this function that hook cannot be ported correctly, only approximately.

**`hash` needs SHA-1, not only SHA-256.** `enforce.py:109` uses `hashlib.sha1(...)[:8]` and
`plugins/airsstack-sdd/hooks/ensure-layout.sh` uses `shasum | cut -c1-8`, which is also SHA-1. These
produce the per-repository project key that names the HOME-global SDD spec and plan directories and
the snapshot store. Shipping only SHA-256 silently re-keys every project and orphans existing
artifacts — invisibly, until someone cannot find last week's plan. SHA-256 should be the default for
new uses; SHA-1 exists for compatibility and should be documented as such.

**`glob`'s `**/` must match zero or more segments.** `enforce.py:36-38` makes `**/Cargo.toml` match a
root-level `Cargo.toml`, which is this repository's most important Rust file. Whether `globset`
agrees needs checking against that case specifically rather than assuming.

## Tier 2 — runtime-class

| Module | Why |
|---|---|
| `stdio` | read stdin, write stdout/stderr, `isatty`. `Restricted` has no `io` at all, and every plugin hook receives its payload on stdin |
| `hook` | the agent-hook contract: parse the payload, emit `hookSpecificOutput`. A thin layer over `stdio` + `json`, already named in `convert.rs:4` |
| `test` | not a module but a runner — `airsl test`. See below |

`airsl test` deserves emphasis. The plugin suite has 23 test files, and neither `cargo make dod` nor
`.github/workflows/ci.yml` executes any of them — they run under `sh` and `python3` by hand. Porting
6,285 lines of script onto a new runtime without a test story is how a migration becomes a rewrite
with unknown behaviour. A runner also happens to be one of the things a batteries-included runtime is
expected to ship.

## Tier 3 — later, each a real project

`http` (settle the async and `Send`/`Sync` questions first — see
[architecture.md](architecture.md)), `sqlite`, `crypto` beyond hashing, and bundling a script plus
its dependencies into a single distributable.

## What is deliberately absent

`fs`, `env` and `proc` are on this list even though `io.open`, `os.getenv` and `io.popen` already
work under `Trusted`. That is intentional: the Lua originals cannot be granted, cannot be confined,
and in `io.popen`'s case take a shell string. A script that must run under `Confined` has no
alternative to the host module.

A `text` module is deliberately *not* proposed. Lua's `string` library is adequate, and a module that
duplicates it would add surface without adding capability.

## Sequencing

`path` first: no grants, no I/O, pure functions. It proves the module shape, the error convention and
the test harness at the lowest possible cost.

Then `fs` and `env`, which unblock most of the plugin corpus and are where the grant machinery gets
designed against something real. The `Policy` model should be settled before `fs`, because `fs` is
the first module whose signature depends on it.

Then `proc`, `regex`, `hash`, `glob`, then `stdio` and `hook` to finish the migration. `airsl test`
should land early enough to test the modules that follow it rather than last.

## See also

- [architecture.md](architecture.md) — the three layers and where modules sit.
- [sandbox.md](sandbox.md) — what a grant is and where it is enforced.
- [extensions.md](extensions.md) — how a manifest names these capabilities.
