---
status: approved
created: 2026-11-25
---

# Milestone: Native module and rock

## Goal

`crates/airsl-lua` builds the `airsstack` native module for Lua 5.4 and LuaJIT: every module except
`ext`, trusted grants, returned by `require("airsstack")` with no global set. Two LuaRocks packages
on luarocks.org carry it: `airsstack` (Lua 5.4) and `airsstack-luajit` (LuaJIT). Each installs
from source with cargo, and the roadmap's release uploads prebuilt binary rocks per platform for
both, so installing needs no Rust toolchain where a binary exists. `airsl lua setup --native`
installs the package matching the Lua that luarocks targets, at the running airsl's version, or
prints the command when luarocks is missing. cargo-make and CI build and test both Lua versions.
The LuaJIT JSON difference and the missing memory ceiling outside airsl are documented.

This milestone absorbs `04-binary-rocks` and `05-setup-native` (merged 2026-11-26).

## Non-goals

- Sandboxing, grants or resource limits outside airsl.
- Platforms airsl itself does not release for, and musl rocks.
- Installing without LuaRocks.
- A rocks server of our own: luarocks.org only.
