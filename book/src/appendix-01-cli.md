# A. Command-Line Reference

```
airsl run    [OPTIONS] <script.lua> [args…]
airsl test   [OPTIONS] [<path>]
airsl check  [<path>]
airsl doctor [--policy <preset>] [GRANTS]
airsl ext doctor <DIR>         [GRANTS] [LIMITS] [--event NAME] [--var NAME=VALUE]
airsl ext fire   <DIR> <EVENT> [GRANTS] [LIMITS] [--event NAME] [--var NAME=VALUE]
```

## Commands

| Command | Does |
|---|---|
| `run` | runs one script; everything after the script's path goes to `arg` |
| `test` | runs every `*_test.lua` and `test_*.lua` under a folder; finding none is a failure |
| `check` | compiles every `.lua` under a folder without running any; finding none is a failure |
| `doctor` | prints the policy a script would get from the preset and grant flags you give it |
| `ext doctor` | prints what a host would grant, reduce and deny for an extension, without running it |
| `ext fire` | loads an extension and fires one event; payload on stdin, result on stdout as JSON |

## Options for `run`, `test` and `doctor`

`doctor` takes `--policy` and the `--allow-…` flags. It takes no `--memory-limit` or
`--instruction-limit`, and no `--fail-open`: it shows the preset's own limits.

| Option | Does |
|---|---|
| `--policy <trusted\|confined\|pure>` | picks the preset; the default is `confined` |
| `--allow-read DIR` | grants reading under `DIR`; repeatable |
| `--allow-write DIR` | grants writing under `DIR`; repeatable; does not grant reading |
| `--allow-env NAME` | grants reading and setting `NAME`; repeatable |
| `--allow-exec PROGRAM` | grants running `PROGRAM`, matched on the name as written; repeatable |
| `--allow-access NAME` | host access the sandbox may use; repeatable. Only `pure` starts without them |
| `--memory-limit <BYTES\|none>` | replaces the preset's memory limit |
| `--instruction-limit <COUNT\|none>` | replaces the preset's instruction limit |
| `--fail-open` | `run` only: swallows any failure and exits `0` |

Under `--policy trusted`, the `--allow-…` flags are ignored, because nothing is restricted. Under
`--policy confined`, `--allow-access` changes nothing, because `confined` already holds every kind of
host access. Under `--policy pure`, it is the only way to give a script any.

The names are `stdin`, `stdout`, `stderr`, `clock`, `cwd`, `require` and `zones`. An unknown name is
refused with that list, and the exit status is `2`. Naming the same access twice is accepted and
grants it once.

## Options for `ext doctor` and `ext fire`

These take the same `--allow-…`, `--memory-limit` and `--instruction-limit` flags, which widen the
host's ceiling. The ceiling always starts from `confined`. There is no `--policy`, because `trusted`
cannot serve as a ceiling.

`--allow-access` is accepted here too, but it cannot change the ceiling: `confined` already holds
every kind of host access.

| Option | Does |
|---|---|
| `--event NAME` | declares an event the host fires, beyond the one being fired; repeatable |
| `--var NAME=VALUE` | supplies a value for `$NAME` in the manifest; repeatable |

## Exit codes

| Command | `0` | `1` |
|---|---|---|
| `run` | the script finished | the script failed or hit a limit; with `--fail-open`, always `0` |
| `test` | every test passed | a test failed, or no test files were found |
| `check` | every file compiled | a file failed to compile, or no `.lua` files were found |
| `ext doctor` | any decision, including deny | the manifest or the ceiling is invalid |
| `ext fire` | the event was fired and a result (or `null`) printed | any error |

## Environment variables

| Variable | Does |
|---|---|
| `AIRSL_DEBUG=1` | prints errors that `--fail-open` would hide |
| `AIRSL_CLI_NO_MODIFY_PATH=1` | read by the installer: leave `PATH` alone |

A script that hits a memory or instruction limit is always reported on standard error, even under
`--fail-open`.
