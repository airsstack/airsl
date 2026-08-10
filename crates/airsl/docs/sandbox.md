# Sandbox and capability policy

**Status: implemented.** All three axes ship — the language surface, the parameterised grants and
the resource ceilings — with three presets over them.

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
| Capability grants | which host modules, and what may each one touch? | **ships** |
| Resource ceilings | how much memory and execution may it consume? | **ships** |

The grant axis is the one that matters for extensions, because the interesting question is never
"may this extension touch files" — it is "may this extension touch *these* files".

## What ships

`LanguageSurface` has three variants, and the globals each withholds are listed beside it in
`sandbox/language_surface.rs`:

| Variant | Libraries | Withheld afterwards |
|---|---|---|
| `Full` | everything except `debug` | nothing |
| `Restricted` | `string`, `table`, `math`, `utf8`, `coroutine`, `os` | the four chunk loaders; `os.execute`, `exit`, `getenv`, `remove`, `rename`, `tmpname`, `setlocale`; the string metatable |
| `Minimal` | `string`, `table`, `math`, `utf8` | the four chunk loaders; the string metatable |

`os.setlocale` is withheld for a subtler reason than the rest, and it is the clearest statement of
this crate's values: Lua compares strings with `strcoll`, so a script that changes the locale changes
the sort order of every subsequent `table.sort`. It is withheld not because it is dangerous but
because it silently destroys determinism. `Minimal` drops `os` entirely for the same kind of reason
rather than a security one — `os.time` and `os.clock` are the last things a script can reach without
a host module that differ between runs.

## The grant model

The shipped shape is presets plus withers, because a policy has no field that must be chosen once
the presets exist:

```rust
Policy::confined()
    .with_limits(ResourceLimits::none().with_instructions(Some(InstructionLimit::count(1_000))))
```

Grants join it the same way:

```rust
Policy::confined().with_grants(
    GrantSet::declared()
        .with_fs(|fs| fs.read("/etc/app").write("/var/app/state"))
        .with_proc(|proc| proc.allow(["git"]))
        .with_env(|env| env.read(["HOME", "AIRSSTACK_HOME"])),
)
```

and the CLI says the same thing on the command line, which is where a hook launcher's authority
should be readable:

```bash
airsl run --allow-read "$AIRSSTACK_HOME" \
          --allow-write "$AIRSSTACK_HOME/journal/.index" \
          --allow-exec git \
          --allow-env AIRSSTACK_HOME \
          hooks/index.lua
```

**Nothing is granted by default** below `trusted`. That does not block the plugin migration: the
first-party scripts port under `trusted`, which already gives them everything and is documented for
exactly that, and tighten to named grants afterwards. The alternatives both cost something
permanent — an implicit script-directory grant would let an extension rewrite its own code and would
not appear in `airsl doctor`, and granting everything would make `confined` a language-surface
restriction rather than a capability boundary.

Two properties carry the design.

**Grants are parameterised, not boolean.** A read root of `/var/app` is a different authority from a
read root of `/`, and the policy has to be able to say which. A boolean grant vocabulary cannot
express a confined extension, which makes it useless for the case the system exists to serve. Read
and write are separate lists for the same reason: the common shape is a wide read root and a narrow
write root, and one list would force the write authority up to the read authority.

**Enforcement lives in the Rust function.** `fs.read` canonicalises its argument and checks
containment *before* opening anything. Lua never holds a file handle — it holds a string and calls
in — so there is nothing to reach around. This discipline is already present in the plugin suite it
replaces: `is_within` in `plugins/airsstack-plugin-dev/hooks/lib/cache.lua:131` is exactly
this check, enforced in the wrong language.

The check itself is worth stating precisely, because a plausible version of it does not work. It
canonicalises the deepest part of a path that **exists** and accepts only ordinary names below that:
canonicalising resolves symlinks, so a link inside a granted root pointing outside is caught, and
what remains below cannot hide a symlink because it does not exist. A `..` below that point is
refused rather than resolved — with `<root>/link` pointing at `/elsewhere`, the path
`<root>/link/../secret` reads lexically as `<root>/secret`, inside the root, while the kernel opens
`/secret`. The lexical answer is the wrong one, and a normaliser that returns it is a containment
check that contains nothing.

The corollary is that **a capability is only as good as the module that implements it**. A grant is a
promise the host module keeps. There is no VM-level backstop if it does not.

## Presets

Most callers should not hand-assemble a policy. All three ship.

| Preset | Language surface | Grants | Ceilings | `require` | Intended for |
|---|---|---|---|---|---|
| `trusted` | full Lua stdlib | unrestricted | none | Lua's own | first-party code — the airsstack plugin scripts |
| `confined` *(default)* | restricted + host modules | declared, empty until named | 64 MiB, 100M | confined to the script directory | third-party extensions |
| `pure` | minimal, no I/O at all | declared, empty until named | 16 MiB, 10M | none | config evaluation, expressions, generated snippets |

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
one-script-per-process CLI and a bug in a dispatcher. Those three are also why an evaluation holds a
lock for its whole duration: `mlua` locks per operation, so without it two threads sharing an engine
set each other's arguments, and the ceilings are the least of what goes wrong.

One concrete piece of the isolation problem is closed rather than deferred. Every Lua string shares
a metatable whose `__index` is the `string` library, so below `Full` that metatable is hidden behind
`__metatable`. Without it a script could write

```lua
getmetatable('').__index.upper = function() return 'PWNED' end
```

and every script afterwards on the same engine would get that function — while reading no global and
calling no host module, so nothing it did would look suspicious. Method calls are unaffected;
`('x'):upper()` never goes through `getmetatable`.

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

- **Isolation between successive scripts on a reused engine.** Still open for globals generally,
  and `Lua::sandbox` is still unavailable to implement it cheaply. The string metatable is handled;
  ordinary globals a script writes are not, and neither is mutation of a shared library table such
  as `string.foo = ...`. Per-chunk environments via `Chunk::set_environment` would close most of it
  and would have to carry `require` into loaded modules too, so it is a design pass rather than a
  patch. Nothing needs it until registered extensions exist.
- **Grant granularity for `proc`.** Still an allowlist of executable names, matched on the program
  as written — so `/bin/echo` is refused when `echo` is granted, and a wrapper earlier on `PATH`
  defeats it. Whether that matters depends on whether the `fs` write grants can reach anywhere on
  `PATH`, which is a question about a specific policy rather than about the mechanism.

Settled since this document was first written: **`utf8` is on every surface.** It was absent below
`trusted` by accident rather than decision — the bit was simply never named in the library set — and
the criterion for `Minimal` was always nondeterminism and authority rather than size. `utf8` has
neither: it reads a string and returns a number. Without it `#s` counts bytes with no way to count
characters, so a confined script could not truncate text without risking a cut through the middle of
one.

Also settled: **the CLI default is `confined`**, which is the
previous default's language surface plus ceilings — so it is strictly safer and behaviourally
identical for any script that terminates.

## See also

- [architecture.md](architecture.md) — where policy sits among the three layers.
- [stdlib.md](stdlib.md) — which modules need grants and which do not.
- [extensions.md](extensions.md) — how an extension requests capabilities, and how the host bounds
  the request.
