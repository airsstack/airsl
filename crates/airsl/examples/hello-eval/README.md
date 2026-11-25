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
  the `EngineBuilder<Present>` that does (both impls in `src/builder.rs`). Forgetting to sandbox an
  engine is not a mistake this API lets you make.
- **`eval` versus `eval_to`.** `eval` runs a chunk for its effects and discards the result
  (`Engine::eval` in `src/engine.rs`); `eval_to::<T>` converts whatever the chunk returned into a
  Rust type through `FromLuaMulti` (`Engine::eval_to` in `src/engine.rs`).
- **`Script::from_source` names the chunk.** The name is what appears in an error message, so it is
  a required argument rather than an option (`Script::from_source` in `src/script.rs`).
- **The policy reads back off the engine** (`Engine::policy` in `src/engine.rs`). A module enforces
  the same object the engine reports, which is why `airsl doctor` cannot describe one policy while
  the runtime applies another.

`Policy::confined()` (in `src/sandbox/policy.rs`) grants nothing, so a script under it can compute
but cannot reach the filesystem, the environment or a subprocess. Nothing here needs authority, so
the empty grant set is never consulted.

## See also

- [Your First Engine](https://airsstack.github.io/airsl/ch06-01-first-engine.html) — the guided version of this.
- [`filesystem-grants`](../filesystem-grants/) — the next step: handing a script authority.
