# hello-eval

The smallest complete embedding: build an engine under a policy, run a chunk, take a value back.

This is the one example that keeps its Lua inline. The chunk is a single expression and the subject
is the Rust API, so a separate `.lua` file would be ceremony — every other example loads a script
from disk, which is what a real embedder does.

## Run

```bash
cargo run -p airsl --example hello-eval
```

## Output

```
hello from Lua Lua 5.4
1..10 sums to 55
surface: restricted, grants: none
```

## What it demonstrates

- **The builder is type-state.** `EngineBuilder<Missing>` has no `build()`; only `policy()` returns
  the `EngineBuilder<Present>` that does — `src/builder.rs:41` and `src/builder.rs:67`. Forgetting
  to sandbox an engine is not a mistake this API lets you make.
- **`eval` versus `eval_to`.** `eval` runs a chunk for its effects and discards the result
  (`src/engine.rs:215`); `eval_to::<T>` converts whatever the chunk returned into a Rust type
  through `FromLuaMulti` (`src/engine.rs:237`).
- **`Script::from_source` names the chunk.** The name is what appears in an error message, so it is
  a required argument rather than an option — `src/script.rs:37`.
- **The policy reads back off the engine** (`src/engine.rs:131`). A module enforces the same object
  the engine reports, which is why `airsl doctor` cannot describe one policy while the runtime
  applies another.

`Policy::confined()` (`src/sandbox/policy.rs:73`) grants nothing, so a script under it can compute
but cannot reach the filesystem, the environment or a subprocess. Nothing here needs authority, so
the empty grant set is never consulted.

## See also

- [tutorial](../../docs/tutorial.md) — the guided version of this, extended into a working hook.
- [`filesystem-grants`](../filesystem-grants/) — the next step: handing a script authority.
