# Failing Safely

Scripts often run on behalf of something else: a git hook, an editor, an AI coding agent. Those
callers read a script's exit code as a decision. For an agent's `PreToolUse` hook, for example, a
non-zero exit can block the action the hook was watching.

That means a script with a typo in it does not just fail. It blocks whatever it was attached to.
If the hook watches every file read, a single syntax error stops the agent from reading any file at
all.

## `--fail-open`

`--fail-open` tells `airsl` that a failure must not escape:

```console
$ echo 'this is not lua' > broken.lua

$ airsl run broken.lua; echo "exit=$?"
airsl: lua error in broken.lua: syntax error: broken.lua:1: syntax error near 'is'
exit=1

$ airsl run --fail-open broken.lua; echo "exit=$?"
exit=0
```

With `--fail-open`, any error is swallowed and `airsl` exits with `0`. A broken hook does nothing,
instead of breaking the session it runs in.

It is a command-line flag rather than something you write in the script for a simple reason. A
syntax error stops Lua before the script's first line runs, so a setting inside the script would
never be read in exactly the case it exists for.

## Seeing the error anyway

Silence is right for the caller, but you still need to debug the script. Set `AIRSL_DEBUG=1` to
print the error while keeping the exit code at `0`:

```console
$ AIRSL_DEBUG=1 airsl run --fail-open broken.lua; echo "exit=$?"
airsl: lua error in broken.lua: syntax error: broken.lua:1: syntax error near 'is'
exit=0
```

## Limits are always reported

There is one thing `--fail-open` does not hide. If a script is stopped for hitting a memory or
instruction limit, `airsl` always prints it:

```console
$ airsl run --fail-open forever.lua; echo "exit=$?"
airsl: script `forever.lua` exceeded its instruction limit of 100000000
exit=0
```

The exit code is still `0`, so the caller is still protected. But a hook that eats memory or loops
until it is stopped is a problem with your machine, not a message the script chose to send, and
keeping quiet about it would leave you with a slow session and no idea why.

## Catch syntax errors before they ship

`--fail-open` keeps a broken hook harmless, but it also keeps it quiet. A typo can ship and nobody
notices that the hook stopped working. `airsl check` catches that ahead of time. It compiles every
`.lua` file under a folder without running any of them. In a folder holding `forever.lua` and
`broken.lua`:

```console
$ airsl check .
2 file(s) compiled, 1 failed

./broken.lua
    lua error in ./broken.lua: syntax error: ./broken.lua:1: syntax error near 'is'
```

Run it in CI next to `airsl test`. Tests cover the modules your hook uses, but the hook's own entry
script is usually not loaded by any test, and `check` is what covers it.

## Summary

- Every `confined` script has a memory limit and an instruction limit. Change them with
  `--memory-limit` and `--instruction-limit`, or remove them with `none`.
- `--fail-open` makes any failure exit with `0`, for scripts that run on behalf of another program.
- `AIRSL_DEBUG=1` shows the error anyway, and hitting a limit is always shown.
- `airsl check` finds syntax errors without running anything.
