# airsl-cli

The `airsl` binary: runs Lua scripts on the [`airsl`](../airsl) runtime.

## Install

```bash
cargo install --path crates/airsl-cli --force
airsl doctor
```

`doctor` prints the runtime version, the Lua version, and the host modules scripts can see:

```
airsl 0.1.0
  lua:     Lua 5.4
  sandbox: restricted
  modules: json
```

## Usage

```
airsl run [--fail-open] [--unrestricted] <script.lua> [args…]
airsl doctor
```

Arguments after the script path reach it in the global `arg` table — `arg[1]` where a shell script
read `$1`. They are passed through untouched, including ones beginning with `-`.

## `--fail-open`

Discards every error and always exits 0.

This exists for scripts run as editor or agent hooks, where a non-zero exit is read as a signal
rather than a diagnostic. A `PreToolUse` hook that exits 2 blocks the tool call that triggered it,
and the matcher for such hooks commonly covers `Read` — so a script that merely failed would block
every file read in the session.

The flag lives on the command line rather than inside the script because a syntax error happens
before any in-script setting could take effect, and that is precisely the case the behaviour exists
for.

Set `AIRSL_DEBUG=1` to see the error on stderr anyway. The exit code stays 0.

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
