# What Does a Script Get?

Before `airsl` runs a script, it decides what that script may do. You can ask it what that decision
is:

```console
$ airsl doctor
airsl 0.1.4
  lua:          Lua 5.4
  language:     restricted
  root table:   airsstack
  grants:       none
  memory:       67108864 bytes
  instructions: 100000000 instructions
  modules:      json, path, fs, env, proc, regex, hash, time, glob, stdio, hook, ext
```

Read the last four lines. They are the whole idea behind `airsl`.

```
  grants:        none                  ← may touch nothing outside itself
  memory:        64 MiB                ← may use this much memory
  instructions:  100 million           ← may run this long
  modules:       json, path, fs, …     ← has these tools available
```

A script gets a full set of tools, but **no permission** to use the ones that reach outside the
script. It can parse JSON, match regular expressions and build file paths, because none of that
touches anything. But it cannot read a file, see an environment variable, or run a program until you
say it may.

It also gets two limits. If a script uses more than 64 MiB of memory, or runs more than a hundred
million instructions, `airsl` stops it. A script with a bug that loops forever cannot hang your
terminal.

## Three presets

What `doctor` just showed is the default preset, called `confined`. There are two others:

```console
$ airsl doctor --policy trusted
airsl 0.1.4
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
| `confined` *(default)* | everyday scripts | only what you grant | 64 MiB, 100M instructions |
| `pure` | calculations with no I/O at all | only what you grant | 16 MiB, 10M instructions |

You will meet all three properly in [chapter 3](ch03-05-presets.md). For now, the default is the one
to get used to.

## Summary

- `airsl run script.lua` runs a script, and `arg[1]` reads its first argument.
- `airsl doctor` shows what a script would be allowed to do.
- By default, a script has tools but no permissions, and it has limits on memory and running time.

In the next chapter you will write a script that needs a permission, watch it get refused, and then
grant it.
