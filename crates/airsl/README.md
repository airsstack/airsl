# airsl

Embeddable Lua runtime with a host standard library, written in Rust on top of [`mlua`].

`airsl` runs sandboxed Lua and gives those scripts the capabilities a shell or Python script would
otherwise reach for — JSON, filesystem access, subprocesses, real regular expressions — through host
modules implemented in Rust. Everything a script can do arrives under a single `airsstack` global,
so the host decides the surface rather than the Lua standard library deciding it.

```rust
use airsl::{Engine, Sandbox, Script};

let engine = Engine::builder().sandbox(Sandbox::Restricted).build()?;
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

Under `Sandbox::Restricted` — the default, and what the CLI uses unless told otherwise — a script
gets `string`, `table`, `math`, `coroutine`, and the pure-computation parts of `os`. It does **not**
get `io`, `debug`, `package`, the chunk loaders (`load`, `loadstring`, `dofile`, `loadfile`), or the
`os` functions that reach outside the process (`execute`, `exit`, `getenv`, `remove`, `rename`,
`tmpname`, `setlocale`).

`os.setlocale` is withheld for a subtler reason than the rest: Lua compares strings with `strcoll`,
so a script that changes the locale changes the sort order of every subsequent `table.sort`.

## Failure policy

`FailurePolicy` makes explicit what is otherwise a convention. A script run as an editor or agent
hook must not turn its own failure into a non-zero exit, because the caller reads that as a signal
rather than a diagnostic. `FailurePolicy::FailOpen` says so in the type system;
`FailurePolicy::Report` is the default for anything a person invoked directly.

## Extending it

Implement `HostModule` and add it to a `ModuleSet`. The module becomes a subtable of `airsstack`
alongside the built-ins, and the host crate never has to modify `airsl` to contribute one.

## Documentation

[`docs/`](docs/README.md) covers the architecture, the capability and sandbox model, the host
standard library roster, and the extension system. Most of it is design rather than shipped code;
each document says which is which.

[`mlua`]: https://crates.io/crates/mlua
