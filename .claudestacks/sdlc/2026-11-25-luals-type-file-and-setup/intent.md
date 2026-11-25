---
status: approved
created: 2026-11-25
milestone: lua-ecosystem/01
---

# Intent: LuaLS type file and `airsl lua setup`

## Problem

A script opened in an editor with the Lua language server (LuaLS) reports every use of the root
table as `Undefined global` (for example `agentstacks.require(...)` under a host whose root is
`agentstacks`). The root and its modules are installed by the host at run time, so LuaLS has no
way to know they exist. Engineers get no autocomplete, no signatures and no hover docs for any
`airsstack` module, and a page of false warnings hides real ones.

Probed with LuaLS 3.19.2: the warning is a missing definition, not a parse error. A `---@meta`
file on `workspace.library`, listed in `.luarc.json`, makes it go away and gives autocomplete.

## Affected systems

- `crates/airsl`: the twelve built-in modules and the `HostModule` extension seam. A host that
  installs its own modules or renames the root must be able to get a type file that matches what
  its engine installs.
- `crates/airsl-cli`: a new `airsl lua setup` command that writes the type file and the LuaLS
  config into a project.
- The gate (`cargo make dod`) and The airsl Book.

## Desired outcome

- A type file describes every module under a root: the twelve built-ins and any module a host
  installs, under whatever root name the engine uses.
- `airsl lua setup` writes the type file for the default `airsstack` root and a `.luarc.json`
  that points LuaLS at it. After that, LuaLS reports no undefined-global warning for the root,
  and every module autocompletes with its functions.
- When `.luarc.json` already exists, setup keeps every setting the user wrote and adds only what
  airsl needs. Running setup again changes nothing unless airsl's modules changed.
- `cargo make dod` fails when a module's Lua surface and its type description disagree. A type
  file always matches the airsl version that wrote it.
- The book tells a reader how to set up their editor.

## Constraints

- Types describe the Lua surface only. They grant nothing and do not depend on the policy: a
  module is always present, even ungranted, so it is always typed.
- Output is deterministic (stable order, byte-stable across runs), like the rest of airsl.
- Setup needs no network and does not require LuaLS to be installed.
- Every LuaLS behaviour the spec relies on is probed against a named LuaLS version.
- Candidate direction (for `design`): each module describes its own surface next to its code, so
  the generator and the drift check read one source.

## Non-goals

- `require("<root>")` inside airsl (milestone `lua-ecosystem/02`).
- The native module, LuaRocks, and `airsl lua setup --native` (milestones 03–05).
- Configuring a particular editor beyond `.luarc.json` (Neovim, VS Code plugins, and so on).
- Typing a project's own Lua files or the modules a script loads through `require`.
