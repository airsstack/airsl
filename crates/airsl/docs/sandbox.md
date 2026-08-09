# Sandbox and capability policy

**Status: partly implemented.** The language surface and the resource ceilings ship, with three
presets over them. Parameterised grants do not — that axis is typed and empty. Each section below
says which it is.

## What a sandbox is for, here

Not "stop the script doing damage" in the abstract. The goal is narrower and more useful: **the host
decides what a script may reach, per script, and can prove it afterwards.** That framing is what
makes the same mechanism serve first-party plugin scripts (which want nearly everything) and
third-party extensions (which want a named, bounded slice).

## The three axes

A `Policy` composes three independent questions. They were previously collapsed into a single
two-value switch, which made the interesting combinations unrepresentable — a full language surface
with a tight memory ceiling had no way to be said, and the third axis did not exist at all.

| Axis | Question | State |
|---|---|---|
| Language surface | which of Lua's own libraries does the script see? | **ships** |
| Capability grants | which host modules, and what may each one touch? | **typed and empty** |
| Resource ceilings | how much memory and execution may it consume? | **ships** |

The grant axis is the one that matters for extensions, because the interesting question is never
"may this extension touch files" — it is "may this extension touch *these* files". It currently
carries two states, unrestricted and declared-and-empty, because the presets need that much and no
more until a module exists that takes a grant.

## What ships

`LanguageSurface` has three variants, and the globals each withholds are listed beside it in
`sandbox/language_surface.rs`:

| Variant | Libraries | Withheld afterwards |
|---|---|---|
| `Full` | everything except `debug` | nothing |
| `Restricted` | `string`, `table`, `math`, `coroutine`, `os` | the four chunk loaders; `os.execute`, `exit`, `getenv`, `remove`, `rename`, `tmpname`, `setlocale` |
| `Minimal` | `string`, `table`, `math` | the four chunk loaders |

`os.setlocale` is withheld for a subtler reason than the rest, and it is the clearest statement of
this crate's values: Lua compares strings with `strcoll`, so a script that changes the locale changes
the sort order of every subsequent `table.sort`. It is withheld not because it is dangerous but
because it silently destroys determinism. `Minimal` drops `os` entirely for the same kind of reason
rather than a security one — `os.time` and `os.clock` are the last things a script can reach without
a host module that differ between runs.

## The grant model, once modules need it

The shipped shape is presets plus withers, because a policy has no field that must be chosen once
the presets exist:

```rust
Policy::confined()
    .with_limits(ResourceLimits::none().with_instructions(Some(InstructionLimit::count(1_000))))
```

Grants will join it the same way, and the type is already in place so that adding them is not a
change any caller has to see:

```rust
Policy::confined()
    .with_grants(GrantSet::declared()
        .allow(Fs::read("/etc/app").write("/var/app/state"))
        .allow(Proc::allow(["git"]))
        .allow(Env::read(["HOME", "AIRSSTACK_HOME"])))
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

Most callers should not hand-assemble a policy. All three ship.

| Preset | Language surface | Grants | Ceilings | `require` | Intended for |
|---|---|---|---|---|---|
| `trusted` | full Lua stdlib | unrestricted | none | Lua's own | first-party code — the airsstack plugin scripts |
| `confined` *(default)* | restricted + host modules | declared | 64 MiB, 100M | confined to the script directory | third-party extensions |
| `pure` | minimal, no I/O at all | declared | 16 MiB, 10M | none | config evaluation, expressions, generated snippets |

`pure` was worth building even though nothing needs it yet: it is the configuration where the
guarantees are strongest and easiest to state, which makes it the right target for the first
adversarial tests. It gets no `require` at all for exactly that reason — a loader that opens files
would contradict the one preset whose promise is "no I/O".

## Resource ceilings

Both ship, armed on the state before any host module is installed, so no caller can hold an engine
whose ceilings are not yet in force.

| Primitive | Location in `mlua-0.12.0` | Used |
|---|---|---|
| `Lua::set_memory_limit` | `src/state.rs:1104` | yes |
| `Lua::set_global_hook` + `HookTriggers::every_nth_instruction` | `src/state.rs:706`, `src/debug.rs:288` | yes |
| `Lua::sandbox(bool)` — environment save/restore | `src/state.rs:673` | **unavailable** — `#[cfg(any(feature = "luau", doc))]` |

