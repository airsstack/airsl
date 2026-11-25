---
status: approved
created: 2026-11-25
---

# Milestone: Native module and rock

## Goal

`crates/airsl-lua` builds `airsstack.so` for Lua 5.4 and LuaJIT (all modules except `ext`, trusted
grants), `luarocks install airsstack` builds it from source, and cargo-make and CI cover both
versions. The LuaJIT JSON difference is documented.

## Non-goals

- Prebuilt binaries.
- Sandboxing.
