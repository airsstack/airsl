# airsl

Lua scripts that can only touch what you allow.

`airsl` runs Lua 5.4 scripts inside a Rust program, and the program decides what each script may
reach. A script that reads a file has to be given a folder. A script that runs `git` has to be given
`git`. Everything else is refused, and the refusal says what was missing.

```console
$ airsl run --allow-read config --allow-write backups backup.lua config/app.toml backups/app.toml
copied 12 bytes

$ airsl run --allow-read config --allow-write backups backup.lua config/app.toml config/copy.toml
airsl: lua error in backup.lua: fs.write denied: `/home/you/project/config/copy.toml` is outside the granted write roots: /home/you/project/backups
```

Use it as a command, to write the scripts you would otherwise write in shell or Python. Or use it as
a library, to let your own program run scripts, plugins or extensions under rules it sets.

## Install

```sh
# macOS and Linux
curl -LsSf https://airsstack.github.io/airsl/install.sh | sh
```

```powershell
# Windows
powershell -ExecutionPolicy Bypass -c "irm https://airsstack.github.io/airsl/install.ps1 | iex"
```

To embed it in a Rust program:

```sh
cargo add airsl
```

## Read the book

**[The airsl Book](https://airsstack.github.io/airsl/)** starts from your first script and goes all
the way to hosting third-party extensions. Each chapter has its own small example.

| If you want to | Start at |
|---|---|
| run scripts from the command line | [Getting Started](https://airsstack.github.io/airsl/ch01-00-getting-started.html) |
| embed `airsl` in a Rust program | [Embedding airsl in Rust](https://airsstack.github.io/airsl/ch06-00-embedding.html) |
| load other people's code safely | [Extensions](https://airsstack.github.io/airsl/ch08-00-extensions.html) |
| look up a flag or a function | [Appendix](https://airsstack.github.io/airsl/appendix-00.html) |

The API reference is on [docs.rs](https://docs.rs/airsl).

## What is in this repository

| Crate | What it is |
|---|---|
| [`airsl`](crates/airsl) | The library: the engine, the sandbox, and the modules scripts use |
| [`airsl-cli`](crates/airsl-cli) | The `airsl` command: `run`, `test`, `check`, `doctor`, `ext`, `lua` |
| [`airsl-lua`](crates/airsl-lua) | The native module, shipped as the LuaRocks packages `airsstack` and `airsstack-luajit`, not on crates.io |

`airsl` runs on Linux, macOS and Windows (`x86_64-pc-windows-msvc`). The prebuilt command needs
nothing else installed. Building from source needs a C compiler, because Lua is compiled from its C
source and linked in. The native module is also built for LuaJIT. To run `airsstack` scripts in
the Lua you already have, see
[Running Scripts in Stock Lua](https://airsstack.github.io/airsl/ch01-05-stock-lua.html),
or run the scripts in [`crates/airsl-lua/examples`](crates/airsl-lua/examples).

## Development

```sh
cargo make dod      # the gate: fmt, clippy, rustdoc, tests, doctests, all warnings as errors
cargo make rocks    # build, pack and load both LuaRocks packages of the native module
cargo make book     # serve the book locally with live reload
cargo make deny     # advisories, licences, duplicates, sources
cargo make install  # install the airsl command from this checkout
```

The tasks need [cargo-make](https://github.com/sagiegurari/cargo-make)
(`cargo install --locked cargo-make`). CI runs `cargo make dod`, so a green local run and a green
pipeline mean the same thing.

`cargo make dod` loads the native module into a real Lua 5.4 and a real LuaJIT, so it needs `lua5.4`
and `luajit` on `PATH`, or `AIRSL_LUA54` and `AIRSL_LUAJIT` naming the interpreters. `cargo make
rocks` is not part of the gate. It builds the packages, so it needs Rust and cargo-make as well as
the network, because luarocks fetches the build backend, and luarocks on `PATH` or `AIRSL_LUAROCKS`
naming it. It then loads each package into its interpreter, so it needs the same `lua5.4` and
`luajit` as the gate. Where luarocks cannot find an interpreter by itself, `AIRSL_LUA54_DIR` and
`AIRSL_LUAJIT_DIR` name the directory of each.

## Releases

[CHANGELOG.md](CHANGELOG.md) covers both crates in one timeline. `airsl`, `airsl-cli` and the
LuaRocks packages share one version, and every release is a lockstep release of both crates: push
`airsl-v<version>` and `airsl-cli-v<version>` on the same commit. There is no library-only release.

For maintainers:

- An `airsl-cli-v*` tag starts the binary release. [dist](https://github.com/axodotdev/cargo-dist)
  builds every platform, the installers, checksums and attestations, then publishes the GitHub
  Release. An `airsl-v*` tag starts nothing. The `airsl-cli-v*` tag also builds the rocks
  of the native module before the GitHub Release, and uploads them to luarocks.org after it.
- `.github/workflows/airsl-cli-release.yml` is generated from `dist-workspace.toml`. Change the
  config and run `dist generate`; never edit the workflow by hand, or the release's plan step fails.
- `.github/workflows/airsl-cli-smoke.yml` is written by hand. It runs every archive on its own
  platform before anything is published, and on every pull request.
- `.github/workflows/build-rocks.yml` and `.github/workflows/publish-rocks.yml` are written by hand
  too. The release workflow calls them through dist's `global-artifacts-jobs` and `publish-jobs`
  hooks. The upload needs the `LUAROCKS_API_KEY` secret, the API key of a luarocks.org account
  without two-factor on uploads, because an account that asks for a code cannot upload unattended.
- The release does not wait for CI. Tag only a commit on `main` whose pull request passed.
- Before tagging, rename `## Unreleased` in CHANGELOG.md to the heading form of the entries below
  it, `## airsl <version> — airsl-cli <version> — <date>`.
- After the first release whose `publish-rocks` job succeeds, the maintainer who pushed the
  `airsl-cli-v*` tag runs, against luarocks.org, the command lines that
  `book/src/ch01-05-stock-lua.md` shows under "Installing": `airsl lua setup --native` and the two
  `luarocks install --local` lines. They also run the Homebrew lines on macOS and the Windows lines
  in "Pointing LuaRocks at Lua 5.4 or LuaJIT". Each output is pasted under its own command line, in
  the chapter's `console` blocks, and merged as a `docs:` pull request.
- Release notes stay in CHANGELOG.md. The GitHub Release carries install instructions only.
- `.github/workflows/pages.yml` publishes the book and the `install/` scripts to GitHub Pages on
  every merge that touches them, so a change to `install/` is live as soon as it merges.

## License

Apache-2.0. See [LICENSE](LICENSE).
