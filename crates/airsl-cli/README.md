# airsl-cli

The `airsl` binary: runs Lua scripts on the [`airsl`](../airsl) runtime.

## Install

```bash
cargo install --path crates/airsl-cli --force
airsl doctor
```

`doctor` prints the runtime version and the policy a script would actually run under:

```
airsl 0.1.0
  lua:          Lua 5.4
  language:     restricted
  root table:   airsstack
  grants:       none
  memory:       67108864 bytes
  instructions: 100000000 instructions
  modules:      json, path, fs, env, proc, regex, hash, time, glob, stdio, hook
```

Pass `--policy` to describe a different preset rather than the default.

## Usage

```
airsl run  [--policy <trusted|confined|pure>]
           [--allow-read <DIR>] [--allow-write <DIR>]
           [--allow-env <NAME>] [--allow-exec <PROGRAM>]
           [--memory-limit <BYTES|none>]
           [--instruction-limit <COUNT|none>]
           [--fail-open]
           <script.lua> [args…]
airsl test [--policy <trusted|confined|pure>] [--allow-…] [<path>]
airsl doctor [--policy <trusted|confined|pure>]
```

Arguments after the script path reach it in the global `arg` table — `arg[1]` where a shell script
read `$1`, and `arg[0]` for the script's own name. They are passed through untouched, including ones
beginning with `-`.

## `--policy`

What the script may reach, and what it may spend.

| Preset | Language surface | Ceilings | `require` |
|---|---|---|---|
| `trusted` | everything except `debug` — `io`, `os`, `package` included | none | Lua's own, unconfined |
| `confined` *(default)* | `string`, `table`, `math`, `utf8`, `coroutine`, pure `os` | 64 MiB, 100M instructions | confined to the script's directory |
| `pure` | `string`, `table`, `math`, `utf8` | 16 MiB, 10M instructions | none |

`trusted` is for first-party scripts only: one can read and write arbitrary files and spawn
processes without going through a host module, so none of the containment the host modules provide
applies to it.

A script under `confined` may `require` its siblings, including files in subdirectories
(`require("lib.index")`). It cannot name anything outside its own directory: a target may not
contain a path separator or a `..` component, and the resolved path is checked for containment, so a
symlink pointing out of the directory is refused too.

## Grant flags

Nothing is granted below `--policy trusted`. A script that reads a file, reads an environment
variable or runs a program says so on the command line, so the authority is visible to whoever reads
the invocation:

| Flag | Grants | Repeatable |
|---|---|---|
| `--allow-read DIR` | reading under `DIR` | yes |
| `--allow-write DIR` | writing under `DIR` — not implicitly readable | yes |
| `--allow-env NAME` | reading and setting the variable `NAME` | yes |
| `--allow-exec PROGRAM` | running `PROGRAM`, matched on the name as written | yes |

```bash
airsl run --allow-read "$AIRSSTACK_HOME" \
          --allow-write "$AIRSSTACK_HOME/journal/.index" \
          --allow-env AIRSSTACK_HOME \
          --allow-exec git \
          hooks/index.lua
```

A refusal names what was granted, because the usual cause is a root one directory too deep:

```
airsl: fs.read denied: `/etc/hostname` is outside them — granted read roots are /home/me/journal
```

Under `--policy trusted` these flags are ignored: that preset waives containment entirely, so a
declared list would narrow nothing and would make `airsl doctor` report something meaningless.

## `airsl test`

Runs the Lua test files under a directory, with the same policy and grant flags as `airsl run`.

```bash
airsl test --allow-read . plugins/
```

A test file is named `*_test.lua` or `test_*.lua` and returns a table whose named function values
are the tests. A test passes by returning and fails by raising, so Lua's own `assert` is the whole
assertion surface:

```lua
-- index_test.lua
return {
  joins_paths = function()
    assert(airsstack.path.join("a", "b") == "a/b")
  end,
}
```

Each file gets a fresh engine, so one file cannot leave globals behind for the next. Finding no test
files at all exits non-zero — "no tests" and "all tests passed" must not read the same to CI.

## `--memory-limit` and `--instruction-limit`

Override whatever the preset supplied. Pass `none` to lift a ceiling the preset imposed, or a count
to impose one it did not:

```bash
airsl run --policy pure --instruction-limit 500000 report.lua
airsl run --policy confined --memory-limit none big-index.lua
```

The instruction ceiling is the only thing that stops a script that never terminates — no policy
decision helps against `while true do end`, because it reaches nothing. It costs roughly a quarter of
the evaluation path, since the check runs inside the VM, which is why lifting it is available.

The memory ceiling caps the whole Lua state rather than each script, and the instruction ceiling is
enforced to within a check interval rather than exactly.

## `--fail-open`

Discards errors and exits 0.

This exists for scripts run as editor or agent hooks, where a non-zero exit is read as a signal
rather than a diagnostic. A `PreToolUse` hook that exits 2 blocks the tool call that triggered it,
and the matcher for such hooks commonly covers `Read` — so a script that merely failed would block
every file read in the session.

The flag lives on the command line rather than inside the script because a syntax error happens
before any in-script setting could take effect, and that is precisely the case the behaviour exists
for.

Set `AIRSL_DEBUG=1` to see the error on stderr anyway. The exit code stays 0.

**One exception, deliberately.** A script stopped for exhausting a memory or instruction ceiling is
always reported on stderr, `AIRSL_DEBUG` or not. The exit code still stays 0 — the fail-open
contract does not bend — but a hook consuming the host's memory or looping until it is killed is a
fact about the machine rather than a diagnostic the script chose to emit, and staying silent about
it makes a misbehaving hook impossible to find.

## Calling it from a hook

The binary is not ambient the way `sh` and `python3` are: until it has been installed, it is not
there. A launcher that checks first keeps a missing runtime silent rather than broken:

```sh
#!/bin/sh
DIR=$(CDPATH= cd -- "$(dirname -- "$0")" 2>/dev/null && pwd) || exit 0
[ -n "$DIR" ] || exit 0
command -v airsl >/dev/null 2>&1 || exit 0
airsl run --fail-open "$DIR/enforce.lua" || exit 0
exit 0
```

Every path exits 0, and `exec` is deliberately not used — it would replace the shell and hand the
child's exit status straight back to the caller.
