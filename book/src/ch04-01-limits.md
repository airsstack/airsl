# Memory and Time Limits

Here is the smallest possible runaway script:

```lua
-- forever.lua
while true do end
```

It touches nothing, so no grant could stop it. Without a limit it would hang your terminal until you
killed it. Under `airsl` it stops on its own:

```console
$ airsl run forever.lua
airsl: script `forever.lua` exceeded its instruction limit of 100000000
```

## Two limits

Every `confined` script runs under two limits:

| Limit | Default | What it stops |
|---|---|---|
| instructions | 100 million | a loop that never ends |
| memory | 64 MiB | a script that keeps allocating |

An instruction is one step of the Lua virtual machine. A hundred million is far more than an
ordinary script needs. It is a ceiling, not a target.

Change either limit with a flag. Pass a number to set it, or `none` to remove it. This script keeps
growing a table until something stops it:

```lua
-- hungry.lua
local t = {}
for i = 1, 1e9 do
  t[i] = ("x"):rep(1024) .. i
end
```

```console
$ airsl run --instruction-limit 1000000 forever.lua
airsl: script `forever.lua` exceeded its instruction limit of 1000000

$ airsl run --memory-limit 10000000 hungry.lua
airsl: script `hungry.lua` exceeded its memory limit of 10000000 bytes

$ airsl run --memory-limit none big-report.lua
```

## What the limits promise, exactly

The limits are close, not exact. `airsl` checks the instruction count every ten thousand
instructions, so a script may run a little past its limit before it is stopped. That is plenty of
precision to catch a runaway loop, and checking more often would slow every script down.

The memory limit covers the whole Lua state. When you run one script per command, that is the same
thing as the script. When a Rust program runs many scripts on the same engine
([chapter 6](ch06-04-reuse.md)), memory left behind by earlier scripts counts until Lua's garbage
collector frees it.

## A limit is not a bug in the script

When a script hits a limit, `airsl` reports it differently from an ordinary error. An ordinary error
means "the script has a problem". Hitting a limit means "the script was stopped from harming the
machine", which you almost always want to know about. The next page shows why that difference
matters.