The memory ceiling turns an allocation past the cap into a catchable error rather than an OOM that
takes the host process with it. The instruction ceiling is the only defence against
`while true do end`; without it a runaway script hangs the host, and no capability grant helps.

**The hook must be global, not per-thread.** `Lua::set_hook` installs a hook on the current thread
only, and a coroutine body runs on a thread Lua creates for itself — so with a thread hook,
`coroutine.create(function() while true do end end)` escapes the ceiling entirely and `resume` never
returns. Verified by substituting one for the other, at which point the coroutine test stops failing
and starts hanging. `Restricted` includes `coroutine`, so this was a live hole rather than a
theoretical one.

Two limits on what the ceilings mean, both worth stating precisely:

**The memory ceiling caps the state, not the script.** An engine that has run several scripts carries
whatever garbage they left until the collector runs, and that counts against the next one. Collecting
before every evaluation would fix it and would tax the hot path that makes engine reuse worth having,
so it is documented instead.

**The instruction ceiling is enforced to within a check interval.** The hook fires every ten thousand
instructions; a tighter interval buys precision nobody needs at a price every script pays.

Neither ceiling is free. Arming them costs roughly a quarter of the eval path — see the measurements
in [README.md](README.md). It is a policy choice, and `ResourceLimits::none()` lifts it.

A breach is reported as its own error rather than as a script failure, classified from the engine's
own counter and the VM error chain rather than from message text — a script can raise a string that
reads exactly like either report and should not be able to disguise one as the other. The CLI acts on
the distinction: a breach reaches stderr even under `--fail-open`, where an ordinary failure does not.

The last row of the table matters for engine reuse: because Luau's one-call environment restore is
unavailable, isolating successive scripts that share one `Engine` has to be built rather than
borrowed. What the crate does do per evaluation is reset the instruction counter, rewrite the `arg`
table, and re-point `require` at the current script's root — each of which would be invisible in a
one-script-per-process CLI and a bug in a dispatcher.

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

`airsl doctor` reports the resolved policy in full — language surface, root table, grants, and both
ceilings — for whichever preset it is asked about. For an extension system that is a requirement: a
manifest states what was *requested*, and only the host knows what was *granted*.

What makes the report trustworthy is that there is no longer a way around it. `Engine::lua` used to
hand out the VM itself, so any holder of an engine could install anything into it — including
putting back a library the policy had just withheld — and the report described the policy rather
than the surface. It is gone; contributions go through `HostModule`, which the report can see.

## Open questions

- **Isolation between successive scripts on a reused engine.** Still open, still needed for the
  dispatch path, and `Lua::sandbox` is still unavailable to implement it cheaply. Nothing needs it
  until registered extensions exist.
- **Grant granularity for `proc`.** An allowlist of executable names is easy to state and easy to
  defeat via a wrapper script. Whether that matters depends on whether `fs` write grants can reach
  anywhere on `PATH`.
- **`utf8` below `trusted`.** Absent, originally by accident rather than decision. It needs no
  authority and hazards no determinism, so the case for adding it is strong; it is held only because
  adding it widens the surface. Decide with the standard library.

Settled since this document was first written: **the CLI default is `confined`**, which is the
previous default's language surface plus ceilings — so it is strictly safer and behaviourally
identical for any script that terminates.

## See also

- [architecture.md](architecture.md) — where policy sits among the three layers.
- [stdlib.md](stdlib.md) — which modules need grants and which do not.
- [extensions.md](extensions.md) — how an extension requests capabilities, and how the host bounds
  the request.
