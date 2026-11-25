# What Does a Script Get?

Before `airsl` runs a script, it decides what that script may do. You can ask it what that decision
is:

```console
$ airsl doctor
airsl <version>
  lua:          Lua 5.4
  language:     restricted
  root table:   airsstack
  grants:       access stdin,stdout,stderr,clock,cwd,require,zones
  memory:       67108864 bytes
  instructions: 100000000 instructions
  modules:      json, path, fs, env, proc, regex, hash, time, glob, stdio, hook, ext
```

Read the last four lines. They are the whole idea behind `airsl`.

```
  grants:        access stdin,stdout,…  ← may use its standard streams, the clock and the like,
                                           but no folders, variables or programs
  memory:        64 MiB                 ← may use this much memory
  instructions:  100 million            ← may run this long
  modules:       json, path, fs, …      ← has these tools available
```

A script gets a full set of tools, but **no permission** to use the ones that reach files, variables
or programs. It can parse JSON, match regular expressions and build file paths, because none of that
touches anything. It can write to its standard streams and read the clock. But it cannot read a file,
see an environment variable, or run a program until you say it may.

It also gets two limits. If a script uses more than 64 MiB of memory, or runs more than a hundred
million instructions, `airsl` stops it. A script with a bug that loops forever cannot hang your
terminal.

## Three presets

What `doctor` just showed is the default preset, called `confined`. There are two others:

```console
$ airsl doctor --policy trusted
airsl <version>
  lua:          Lua 5.4
  language:     full
  root table:   airsstack
  grants:       unrestricted
  memory:       unlimited
  instructions: unlimited
  modules:      json, path, fs, env, proc, regex, hash, time, glob, stdio, hook, ext
```

| Preset | Meant for | Permissions | Limits |
|---|---|---|---|
| `trusted` | scripts you wrote and fully trust | everything | none |
| `confined` *(default)* | everyday scripts | host access, plus whatever else you grant | 64 MiB, 100M instructions |
| `pure` | calculations with no I/O at all | only what you grant | 16 MiB, 10M instructions |

You will meet all three properly in [chapter 3](ch03-05-presets.md). For now, the default is the one
to get used to.

## Grants on the command line

`doctor` takes the same preset and grant flags as `run`, so it shows exactly what `run` would build
from them. Ask about `pure` and it reports no grants at all:

```console
$ airsl doctor --policy pure
airsl <version>
  lua:          Lua 5.4
  language:     minimal
  root table:   airsstack
  grants:       none
  memory:       16777216 bytes
  instructions: 10000000 instructions
  modules:      json, path, fs, env, proc, regex, hash, time, glob, stdio, hook, ext
```

Add `--allow-access` to give a `pure` script one kind of host access, here the standard error stream:

```console
$ airsl doctor --policy pure --allow-access stderr
airsl <version>
  lua:          Lua 5.4
  language:     minimal
  root table:   airsstack
  grants:       access stderr
  memory:       16777216 bytes
  instructions: 10000000 instructions
  modules:      json, path, fs, env, proc, regex, hash, time, glob, stdio, hook, ext
```

[Nothing Is Allowed by Default](ch03-01-nothing-by-default.md) lists the kinds of host access, and
the [command-line reference](appendix-01-cli.md) lists every flag.

## Summary

- `airsl run script.lua` runs a script, and `arg[1]` reads its first argument.
- `airsl doctor` shows what a script would be allowed to do.
- `airsl doctor --policy pure --allow-access stderr` shows the policy for the flags you give it,
  without running anything.
- By default, a script has tools but no permission to reach files, variables or programs, and it has
  limits on memory and running time.

In [A First Project](ch02-00-first-project.md) you will write a script that needs a permission,
watch it get refused, and then grant it. First, [Editor Support](ch01-04-editor-support.md) sets
up your editor to complete `airsstack` calls.
