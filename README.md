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
```

The `airsl` binary ships prebuilt for macOS, Linux and Windows from `airsl-cli` 0.1.4 on, so running
scripts needs no Rust toolchain:

```
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/airsstack/airsl/releases/latest/download/airsl-cli-installer.sh | sh
```

The Windows installer, the per-platform archives, and `cargo install` are in
[`airsl-cli`'s install section](crates/airsl-cli/README.md#install).

**Supported targets: Linux, macOS, and `x86_64-pc-windows-msvc`.** On Windows, `airsstack.proc` runs
`.exe` programs only — `npm`, `npx`, `yarn` and `tsc`, which ship as `.cmd` shims, are not reachable
through it.

**Building needs a C compiler** — embedding the library, or installing the binary with `cargo`; the
prebuilt binaries do not. `mlua`'s `vendored` feature builds Lua 5.4 from the C sources shipped by
`lua-src` and links it statically, so there is no system Lua and no `pkg-config` — but `cc` must be
present.

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
`airsl-v0.1.4`, `airsl-cli-v0.1.3` — because one commit has shipped two crates under two different
version numbers, which a single `vX.Y.Z` tag cannot name.

Only an `airsl-cli-v*` tag starts the binary release. [`dist`](https://github.com/axodotdev/cargo-dist)
builds every target, the installers, checksums and attestations, and publishes the GitHub Release;
an `airsl-v*` tag has no binary to ship and starts nothing. The release workflow,
`.github/workflows/airsl-cli-release.yml`, is generated from `dist-workspace.toml` — change the
config and run `dist generate`, never edit the workflow, because the release's own plan step fails
on a workflow that no longer matches its config. Before anything is published,
`.github/workflows/airsl-cli-smoke.yml` — written by hand, and wired in through dist's
`global-artifacts-jobs` — unpacks every archive on its own platform and runs the example scripts
through it; the Release waits for it. Every pull request runs the same builds and smoke tests
and stops short of publishing, so a change that breaks a release target fails on its pull request
rather than on the tag.

The release workflow does not wait for `ci.yml`, so a tag is not proof the gate passed. Tag only a
commit on `main` whose pull request passed `cargo make dod`. GitHub Release notes carry install instructions
only: dist finds release notes by a heading that starts with the version, which a changelog naming
two crates cannot offer, so the notes for a version stay in CHANGELOG.md.

## License

Apache-2.0. See [LICENSE](LICENSE).
