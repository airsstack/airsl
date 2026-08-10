# Host standard library

**Status: every module on this page is implemented, and so is `airsl test`. The plugin corpus
this tier was designed against now runs on it. What remains proposed is Tier 3 and the JSON
value constructors.**

Everything a script can reach arrives under one Lua global, `airsstack`, as subtables installed from
Rust. This document is the roster, the reasoning, and the rules every module follows.

## Why a host stdlib at all

The answer that sounds right and is wrong: "because the sandbox removes Lua's own libraries." Under
`--policy trusted`, `io.open`, `os.getenv`, `io.popen` and `require` all work today, so a script can
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
story a script author has to learn. One function deviates on purpose: `fs.create_exclusive` returns
`false` rather than raising when the file already exists, because losing that race is the expected
*other outcome* of an atomic claim rather than a failure. A new sentinel return needs an argument of
that kind, not a preference.

**A refusal is never an answer.** Asking about something the policy does not grant raises; it does
not return a value that could be mistaken for a fact about the world. `env.get` raises for an
ungranted name rather than reporting `nil`, and `fs.exists` raises for an ungranted path rather than
reporting `false` — otherwise a script cannot tell "you may not ask" from "it is not there", and
will report a missing file or an unset variable when it was actually denied. This costs nothing in
confidentiality: a denial says the path is outside the grant, which the caller's own manifest
already told it, and says nothing about what is there.

**Grants are checked in Rust, before the operation.** See [sandbox.md](sandbox.md).

## JSON: two gaps closed, one narrower than it looked

`encode`, `encode_pretty`, `decode`.

**Object keys sort.** They used to come out in Lua hash order, which varies between runs, so the
same table encoded to a different byte string every time — unusable for an index, a lockfile or any
cached artifact. `convert::sorted` now routes through `Value::to_serializable().sort_keys(true)`
(`mlua-0.12.0/src/value.rs:489` and `:681`), which mlua had all along. Sorting is the behaviour
rather than an option, because a caller could not ask for insertion order anyway — Lua never had it.
Arrays keep their order; only object keys are affected.

**JSON `null` still does not round-trip.** `convert.rs` documents it: `null` decodes to Lua `nil`,
which is indistinguishable from an absent key, so `{"a": null}` and `{}` decode identically. A null
sentinel value fixes it, and the shape of that sentinel is a real API decision — whether `decode`
produces it by default, and how `encode` treats it — which is why it is not simply bolted on ahead
of the module that needs it. It matters more for extensions than for scripts, because extensions
exchange JSON with the host, so it should be settled with `hook`.

Everything else on this page is built. The roster rows below are the shipped surface, not a plan —
they were checked against the live table by enumerating `airsstack` under `--policy pure`.

## Tier 1 — the modules the plugin corpus needs

Validated against the production scripts in `plugins/`, which between them exercise filesystem
walking, subprocess capture, environment lookup, regex, glob matching, hashing, time formatting and
JSON round-trips against real data. That corpus is the acceptance test for this tier, not its
specification: each module is designed for the general case.

