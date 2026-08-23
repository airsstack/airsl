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

### Working in Lua

| Example | What it shows |
| --- | --- |
| [`text-toolkit`](text-toolkit/) | `regex`, `path`, `hash`, `time` and `glob.match` under a policy that grants nothing — plus the `_test.lua` twin `airsl test` runs. |
| [`multi-file-project`](multi-file-project/) | The confined `require`: a module tree, its cache, and the three different functions that share the name. |

### The shape of a real host

| Example | What it shows |
| --- | --- |
| [`engine-reuse`](engine-reuse/) | What an evaluation resets and what it leaves behind, and why a shared engine buys reuse rather than parallelism. |
| [`agent-hook`](agent-hook/) | A `PreToolUse` hook end to end: the payload on stdin, the envelope on stdout, and why only the exit status blocks a tool call. |

### Extending the runtime

| Example | What it shows |
| --- | --- |
| [`custom-module`](custom-module/) | Implementing `HostModule`, reading authority from `InstallContext`, and naming your own root table. |
| [`extension-host`](extension-host/) | Loading a directory of third-party extensions under one ceiling, and broadcasting one event to every extension that made it in. |

## Conventions these examples follow

- **Output is deterministic.** No wall-clock timestamps, no absolute paths, no figures that differ
  between Linux and macOS. Every `## Output` block in a per-example README is real captured stdout,
  and running the example again reproduces it byte for byte.
- **Nothing is written inside the repository.** Examples that need a writable directory use a
  temporary one, removed when the example ends.
- **Printing from both sides is safe, and several examples do.** Lua's `print` writes to C `stdout`
  and `println!` writes to Rust's, which sounds like two buffers waiting to reorder — they are not.
  Lua flushes after every line (`lua_writeline` is `fwrite` followed by `fflush`), Rust's stdout is
  a `LineWriter` that flushes on the newline, and `airsstack.stdio.write` flushes explicitly
  (`src/modules/stdio.rs:109`). So a run piped into a file has the same line order as a run on a
  terminal, which is what lets an `## Output` block be captured through a pipe and still be true.
- **Only `sh` is ever executed.** An example that wanted some other program would be an example that
  fails on a machine without it.
- **A shipped `*_test.lua` runs under `confined` with no grants.** `cargo make examples` finishes by
  running `airsl test` over this whole tree, and that command's default policy is confined with an
  empty grant set. Discovery is by directory rather than a list, for the reason the run above is: a
  test file no command picks up is worse than no test file. So a test that needs authority to run is
  a test this suite cannot carry.
- **`Script::from_file` is a host read.** Loading a script from disk is not governed by `FsGrant`;
  the grant governs the `airsstack.fs.*` calls the script itself makes. An example may therefore
  load a script from a path that script could not read.
- **Several examples name the chunk themselves.** `Script::from_file` takes the chunk name from the
  path as given (`src/types/chunk_name.rs:60`), and these examples build that path from
  `CARGO_MANIFEST_DIR` — so a raised error would carry an absolute path into the traceback, which is
  neither reproducible nor anyone else's business. Where an example prints a failure, it uses:

  ```rust
  let script = Script::from_file(&path)?.with_name("raises.lua")?;
  ```

  This is worth knowing outside the examples: a hook loaded from an absolute path puts that path in
  every diagnostic it emits, and a hook's stderr often ends up in someone else's log. `with_name`
  (`src/script.rs:118`) changes only the label — the source, the arguments and the root the file was
  read from are untouched, so a renamed script still `require`s exactly what it could before.

## See also

- [tutorial](../docs/tutorial.md) — from nothing to a working hook, in ten minutes.
- [how-to](../docs/how-to.md) — recipes, shorter than an example and without the scaffolding.
- [sandbox](../docs/sandbox.md) — what a grant is and where it is enforced.
- [host standard library](../docs/stdlib.md) — the full module roster.
