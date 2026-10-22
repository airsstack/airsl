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
