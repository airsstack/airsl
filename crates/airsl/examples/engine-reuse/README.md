# engine-reuse

The lifecycle of one engine: what an evaluation resets, what it leaves behind, and what sharing one
between threads actually buys.

Every other example builds an engine, runs one script and exits — which is exactly the shape that
hides the two facts a dispatch loop depends on. An engine is *not* a fresh sandbox per evaluation,
and an engine shared between threads is a way to avoid rebuilding a state rather than a way to run
scripts at the same time. Neither is visible from a single run, and both are cheap to get wrong.

Like `hello-eval`, the chunks stay inline: the subject here is the Rust API and each chunk is a
line. The `require` section is the exception, because a module cache needs modules on disk — it
writes them into a `tempfile::TempDir` that is removed when the example ends.

## Run

```bash
cargo run -p airsl --example engine-reuse
```

## Output

```
the arg table is written before every evaluation:
  arg[1] = 1 doubles to 2
  arg[1] = 2 doubles to 4
  arg[1] = 3 doubles to 6
  arg[0] is the chunk's own name: reporter

the instruction counter is reset before every evaluation:
  the first evaluation exhausts the budget: script `runaway` exceeded its instruction limit of 100000
  the next evaluation gets the whole ceiling again, and returns 7

the globals a script writes outlive it:
  SESSION, before anything wrote it: nil
  a script named `remember` assigns it
  SESSION, read back by an unrelated script: written by the first script

the require root belongs to the script, not to the engine:
  alpha/main.lua resolves require('lib') against its own directory: alpha
  beta/main.lua resolves require('lib') against its own directory: beta

the require module cache belongs to the engine, not to the script:
  one counting module, required by three evaluations: 1, 1, 1

one engine shared by 4 threads through an Arc:
  thread 0 saw its own argument and doubled it to 0
  thread 1 saw its own argument and doubled it to 2
  thread 2 saw its own argument and doubled it to 4
  thread 3 saw its own argument and doubled it to 6
  evaluations are serialised: sharing avoids rebuilding, it does not parallelise
```

The thread block is sorted by worker id before it is printed. The order four threads finish in is
not this example's to choose, and every `## Output` block here has to reproduce byte for byte.

## What it demonstrates

- **Reuse is the shape, not an optimisation.** Building a state and evaluating on one differ by
  roughly thirty-fold — `benches/eval.rs` times construction, evaluation on a reused engine and
  evaluation on a fresh engine per call, and [docs/README.md](../../docs/README.md) tabulates the
  result. Those are a snapshot from one machine at 1000 iterations, not a guarantee; re-take them
  with `cargo bench -p airsl` before relying on them. Nothing is measured *inside* this example,
  because a printed duration would give it a different output on every run.
- **The `arg` table is per evaluation.** `set_arguments` builds a fresh table and assigns the global
  before the chunk loads (`src/engine.rs:170`), so one `Script` cloned with three different
  `with_args` (`src/script.rs:130`) yields three different answers on one engine. `arg[0]` is the
  chunk's own name — Lua's convention for a standalone script, and what a ported shell script reads
  where it read `$0`.
- **The instruction counter is per evaluation.** `eval_to` resets the budget before anything else
  (`src/engine.rs:150`), so the script after a breach gets the whole ceiling rather than what the
  previous one left of it. The alternative fails in a way that depends on what ran before it, which
  is not reproducible from the failing script alone. Tested at `src/engine.rs:467`.
- **The `require` root is per evaluation.** `set_require` decides from the *script's* directory
  (`src/engine.rs:190`), so `alpha/main.lua` and `beta/main.lua` each resolve `require('lib')`
  against their own root on the same engine. A confined script built from source has no directory
  and so gets no `require` at all.
- **The `require` module cache is not.** The table of loaded modules is created once and kept
  (`src/require_loader.rs:99`), keyed by canonical absolute path, so it stays correct across roots.
  Three evaluations of a script requiring a counting module answer `1, 1, 1`, which is what Lua's
  own `package.loaded` does. This used to be the other way round — the cache was rebuilt per
  evaluation and answered `1, 2, 3` — so cached state was discarded while leaked state was kept,
  exactly inverted for a dispatch path.
- **Globals persist, and that is the sharp edge.** Nothing between evaluations restores the
  environment: `mlua`'s one-call version (`Lua::sandbox`) is Luau-only, so isolating successive
  scripts on one engine has to be built rather than borrowed
  ([architecture](../../docs/architecture.md)). A script writes `SESSION` and an unrelated chunk
  reads it back. **A dispatch loop that reuses one engine across untrusted scripts is leaking state
  between them** — give one engine scripts that trust each other, and give an untrusted script an
  engine of its own. The memory ceiling has the same shape: it caps the state, not the script, so an
  engine carries earlier scripts' garbage until the collector runs.
- **`Engine` is `Send + Sync`, and sharing is not parallelism.** Four threads hold one engine through
  an `Arc` and each sees its own arguments. That works because the engine takes a lock spanning the
  whole evaluation (`src/engine.rs:145`) rather than relying on `mlua`'s per-operation locking, which
  is memory-safe without being correct across the four steps of an eval — reset the budget, write
  `arg`, install `require`, run the chunk (`src/engine.rs:142`). Before that lock existed, eight
  threads evaluating `return arg[1]` on one engine got another thread's argument 12,247 times out of
  16,000 (`src/engine.rs:50`); the regression test is at `src/engine.rs:512`. Lua on a single state
  cannot execute in parallel however you hold it, so the lock costs an uncontended acquisition and
  no throughput — **a shared engine buys reuse, never concurrency.** For parallelism, build one
  engine per thread and pay construction once each.

The ceiling this example arms is enforced to within the hook's 10,000-instruction check interval
(`src/instruction_budget.rs:34`), which is why `TIGHT_INSTRUCTIONS` is a multiple of it. The figure
in the output is the limit that was set, not a measurement, so it is identical on every platform.

## See also

- [`resource-limits`](../resource-limits/) — arming the ceilings this example only resets.
- [`custom-module`](../custom-module/) — why a `HostModule` must be `Send + Sync`: an engine is
  shareable, so everything installed into one has to be too.
- [architecture](../../docs/architecture.md) — the engine-lifecycle table, and what `Engine: Sync`
  does and does not buy.
- [how-to](../../docs/how-to.md) — the same two recipes without the scaffolding.
