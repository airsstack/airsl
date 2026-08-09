# airsl

Embeddable Lua runtime with a host standard library, written in Rust on top of [`mlua`].

`airsl` runs sandboxed Lua and gives those scripts the capabilities a shell or Python script would
otherwise reach for — JSON, filesystem access, subprocesses, real regular expressions — through host
modules implemented in Rust. Everything a script can do arrives under a single `airsstack` global,
so the host decides the surface rather than the Lua standard library deciding it.

```rust
use airsl::{Engine, Policy, Script};

let engine = Engine::builder().policy(Policy::confined()).build()?;
let script = Script::from_source("return airsstack.json.encode({ok = true})", "demo")?;
assert_eq!(engine.eval_to::<String>(&script)?, r#"{"ok":true}"#);
# Ok::<(), airsl::Error>(())
```

## Building

A **C compiler is required**. `mlua`'s `vendored` feature compiles Lua 5.4 from the C sources
shipped by the `lua-src` crate and links it statically, so there is nothing to install and no
`pkg-config` involved — but `cc` must be present.

Lua 5.4 rather than 5.1 or LuaJIT: only 5.3 and later distinguish integers from floats in the VM.
On 5.1 a JSON `3` and a JSON `3.0` are the same value, which breaks byte-stable JSON output.

## What scripts can see

| Module | Purpose |
|---|---|
| `airsstack.json` | `encode`, `encode_pretty`, `decode` |

More modules are added as the tooling that drives this crate needs them.

## Policy

A policy answers three independent questions: which of Lua's own libraries a script sees, what the
host modules it reaches may touch, and how much it may consume. Three presets cover the cases worth
naming.

| Preset | Language surface | Grants | Ceilings |
|---|---|---|---|
| `Policy::trusted()` | everything except `debug`, including `io`, `os`, `package` | unrestricted | none |
| `Policy::confined()` *(default)* | `string`, `table`, `math`, `coroutine`, pure `os` | declared only | 64 MiB, 100M instructions |
| `Policy::pure()` | `string`, `table`, `math` | declared only | 16 MiB, 10M instructions |

Below `trusted`, a script does **not** get `io`, `debug`, `package`, the chunk loaders (`load`,
`loadstring`, `dofile`, `loadfile`), or the `os` functions that reach outside the process
(`execute`, `exit`, `getenv`, `remove`, `rename`, `tmpname`, `setlocale`). `pure` additionally drops
`os` and `coroutine` entirely.

`os.setlocale` is withheld for a subtler reason than the rest: Lua compares strings with `strcoll`,
so a script that changes the locale changes the sort order of every subsequent `table.sort`.

Adjust any preset with a wither:

```rust
use airsl::{Policy, ResourceLimits, InstructionLimit};

let policy = Policy::confined()
    .with_limits(ResourceLimits::none().with_instructions(Some(InstructionLimit::count(1_000))));
```

Grants are typed but currently empty — no host module takes one yet, because the grant vocabulary is
the module list and `json` needs no authority.

## Ceilings

The memory ceiling turns an allocation past the cap into a catchable error rather than an OOM that
takes the host process with it. The instruction ceiling is the only defence against a script that
never terminates; no capability decision helps against `while true do end`, because it reaches
nothing.

Both surface as their own error variants, so a script stopped for consuming the host's resources is
distinguishable from one that merely failed:

```rust
use airsl::ExhaustedLimit;
# use airsl::{Engine, Policy, Script};
# let engine = Engine::builder().policy(Policy::confined()).build()?;
# let script = Script::from_source("error('boom')", "demo")?;
if let Err(error) = engine.eval(&script) {
    match error.exhausted_limit() {
        Some(ExhaustedLimit::Instructions) => eprintln!("the script did not terminate"),
        Some(ExhaustedLimit::Memory) => eprintln!("the script exhausted its memory"),
        None => eprintln!("the script failed: {error}"),
    }
}
# Ok::<(), airsl::Error>(())
```

The classification is structural — the engine's own instruction counter, and the VM error chain —
never the message text, so a script cannot disguise its own failure as a resource breach.

Two things worth knowing. The memory ceiling caps the whole state rather than each script, so an
engine that has run several scripts carries whatever garbage they left until the collector runs.
And the instruction ceiling is enforced to within a check interval rather than exactly.

## `require`

A script loaded from a file may `require` its siblings; a script built from source may not, because
it has no directory. Targets cannot contain a path separator or a `..` component, so an escape
cannot be spelled, and the resolved path is canonicalised and checked for containment, which catches
a symlink pointing out of the root. Cycles raise an error rather than recursing.

Under `trusted` Lua's own `require` is left in place. Under `pure` there is none at all.

## Failure policy

`FailurePolicy` makes explicit what is otherwise a convention. A script run as an editor or agent
hook must not turn its own failure into a non-zero exit, because the caller reads that as a signal
rather than a diagnostic. `FailurePolicy::FailOpen` says so in the type system;
`FailurePolicy::Report` is the default for anything a person invoked directly.

## Extending it

Implement `HostModule` and add it to a `ModuleSet`. The module becomes a subtable of the engine's
root table alongside the built-ins, and the host crate never has to modify `airsl` to contribute
one. `mlua` is re-exported as `airsl::mlua`, so a contributor stays on the version the engine was
built with.

An engine's root table defaults to `airsstack` and can be named per engine, so a module contributed
by a third party need not land in a namespace named after somebody else's system.

`Engine` is `Send + Sync`, so it can be shared between threads. One caveat: `require` is a global,
so give a shared engine scripts under a single root.

## Documentation

[`docs/`](docs/README.md) covers the architecture, the capability and sandbox model, the host
standard library roster, and the extension system. Each document says which parts are shipped and
which are design.

[`mlua`]: https://crates.io/crates/mlua
