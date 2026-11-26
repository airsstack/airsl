---
status: done
created: 2026-11-25
milestone: lua-ecosystem/02
---

# Intent: `require("<root>")` returns the root table

## Problem

A script reaches the host modules only through the global root table (`airsstack` by default, or
the name a host chose). `local airsstack = require("airsstack")`, the form ordinary Lua code uses
to get a library, fails in airsl under every preset, and each preset fails differently:

| Preset / case | What `require("airsstack")` does today |
|---|---|
| `trusted` | Lua's own `require` searches `package.path`: a file named `airsstack.lua` is loaded, otherwise an error |
| `confined`, script on disk | the confined loader looks for `airsstack.lua` in the script's folder: a user file is loaded, otherwise an error |
| `pure` (no `require` grant) | the refusing `require` refuses every call, without reading the name |
| `require` granted, script from source | there is no `require` at all, so the call fails on `nil` |

So a script cannot use the one form that would also work in stock Lua once the native module
exists (milestone `lua-ecosystem/03`). A user file that happens to share the root's name is
silently loaded in place of the host modules.

Editors have the same gap. The type file from milestone 01 makes the global known to LuaLS, but a
local bound through `require("airsstack")` gets no types and no autocomplete.

## Affected systems

- `crates/airsl`: the engine's choice of `require` for each surface and grant, the confined
  loader, the refusing `require`, and the `require`-free case for scripts built from source. Hosts
  that rename the root and the extension path (`ExtensionHost`) run through the same engine.
- `crates/airsl`'s LuaLS type file, and so what `airsl lua setup` writes.
- `crates/airsl-cli` (`run`, `test`, `ext fire`): behaviour only, no new commands or flags.
- The airsl Book (the `require`, presets and editor-support chapters) and the
  `multi-file-project` example.

## Desired outcome

- `require("<root>")` returns the root table under every preset (`trusted`, `confined`, `pure`)
  and every custom policy, for a script on disk or built from source, with or without the
  `require` grant.
- `<root>` is the engine's own root name. Under a host whose root is `agentstacks`,
  `require("agentstacks")` returns its root, and `require("airsstack")` is an ordinary lookup.
- The root name wins over a user file with the same name: a file `airsstack.lua` beside the
  script is never loaded by `require("airsstack")`. The one exception is a `trusted` script that
  removes or replaces `package.loaded.airsstack` itself, which then gets stock Lua's behaviour.
- Scripts that use the global keep working unchanged. The global and `require("<root>")` reach
  the same modules.
- Every other `require` call behaves as it does today: the confined loader and its containment,
  the refusal that names the granted access, Lua's own loader under `trusted`.
- In an editor set up with `airsl lua setup`, a local bound with `require("<root>")` has the same
  types and autocomplete as the global.
- The book shows the `require` form and states what each preset does with it.

## Constraints

- Getting the root table is not host access. It needs no `require` grant, because the global
  already hands the same table to every script. This replaces 0.2.0's documented rule that the
  refusing `require` refuses every call without reading its argument. Every other name is still
  refused there.
- No new authority anywhere. Under a preset that refuses `require`, only the root name is
  answered, and no file is read.
- Enforcement stays in Rust. The sandbox does not rely on a Lua-side `package.loaded` or
  `package.preload` a script could change.
- Byte-stable, deterministic output from the type file, as in milestone 01.
- Every Lua and LuaLS behaviour the spec relies on is probed against mlua 0.12.0 (Lua 5.4) and a
  named LuaLS version.
- Candidate direction (for `design`): check the root name before any other resolution in each
  `require` airsl installs, and give the type file a `return <root>`.

## Non-goals

- Removing or deprecating the global root table.
- `require("<root>.<module>")` returning a module's subtable. Only the bare root name is special;
  `require("airsstack.fs")` stays an ordinary lookup.
- The native module, LuaRocks and `airsl lua setup --native` (milestones 03–05).
- Typing a project's own Lua files or modules it loads through `require`.
- A release: none until the `lua-ecosystem` roadmap is complete.
