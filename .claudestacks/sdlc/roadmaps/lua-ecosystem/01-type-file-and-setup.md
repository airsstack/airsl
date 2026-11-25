---
status: approved
created: 2026-11-25
---

# Milestone: Type file and setup

## Goal

airsl generates a LuaLS type file for every module under the default root (or a host's root), and
`airsl lua setup` writes it with a `.luarc.json`. Undefined-global warnings are gone and modules
autocomplete.

## Non-goals

- The native module.
- LuaRocks.
