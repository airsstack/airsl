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
surface: restricted, grants: access stdin,stdout,stderr,clock,cwd,require,zones
```

## What it demonstrates

- **The builder is type-state.** `EngineBuilder<Missing>` has no `build()`; only `policy()` returns
  the `EngineBuilder<Present>` that does (`EngineBuilder::policy` in `src/builder.rs`). Forgetting
  to sandbox an engine is not a mistake this API lets you make.
- **`eval` versus `eval_to`.** `eval` runs a chunk for its effects and discards the result
  (`Engine::eval` in `src/engine.rs`); `eval_to::<T>` converts whatever the chunk returned into a
  Rust type through `FromLuaMulti` (`Engine::eval_to`).
- **`Script::from_source` names the chunk.** The name is what appears in an error message, so it is
  a required argument rather than an option (`Script::from_source` in `src/script.rs`).
- **The policy reads back off the engine** (`Engine::policy`). A module enforces the same object
  the engine reports, which is why `airsl doctor` cannot describe one policy while the runtime
  applies another.

`Policy::confined()` (in `src/sandbox/policy.rs`) holds every host-access member — standard input
and output, the clock, the working directory, `require` and time zones — and no filesystem,
environment or exec grant, so a script under it can compute but cannot reach files, variables or a
program. The chunks here call no host function that needs any of it, so no grant is exercised.

## See also

- [Your First Engine](https://airsstack.github.io/airsl/ch06-01-first-engine.html) — the guided version of this.
- [`filesystem-grants`](../filesystem-grants/) — the next step: handing a script authority.
