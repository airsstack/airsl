# Sandbox and capability policy

**Status: proposed.** What ships today is the two-value `Sandbox` enum described under "What exists"
below. Everything else here is design.

## What a sandbox is for, here

Not "stop the script doing damage" in the abstract. The goal is narrower and more useful: **the host
decides what a script may reach, per script, and can prove it afterwards.** That framing is what
makes the same mechanism serve first-party plugin scripts (which want nearly everything) and
third-party extensions (which want a named, bounded slice).

## What exists

`Sandbox::Restricted` selects `STRING | TABLE | MATH | COROUTINE | OS` (`builder.rs:100-105`), then
strips the chunk loaders `load`, `loadstring`, `dofile`, `loadfile` (`builder.rs:32`) and the `os`
functions that reach outside the process — `execute`, `exit`, `getenv`, `remove`, `rename`,
`tmpname`, `setlocale` (`builder.rs:40-48`).

`Sandbox::Full` selects `StdLib::ALL_SAFE`: everything except `debug`, including `io`, `os`,
`package` and `require`.

`os.setlocale` is withheld for a subtler reason than the rest, and it is the clearest existing
statement of this crate's values: Lua compares strings with `strcoll`, so a script that changes the
locale changes the sort order of every subsequent `table.sort`. It is withheld not because it is
dangerous but because it silently destroys determinism.

## The problem

One switch expresses three independent things:

| Axis | Question | Expressed today? |
|---|---|---|
| Language surface | which of Lua's own libraries does the script see? | yes — the enum |
| Capability grants | which host modules, and what may each one touch? | crudely — a module is present or absent |
| Resource limits | how much memory and execution may it consume? | no |

For first-party scripts that is adequate. For extensions it is not, because the interesting question
is never "may this extension touch files" — it is "may this extension touch *these* files".

## The proposed model

A `Policy` composing all three axes:

```rust
Policy::builder()
    .language(Language::Minimal)                    // string, table, math — no io/os/package
    .grant(Fs::read("/etc/app").write("/var/app/state"))
    .grant(Proc::allow(["git"]))
    .grant(Env::read(["HOME", "AIRSSTACK_HOME"]))
    .memory_limit(64 * MiB)
    .instruction_limit(10_000_000)
    .build()
```

Two properties carry the design.

**Grants are parameterised, not boolean.** `Fs::read("/var/app")` is a different authority from
`Fs::read("/")`, and the policy has to be able to say which. A boolean grant vocabulary cannot
express a confined extension, which makes it useless for the case the system exists to serve.

**Enforcement lives in the Rust function.** A path-confined `fs.read` canonicalises its argument and
checks containment *before* opening anything. Lua never holds a file handle — it holds a string and
calls in — so there is nothing to reach around. This discipline is already present in the plugin
suite it will replace: `cache_sync.is_within` in
`plugins/airsstack-plugin-dev/hooks/cache_sync.py:66` is exactly this check, enforced in the wrong
language.

The corollary is that **a capability is only as good as the module that implements it**. A grant is a
promise the host module keeps. There is no VM-level backstop if it does not.

## Presets

Most callers should not hand-assemble a policy.

| Preset | Language surface | Grants | Intended for |
|---|---|---|---|
| `Trusted` | full Lua stdlib | unrestricted | first-party code — the airsstack plugin scripts |
| `Confined` | minimal + host modules only | declared roots only | third-party extensions |
| `Pure` | minimal, no I/O modules at all | none | config evaluation, expressions, generated snippets |

`Pure` is worth building even though nothing needs it yet: it is the configuration where the
guarantees are strongest and easiest to state, which makes it the right target for the first
adversarial tests.

## Resource limits

`mlua` 0.12 provides the primitives on Lua 5.4, and `airsl` currently uses none of them:

| Primitive | Location in `mlua-0.12.0` | Available on Lua 5.4? |
|---|---|---|
| `Lua::set_memory_limit`, `Lua::used_memory` | `src/state.rs:1104`, `src/state.rs:1081` | yes — not feature-gated |
| `Lua::set_hook` + `HookTriggers::every_nth_instruction` | `src/state.rs:756`, `src/debug.rs:343` | yes |
| `Lua::gc_collect` | `src/state.rs:1153` | yes |
| `Lua::sandbox(bool)` — environment save/restore | `src/state.rs:675` | **no** — `#[cfg(any(feature = "luau", doc))]` |

Memory limits turn an allocation past the cap into a catchable `Error::MemoryError` rather than an
OOM that takes the host process with it. The instruction hook is the only defence against
`while true do end`; without it a runaway script hangs the host, and no capability grant helps.

The last row matters for engine reuse: because Luau's one-call environment restore is unavailable,
isolating successive scripts that share one `Engine` has to be built rather than borrowed.

## What this can and cannot promise

Stating this precisely is a requirement, not a caveat, because `Confined` will otherwise be mistaken
for a security perimeter.

**It provides** capability isolation at the API boundary; memory and execution ceilings; a surface
the host fully enumerates; and microsecond startup with no toolchain for extension authors.

**It does not provide** memory isolation from the host process. A bug in a host module — or in Lua's
own C code — is a host compromise, not a trapped fault. There is no protection against a *malicious
host module*, and no CPU isolation between scripts.

### Against WASM

WASM isolates at the VM boundary and would survive a buggy host function. Lua isolates at the API
boundary and would not. WASM also lets extension authors choose their language, at the cost of a
toolchain, a heavier runtime, and a much more awkward data boundary.

For extensions you write, review, or accept from a marketplace you control, Lua is the right trade
and Redis is the precedent. For executing genuinely hostile third-party code, WASM is stronger, and
these documents should not be read as claiming otherwise.

## Auditability

`airsl doctor` reports the sandbox and the installed modules today. Under this model it should
report the resolved policy in full — every grant, every root, every limit — so an installed
extension's actual authority is inspectable rather than inferred from its manifest. For an extension
system that is a requirement: a manifest states what was *requested*, and only the host knows what
was *granted*.

## Open questions

- **Default for the CLI.** `Confined` is safe but will surprise anyone running a first-party script;
  `Trusted` is convenient but unguarded. A third option is to make the default depend on
  provenance — scripts inside a known root are trusted, others are not.
- **Isolation between successive scripts on a reused engine.** Needed for the dispatch path, and
  `Lua::sandbox` is unavailable to implement it cheaply.
- **Grant granularity for `proc`.** An allowlist of executable names is easy to state and easy to
  defeat via a wrapper script. Whether that matters depends on whether `fs` write grants can reach
  anywhere on `PATH`.

## See also

- [architecture.md](architecture.md) — where policy sits among the three layers.
- [stdlib.md](stdlib.md) — which modules need grants and which do not.
- [extensions.md](extensions.md) — how an extension requests capabilities, and how the host bounds
  the request.