It has now been run: the whole suite is Lua, and 244 tests over it run under `airsl test`. Four
things the corpus asked for that the roster did not supply are recorded under
[what the port had to work around](#what-the-port-had-to-work-around).

| Module | Surface | Grant | Backing crate |
|---|---|---|---|
| `path` | `join`, `dirname`, `basename`, `stem`, `ext`, `normalize`, `relative_to`, `is_absolute`, `absolute` | none | std |
| `fs` | `read`, `read_lines`, `write`, `append`, `exists`, `is_file`, `is_dir`, `stat`, `list`, `walk`, `mkdir`, `remove`, `remove_dir`, `copy`, `rename`, `canonicalize`, `tempfile`, `tempdir`, `atomic_write`, `create_exclusive`, `same_content` | read roots, write roots | `walkdir`, `tempfile` |
| `env` | `get`, `all`, `set` | name allowlist | std |
| `proc` | `run(argv) -> {stdout, stderr, status}`, `which` | executable allowlist | std |
| `regex` | `compile`, `is_match`, `find`, `find_all`, `captures`, `replace`, `replace_all`, `split` | none | `regex` |
| `hash` | `sha1`, `sha256`, `hash_file`, hex encoding | none, except `hash_file`, which needs the read grant | `sha2`, `sha1` |
| `time` | `now`, `monotonic`, `format`, `parse` | none | `jiff` |
| `glob` | `match(pattern, path)`, `walk(root, pattern)` | inherits `fs` | `globset` |

All eight are built and every backing crate is now used by the module that named it. `getrandom`
was declared with no module on this roster to consume it and has been removed; `sha1` was added
with `hash`, so the dependency list keeps meaning "something uses this".

Three rows carry a grant, and two more inherit one. `hash_file` and `glob.walk` read the filesystem,
so they go through the same guard `fs` does and need the same read grants — "inherits `fs`" made
concrete rather than left as a note.

### Four requirements that were easy to miss, and how each landed

**`env` needs an allowlist, not just a read grant.** The environment routinely carries credentials.
An extension granted "read env" should see the names it declared, not everything the host process
inherited. `env.all` returns only granted names for exactly this reason.

`env.set` writes to a per-process overlay rather than to the real environment, and this is worth
knowing before reading the roster row as "sets the variable". Two reasons: `std::env::set_var` is
`unsafe` in Edition 2024 because it races every other thread reading the environment, and this crate
forbids `unsafe`; and a sandboxed script silently changing the *host's* environment is not a
capability anyone meant to grant. `env.get`, `env.all` and `proc.run` all read the overlay first, so
from inside Lua the behaviour is what a script expects — what does not change is what the host
process itself sees.

**`fs.create_exclusive` is a concurrency primitive, not a convenience.** It is `O_CREAT|O_EXCL` — an
atomic claim. `plugins/airsstack/hooks/lib/enforce.lua:339-347` relies on it for a sentinel claim,
and its comment records that the previous read-then-append design let 3 of 4 concurrent hooks all
fire. Without this function that hook could only have been ported approximately.

**`hash` needs SHA-1, not only SHA-256.** `plugins/airsstack/hooks/lib/enforce.lua:95` and
`plugins/airsstack-sdd/hooks/lib/layout.lua:80` both take `sha1(path)[:8]`, replacing a
`shasum | cut -c1-8` pipeline that was also SHA-1. These produce the per-repository project key
that names the HOME-global SDD spec and plan directories and the snapshot store. Shipping only
SHA-256 would silently re-key every project and orphan existing artifacts — invisibly, until
someone cannot find last week's plan. SHA-256 is the default for new uses; SHA-1 exists for
compatibility and is documented as such.

**`glob`'s `**/` must match zero or more segments.** `plugins/airsstack/hooks/lib/globs.lua` makes
`**/Cargo.toml` match a root-level `Cargo.toml`, which is this repository's most important Rust
file. `globset` agrees — checked against that exact case rather than assumed, and pinned by a test
that asserts both the zero-segment and the many-segment match. It did **not** agree about `*`; see
the defect note below.

## Tier 2 — runtime-class

| Module | Why |
|---|---|
| `stdio` | `read`, `lines`, `write`, `error`, `isatty` — read stdin, write stdout/stderr. `Restricted` has no `io` at all, and every plugin hook receives its payload on stdin |
| `hook` | `payload`, `emit`, `context` — the agent-hook contract. A thin layer over `stdio` + `json` |
| `test` | not a module but a runner — `airsl test`. See below |

All three ship. `airsl test` deserves emphasis: the plugin suite has test files that neither
`cargo make dod` nor `.github/workflows/ci.yml` executes — they run under `sh` and `python3` by hand
— and porting several thousand lines of script onto a new runtime without a test story is how a
migration becomes a rewrite with unknown behaviour.

Its conventions are deliberately thin, because each one is something an author has to learn. A test
file is named `*_test.lua` or `test_*.lua`; it returns a table whose named function values are the
tests; a test passes by returning and fails by raising, so Lua's own `assert` is the entire
assertion surface. Each file gets a fresh engine, because sharing one would let a file leave globals
behind for the next — the isolation gap the crate documents, and a test suite is exactly where that
becomes a failure nobody can reproduce alone.

Finding no test files at all exits non-zero. "No tests" and "all tests passed" must not read the
same to CI, which is how a discovery glob that stopped matching goes unnoticed for months.

## Tier 3 — later, each a real project

`http` (settle the async and `Send`/`Sync` questions first — see
[architecture.md](architecture.md)), `sqlite`, `crypto` beyond hashing, and bundling a script plus
its dependencies into a single distributable.

## What is deliberately absent

`fs`, `env` and `proc` are on this list even though `io.open`, `os.getenv` and `io.popen` already
work under `trusted`. That is intentional: the Lua originals cannot be granted, cannot be confined,
and in `io.popen`'s case take a shell string. A script that must run under `confined` has no
alternative to the host module.

A `text` module is deliberately *not* proposed. Lua's `string` library is adequate, and a module that
duplicates it would add surface without adding capability.

## Sequencing

`path` is done: no grants, no I/O, pure functions, and it fixed the module shape, the error
convention and the test harness at the lowest possible cost. Two decisions it settled are worth
carrying forward — a function returns an empty string rather than `nil` for "no extension", so that
absence and failure are never the same value; and `relative_to` refuses a path outside its base
instead of walking up with `..`, because the caller asked "where is this under that", not "how do I
get from one to the other".

Then `fs` and `env`, which unblock most of the plugin corpus and are where the grant machinery gets
designed against something real. The plumbing is in place: `HostModule::install` receives an
`InstallContext` carrying the policy, so a module reads its authority from the same object the
engine reports. What `fs` adds is the vocabulary — the parameterised grant types — plus the answer
to a question `path` never had to face: whether a module the policy has granted nothing is installed
and refuses every call, or is not installed at all so that a script can test for it.

`proc`, `regex`, `hash`, `glob`, `stdio`, `hook` and `airsl test` are all built, and the plugin
corpus is ported. What is left is Tier 3 and the JSON value constructors.

## What the port had to work around

Four gaps the corpus hit that the roster above does not close. None blocked the migration; each
cost a workaround worth naming, because the next consumer will hit the same ones.

| Gap | What the port did instead |
|---|---|
| `proc.run` takes argv only — no working directory, no stdin, no per-call environment | every git call travels through `git -C <dir>`; `CMUX_QUIET=1 cmux …` becomes an `env.set` on the process overlay |
| No exit code but 0 and 1 — `os.exit` is withheld below `Full`, and the CLI maps any failure to 1 | the four scripts documenting `exit 2` for a usage error now exit 1; the stderr message is unchanged |
| No JSON `null` or empty-array constructor | `airsstack.json.decode("[]")` as the empty-array idiom |
| No random source — `getrandom` was removed with no module consuming it | `math.random`, which Lua 5.4 seeds per state, for a session-directory suffix |

The port also found one outright defect, since fixed. **`airsstack.glob`'s `*` used to cross
`/`**, because `matcher` left `literal_separator` off — and said in a comment that it did so
"the way the plugin scripts expect", which was the reverse of the truth. Under it a manifest
declaring `match: ["*.rs"]` also selected `deeply/nested/file.rs`, enforcing a rule over files
its author never named. `*` and `?` now stop at a separator, `**` stays recursive, and two
regression tests pin both halves (`modules/glob.rs`).

The dispatcher still compiles its own globs
(`plugins/airsstack/hooks/lib/globs.lua`) rather than delegating, for a different reason than
before: `globset` accepts a strictly larger grammar than the enforcement manifests were written
against. `*.{lua,rs}` matches here and not there, so delegating would widen matching for any
manifest using braces — and a manifest is a contract with plugin authors outside this
repository.

## See also

- [architecture.md](architecture.md) — the three layers and where modules sit.
- [sandbox.md](sandbox.md) — what a grant is and where it is enforced.
- [extensions.md](extensions.md) — how a manifest names these capabilities.
