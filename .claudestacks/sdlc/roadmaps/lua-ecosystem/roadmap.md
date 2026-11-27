---
status: done
created: 2026-11-25
---

# Roadmap: Lua ecosystem

## Vision

Scripts written for airsl should feel like ordinary Lua to the
engineer writing them. Today `airsstack` exists only inside airsl:
editors flag it as an undefined global, `require("airsstack")` fails,
and nothing outside airsl can load the modules.

This roadmap makes `airsstack` a first-class part of the Lua
ecosystem:

- editors and LuaLS understand every module (generated type file);
- one script form, `local airsstack = require("airsstack")`, works in
  airsl and in stock Lua, and existing scripts using the global keep
  working;
- the modules ship as a native Lua module (`airsl-lua`) on LuaRocks for
  Lua 5.4 and LuaJIT, so a script can run or be debugged without airsl;
- airsl itself sets this up for an engineer, version-matched to the
  airsl they run.

Outside airsl the modules run with full authority: the sandbox is
airsl's, and the native module does not pretend otherwise.

## Non-goals

- No sandbox outside airsl: the native module never enforces grants or resource limits; it runs
  with full authority.
- No debugger in airsl: no `debug` library, no DAP server, no `unsafe` exception. Debugging happens
  in stock Lua through the native module.
- airsl itself is not published on LuaRocks: only the `airsstack` native module is a rock; airsl
  keeps crates.io and its installers.
- No `ext` outside airsl: the native module leaves `ext` out, with no stand-in.
