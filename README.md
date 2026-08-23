# airsl

An embeddable Lua 5.4 runtime for Rust, where the **host** decides what a script may reach.

Lua is statically linked, a sandbox policy is required at construction rather than optional, and
every capability a script has arrives as a host module under one `airsstack` global. A script gets
JSON, filesystem access, subprocesses and real regular expressions only to the extent the embedding
program granted them — the authority lives in the grant, not in whether the table exists.

```rust
use airsl::{Engine, Policy, Script};

let engine = Engine::builder().policy(Policy::confined()).build()?;
let script = Script::from_source("return airsstack.json.encode({ok = true})", "demo")?;
assert_eq!(engine.eval_to::<String>(&script)?, r#"{"ok":true}"#);
# Ok::<(), airsl::Error>(())
```

## Crates

| Crate | What it is |
|---|---|
| [`airsl`](crates/airsl) | The library. Engine, sandbox policy, host-module registry, standard library. |
| [`airsl-cli`](crates/airsl-cli) | The `airsl` binary: `run`, `test`, `check`, `doctor`, `ext`. |

```
cargo add airsl              # embed the runtime
cargo install airsl-cli      # get the `airsl` binary
```

**Linux and macOS only.** `airsstack.proc` resolves executables by unix mode bits, which have no
Windows equivalent; the crate refuses to build off unix rather than pretending. Supporting Windows
is a decision about what "executable" means there, not a portability patch.

**A C compiler is required.** `mlua`'s `vendored` feature builds Lua 5.4 from the C sources shipped
by `lua-src` and links it statically, so there is no system Lua and no `pkg-config` — but `cc` must
be present.

## Documentation

Written to the [Diátaxis](https://diataxis.fr/) split; start at
[`crates/airsl/docs/README.md`](crates/airsl/docs/README.md).

- [Tutorial](crates/airsl/docs/tutorial.md) — embed the runtime, end to end
- [How-to](crates/airsl/docs/how-to.md) — task recipes
- [Sandbox](crates/airsl/docs/sandbox.md) — presets, grants, resource ceilings
- [Standard library](crates/airsl/docs/stdlib.md) — every host module
- [Architecture](crates/airsl/docs/architecture.md) — why it is shaped this way
- [Extension system](crates/airsl/docs/extensions.md) — dispatch (`ext` module, `Engine::dispatch`), the manifest parser, the ceiling, negotiation, the approver, the loader (`ExtensionHost`), and the `airsl ext` CLI (`doctor`, `fire`) — all built

## Development

```
cargo make dod          # the full gate: fmt, clippy, rustdoc, tests, doctests
cargo make deny         # advisories, licenses, duplicates, sources
cargo make install      # install the airsl binary from this checkout
cargo make --list-all-steps
```

Needs [cargo-make](https://github.com/sagiegurari/cargo-make)
(`cargo install --locked cargo-make`). CI runs `cargo make dod`, so the pipeline and a local run are
the same command.

## Releases

[CHANGELOG.md](CHANGELOG.md) covers both crates in one timeline. Each release is tagged per crate —
`airsl-v0.1.3`, `airsl-cli-v0.1.2` — because one commit has shipped two crates under two different
version numbers, which a single `vX.Y.Z` tag cannot name.

## License

Apache-2.0. See [LICENSE](LICENSE).
