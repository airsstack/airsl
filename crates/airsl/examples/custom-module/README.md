# custom-module

Contributes a module this crate does not ship, under a root table of the host's choosing — and has
it refuse a call it was not granted authority for.

This is the seam that makes `airsl` a shared Lua integration point rather than a fixed script
runner. It gets its own example because the contributed module goes through the *same*
`HostModule` trait, the same uniqueness check and the same `InstallContext` as the twelve
built-ins: there is no second, lesser path for third-party code, and the only way to show that is
to add one and watch it come out indistinguishable at the call site.

## Run

```bash
cargo run -p airsl --example custom-module
```

`metrics.lua` compiles on its own — `airsl check` parses without running, so the `myapp.*` globals
being absent outside this example is not a problem:

```bash
airsl check crates/airsl/examples/custom-module/
```

Running it with the `airsl` binary would fail, and should: the binary builds engines with the
built-in stdlib under the `airsstack` root, and knows nothing about `myapp.metrics`. A module
contributed from Rust is reachable only from a host that installed it.

## Output

```
root table: myapp
modules: json, path, fs, env, proc, regex, hash, time, glob, stdio, hook, ext, metrics

["myapp.metrics recorded requests","myapp.metrics recorded cache.hits","myapp.metrics recorded cache.misses"]
reset allowed: false
refusal: runtime error: metrics.reset denied: it needs a filesystem write grant — granted: none
```

## What it demonstrates

Three things a contributor needs to know, in the order they bite:

- **Depend on `airsl::mlua`, not on `mlua`.** The binding is re-exported from the crate root
  deliberately — `install` receives an `&mlua::Lua` and an `&mlua::Table`, so a module cannot be
  written without naming those types. A separately declared `mlua` version produces type errors
  that never mention the real cause. This example writes `airsl::mlua::Lua` in full to make the
  point unmissable.
- **`install` receives the policy** through `InstallContext` (`src/modules/registry.rs:33`). A
  module that guards an operation reads its authority from `context.grants()`
  (`src/modules/registry.rs:52`) rather than from a copy it was constructed with, which is what
  makes it impossible for a module to enforce one thing while `airsl doctor` reports another.
- **Name your own root table.** `RootTable::new("myapp")` (`src/builder.rs:83`); without it a
  third-party module lands in a namespace called `airsstack`, which belongs to this crate rather
  than to the embedding application. The module asks the context for the name
  (`src/modules/registry.rs:61`) instead of hard-coding it, so its own messages stay correct under
  whatever root the host picked — hence `myapp.metrics recorded …` in the output.

And two things the output shows about the surrounding design:

- **A module is present even when ungranted.** `reset` is installed unconditionally and refuses at
  the *call*, naming what was granted. Refusing to install would make `if myapp.metrics then` mean
  "am I allowed" instead of "does this runtime have it" — two questions that must not share an
  answer. The authority is read once at installation and captured; only the decision is deferred.
- **Enforcement lives in Rust, never in Lua.** The check runs inside the host function before the
  operation, so `pcall` *observes* the refusal without bypassing it. Lua never holds a handle to
  anything it was not granted.

The module list in the output is insertion order (`src/modules/registry.rs:144` preserves it), which
is why `metrics` appears last, after the twelve from `airsl::modules::stdlib()`
(`src/modules/stdlib.rs:22`). `insert` rejects a duplicate name, so colliding with `json` is an
error when the engine is built rather than a module that silently replaces another.

## See also

- [extensions](../../docs/extensions.md) — the extension host. Note the status table in
  `docs/README.md`: manifests, negotiation and dispatch are **proposed**, not built. What this
  example uses — `HostModule`, `ModuleSet`, `InstallContext`, `RootTable` — is implemented today.
- [`hello-eval`](../hello-eval/) — the builder this example adds two calls to.
