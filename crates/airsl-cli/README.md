# airsl-cli

The `airsl` command. It runs Lua scripts that can only touch what the command line allows.

## Install

```sh
# macOS and Linux
curl -LsSf https://airsstack.github.io/airsl/install.sh | sh
```

```powershell
# Windows
powershell -ExecutionPolicy Bypass -c "irm https://airsstack.github.io/airsl/install.ps1 | iex"
```

The installer puts `airsl` in `~/.local/bin` and adds that folder to your `PATH`. Set
`AIRSL_VERSION=0.1.4` to install a particular release instead of the latest. The prebuilt binary
needs no Rust, Lua or C compiler.

To build it yourself instead, with a Rust toolchain and a C compiler:

```sh
cargo install --locked airsl-cli
```

## A quick tour

A script gets its arguments in `arg`, the way a shell script gets `$1`:

```console
$ airsl run greet.lua airsl
hello, airsl
```

Nothing outside the script is reachable until you grant it. Each grant is a flag, so whoever reads
the command can see what the script may touch:

```console
$ airsl run --allow-read config --allow-write backups backup.lua config/app.toml backups/app.toml
copied 12 bytes
```

| Flag | Lets the script |
|---|---|
| `--allow-read DIR` | read files under `DIR` |
| `--allow-write DIR` | write files under `DIR` |
| `--allow-env NAME` | read and set the environment variable `NAME` |
| `--allow-exec PROGRAM` | run `PROGRAM` |

Every script also runs under a memory limit and an instruction limit, so one that loops forever is
stopped instead of hanging:

```console
$ airsl run forever.lua
airsl: script `forever.lua` exceeded its instruction limit of 100000000
```

## Commands

| Command | What it does |
|---|---|
| `airsl run SCRIPT [ARGS…]` | run a script |
| `airsl test [PATH]` | run every `*_test.lua` and `test_*.lua` file under `PATH` |
| `airsl check [PATH]` | compile every `.lua` file under `PATH` without running any of it |
| `airsl doctor` | show the version, and what a script would be allowed to do |
| `airsl ext doctor DIR` | show what an extension asks for, and what it would be given |
| `airsl ext fire DIR EVENT` | load an extension and send it one event, with the payload on stdin |

`--policy trusted|confined|pure` picks a preset, `--memory-limit` and `--instruction-limit` change
the limits, and `--fail-open` keeps a failing script from failing whatever ran it, which is what an
editor or agent hook needs.

## Learn more

[The airsl Book](https://airsstack.github.io/airsl/) walks through all of this with examples:

- [Getting Started](https://airsstack.github.io/airsl/ch01-00-getting-started.html): install, and
  run your first script
- [Asking Permission](https://airsstack.github.io/airsl/ch03-00-asking-permission.html): files,
  environment variables, programs and presets
- [When Scripts Misbehave](https://airsstack.github.io/airsl/ch04-00-when-scripts-misbehave.html):
  limits, and failing safely in hooks
- [Testing and Checking](https://airsstack.github.io/airsl/ch05-05-testing.html): `airsl test` and
  `airsl check`
- [Command-Line Reference](https://airsstack.github.io/airsl/appendix-01-cli.html): every command
  and flag
- [Installation Details](https://airsstack.github.io/airsl/appendix-04-installation.html): archives,
  checksums, attestations and unsigned binaries

## Releases

See [CHANGELOG.md](https://github.com/airsstack/airsl/blob/main/CHANGELOG.md). This crate and the
`airsl` library it is built on are versioned independently and share one changelog, so an entry for
`airsl` describes this command too.
