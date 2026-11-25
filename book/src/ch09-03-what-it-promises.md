# What airsl Can and Cannot Promise

A sandbox is only useful if you know exactly where its edges are. Here they are.

## What it does promise

- **A script reaches only what its policy allows.** Every module checks its grant in Rust before it
  acts, and Lua's own ways around the modules (`io`, `os.execute`, `load` and the rest) are removed
  below `trusted`.
- **A script cannot run forever or use unlimited memory**, as long as the limits are on.
- **You can see the whole surface.** `airsl doctor` and `engine.policy()` describe exactly what a
  script gets, and there is no way to add to an engine except through `HostModule`, which those
  reports include.
- **Starting a script is fast.** It takes microseconds, and extension authors need no toolchain.

## What `pure` guarantees

The `pure` preset is for calculations, and it makes a stronger promise than the others. Unless you
give it more, a `pure` script reaches no standard input, standard output or standard error, no
clock, no working folder, no file on the host and no time-zone database. It cannot print, because
`print` is not there, and it cannot seed its random numbers from the clock, because
`math.randomseed()` with no seed is refused.

So on a **fresh engine**, the same script given the same input produces the same output. Here is a
script that draws a random number:

```lua
-- roll.lua
airsstack.stdio.write(math.random(1, 1000000) .. "\n")
```

Under `pure`, every run draws the same number. The script needs `stdout` to write its answer, which
[chapter 3](ch03-01-nothing-by-default.md) explains. Under `confined`, the number changes each run:

```console
$ airsl run --policy pure --allow-access stdout roll.lua
275394
$ airsl run --policy pure --allow-access stdout roll.lua
275394
$ airsl run --policy confined roll.lua
693869
$ airsl run --policy confined roll.lua
561919
```

Each thing you grant opens its own route. A `pure` script that holds `clock` can read the time, so
its output can then change from run to run.

### Three things that can still differ

The guarantee has three exceptions. They come from the Lua interpreter itself, and `airsl` does
not control them:

- **`pairs` order.** Walking a table with `pairs` can visit string keys in a different order on
  each run. Collect the keys, sort them, and walk the sorted list.
- **Printed addresses.** `tostring` of a table or function, and `string.format("%p", value)`,
  show a memory address, which changes between runs.
- **`table.sort` ties.** When the comparison treats two elements as equal, they can come out in a
  different order on each run. A comparison that breaks every tie gives one order. Here, people of
  the same age are ordered by name:

```lua
-- sort.lua
local people = {
  { name = "Mia", age = 30 },
  { name = "Ben", age = 25 },
  { name = "Zoe", age = 30 },
  { name = "Ann", age = 25 },
}
table.sort(people, function(a, b)
  if a.age ~= b.age then return a.age < b.age end
  return a.name < b.name
end)
for _, p in ipairs(people) do
  airsstack.stdio.write(p.age .. " " .. p.name .. "\n")
end
```

```console
$ airsl run --policy pure --allow-access stdout sort.lua
25 Ann
25 Ben
30 Mia
30 Zoe
```

### Reusing an engine

The promise is for a fresh engine. An engine that runs several scripts keeps the globals they set,
and it keeps going along the same random sequence rather than starting it again. The second script
then sees a different number from the first. [Chapter 6](ch06-04-reuse.md) covers reuse. Use a new
engine when each run must stand alone.

### Granting `require`

A `pure` script can load its own sibling files once you grant `require`. Doing so shows the script
the absolute location of its folder. Each module receives its own absolute path as its second
argument, and error messages name it:

```lua
-- greeting.lua
local name, path = ...
airsstack.stdio.write("loading " .. name .. " from " .. path .. "\n")
return { hello = function(who) return "hello, " .. who end }
```

```lua
-- main.lua
local greeting = require("greeting")
airsstack.stdio.write(greeting.hello("world") .. "\n")
```

```console
$ airsl run --policy pure --allow-access stdout --allow-access require main.lua
loading greeting from /home/you/proj/greeting.lua
hello, world
```

If that location should stay private, do not grant `require`, or keep the scripts in one file.

## What it does not promise

- **No protection from a bug in a module.** The sandbox lives at the boundary between Lua and Rust.
  If a module's Rust code has a bug, or Lua's own C code does, that bug runs inside your program with
  your program's full rights. A grant is a promise the module keeps, and there is nothing underneath
  to catch a module that breaks it.
- **No protection from a malicious module.** A module you install can do anything Rust can do.
  Install only modules you trust.
- **No separation of CPU or memory between scripts on one engine.** Scripts that share an engine
  share its state, as [chapter 6](ch06-04-reuse.md) described.

## Compared with WebAssembly

WebAssembly sandboxes at a deeper level. A WASM module runs in its own memory, so even a buggy host
function cannot let it read the host's memory. Lua under `airsl` cannot offer that.

WASM also lets extension authors use any language that compiles to it. The price is a compiler
toolchain for every author, a heavier runtime, and a much more awkward boundary for passing data
back and forth.

| | `airsl` (Lua) | WebAssembly |
|---|---|---|
| Where the sandbox is | the API boundary | the VM boundary |
| Survives a buggy host function | no | yes |
| Author needs | a text editor | a compiler toolchain |
| Start-up | microseconds | heavier |
| Passing data | Lua tables and JSON | bytes in linear memory |

So the honest advice is this. For extensions you write yourself, review, or accept from a registry
you control, Lua under `airsl` is a good trade, and Redis has shown it works for years. For running
genuinely hostile code from strangers, WebAssembly is the stronger choice.

## What is not built yet

A few things are known gaps rather than design choices:

- **Isolation between scripts on a reused engine.** Global variables a script sets, and changes it
  makes to shared tables such as `string`, stay visible to the next script on the same engine. Until
  that is built, give scripts that do not trust each other separate engines.
- **`proc` cannot set a working folder, standard input or extra environment for one call.** Scripts
  work around it, for example with `git -C <dir>`.
- **Exit codes other than 0 and 1.** `os.exit` is withheld below `trusted`, so a script cannot choose
  its own exit code.
- **JSON `null` and empty arrays cannot be built from Lua.** They can only come from decoding JSON.
- **Future modules**: `http`, `sqlite`, cryptography beyond hashing, and bundling a script with its
  modules into one file are all planned, and none exists yet.
