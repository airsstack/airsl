# Presets

Grants decide what a script may touch through `airsstack`. A **preset** decides everything around
that: which parts of Lua itself the script can use, and how much memory and time it may spend.
Choose one with `--policy`.

## `confined`: the default

This is what every example so far has used. The script gets the everyday parts of Lua (`string`,
`table`, `math`, `utf8`, `coroutine`, and the harmless parts of `os` such as `os.time`) plus the
`airsstack` modules. It gets no grants until you add them, and it runs under a 64 MiB memory limit
and a 100 million instruction limit.

What it does **not** get are the parts of Lua that reach outside the script without asking:

- `io`, which opens files directly
- `os.execute`, `os.remove`, `os.getenv` and the other `os` functions that touch the system
- `load`, `dofile` and the other functions that run code from a string or file
- Lua's own `require`, which is replaced by the confined one you met in chapter 2

Each of those has an `airsstack` replacement that asks for a grant first.

## `trusted`: for your own scripts

```console
$ airsl run --policy trusted script.lua
```

A trusted script gets all of Lua, including `io` and `os`, and every `airsstack` module with no
restrictions. There are no limits on memory or time. The grant flags are ignored, because there is
nothing left to restrict.

Use it for scripts you wrote yourself and would happily run as a shell script. It is also the easy
first step when you move existing scripts onto `airsl`: run them as `trusted` to start with, then
tighten them to `confined` one grant at a time.

## `pure`: no I/O at all

```console
$ airsl run --policy pure script.lua
```

A pure script gets only `string`, `table`, `math` and `utf8`. It has no `os`, so it cannot even read
the clock, and no `require`. Its limits are tighter: 16 MiB of memory and 10 million instructions.

Use it to evaluate something that should be a pure calculation, such as a configuration file written
in Lua, or an expression a user typed in. Given the same input, a pure script always produces the
same output.

## Side by side

| | `trusted` | `confined` | `pure` |
|---|---|---|---|
| Lua's `io`, `os.execute`, `load` | yes | no | no |
| `os.time`, `os.clock` | yes | yes | no |
| `require` | Lua's own | script's folder only | none |
| `airsstack` modules | unrestricted | only what is granted | only what is granted |
| Memory limit | none | 64 MiB | 16 MiB |
| Instruction limit | none | 100 million | 10 million |

## Summary

- There are four kinds of grant: read a folder, write a folder, use a variable, run a program.
- Each grant names exactly what it covers.
- A refusal is always an error that names what was missing.
- Presets decide which parts of Lua a script sees, and how much it may spend.

The next chapter looks at those spending limits, and at what happens when a script fails.
