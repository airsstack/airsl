# Examples

Each example is a directory with its own `README.md`, a `main.rs`, and — where the Lua is the
subject rather than the plumbing — the `.lua` files it runs. Read them in the order below; each one
assumes the one above it.

Run one:

```bash
cargo run -p airsl --example hello-eval
```

Run all of them, and compile every shipped `.lua`:

```bash
cargo make examples
```

That task is not part of `cargo make dod`. The gate compiles every example (`--all-targets`) but
does not run one, which is the difference between "it builds" and "it works" — so CI runs
`cargo make examples` as a separate step.

## The ladder

### First contact

| Example | What it shows |
| --- | --- |
| [`hello-eval`](hello-eval/) | The smallest embedding: type-state builder, `eval` vs `eval_to`. The one example with inline Lua. |
| [`values-and-json`](values-and-json/) | Values across the boundary: `arg`, tables back to Rust, and byte-stable JSON. |
| [`policy-presets`](policy-presets/) | One script under `trusted`, `confined` and `pure` — what each `LanguageSurface` withholds. |

### One example per grant axis

| Example | What it shows |
| --- | --- |
| [`filesystem-grants`](filesystem-grants/) | Read and write roots as separate grants, atomic replace, exclusive claim, and a refusal. |
| [`env-and-proc`](env-and-proc/) | An environment allowlist, the per-process overlay a child sees, and argv-only subprocesses. |
| [`denials-are-data`](denials-are-data/) | Every module present even when ungranted, refusing with a message that names what *was* granted. |

### Resource and failure semantics

| Example | What it shows |
| --- | --- |
| [`resource-limits`](resource-limits/) | Instruction and memory ceilings, and why a breach is classified structurally rather than by message text. |
| [`failure-policy`](failure-policy/) | Fail-open versus fail-closed, and why a limit breach is reported under both. |

### Extending the runtime

| Example | What it shows |
| --- | --- |
| [`custom-module`](custom-module/) | Implementing `HostModule`, reading authority from `InstallContext`, and naming your own root table. |

## Conventions these examples follow

- **Output is deterministic.** No wall-clock timestamps, no absolute paths, no figures that differ
  between Linux and macOS. Every `## Output` block in a per-example README is real captured stdout,
  and running the example again reproduces it byte for byte.
- **Nothing is written inside the repository.** Examples that need a writable directory use a
  temporary one, removed when the example ends.
- **Only `sh` is ever executed.** An example that wanted some other program would be an example that
  fails on a machine without it.
- **`Script::from_file` is a host read.** Loading a script from disk is not governed by `FsGrant`;
  the grant governs the `airsstack.fs.*` calls the script itself makes. An example may therefore
  load a script from a path that script could not read.
- **Several examples read the file and name the chunk themselves.** `Script::from_file` takes the
  chunk name from the path as given (`src/types/chunk_name.rs:60`), and these examples build that
  path from `CARGO_MANIFEST_DIR` — so a raised error would carry an absolute path into the
  traceback, which is neither reproducible nor anyone else's business. Where an example prints a
  failure, it uses:

  ```rust
  let script = Script::from_source(std::fs::read_to_string(&path)?, "raises.lua")?;
  ```

  This is worth knowing outside the examples: a hook loaded from an absolute path puts that path in
  every diagnostic it emits. `with_root` is a separate wither, so a script named this way can still
  have `require` — `Script::from_source(text, "main.lua")?.with_root(dir)` gives both.

## See also

- [tutorial](../docs/tutorial.md) — from nothing to a working hook, in ten minutes.
- [how-to](../docs/how-to.md) — recipes, shorter than an example and without the scaffolding.
- [sandbox](../docs/sandbox.md) — what a grant is and where it is enforced.
- [host standard library](../docs/stdlib.md) — the full module roster.
