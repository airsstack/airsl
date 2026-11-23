---
status: approved
created: 2026-11-23
---

# Intent: A required module receives its own name

## Problem

Tracked as [airsstack/airsl#17](https://github.com/airsstack/airsl/issues/17).

The confined `require` runs a module's chunk with no arguments, so `...` is `nil` inside every
required module. Standard Lua 5.4 passes the module name: "Once a loader is found, require calls
the loader with two arguments: modname and an extra value" (Lua 5.4 manual, §6.3).

Every `require` in one run resolves against the entry script's directory. Without its own name, a
module cannot require its siblings by a relative name, and the standard idiom for doing so fails:

```lua
local base = (...):match("^(.-)[^.]*$")
local lib = require(base .. "lib.x")
```

So one module cannot be loaded both ways it is actually used: through an entry script
(`airsl run`, root = the entry's directory) and under `airsl test` (root = the test file's
directory). A plain `require("lib.x")` works in one and fails in the other.

Reproduced on this checkout (airsl 0.1.4, 2026-11-23), with `R/main.lua` requiring
`core.hooks.enforce`, which uses the idiom above to require `core/hooks/lib/x.lua`:

```text
$ airsl run --policy confined R/main.lua
airsl: lua error in .../r17/main.lua: lua error in core.hooks.enforce: runtime error: .../r17/core/hooks/enforce.lua:1: attempt to index a nil value
```

Why now: agentstacks runs pack hooks as modules through one runtime script
(`require("<pack>.hooks.<name>")`). Those hooks must require their own libraries both when
installed and under `airsl test`. agentstacks will depend on the first airsl release that fixes
this.

## Affected systems

- `crates/airsl/src/require_loader.rs`: how a resolved module's chunk is run.
- `crates/airsl-cli`: no code change expected, but it ships the binary agentstacks hooks run on,
  so it must be released together with the library.
- The airsl Book: the chapters that describe `require` (`ch02-00-first-project.md` at least).
- Release: `airsl` and `airsl-cli` versions, both at 0.1.4 today.
- Downstream: agentstacks (`airsl = "0.1.4"` today), which bumps after the release.

## Desired outcome

- Inside a module loaded by the confined `require`, `...` evaluates to the module name the
  caller passed, followed by the module's path, as Lua 5.4's own `require` passes them, so the
  sibling-require idiom above works under `airsl run` and `airsl test` alike.
- `require` still returns one value, the module's, so a `require` call in a multi-value position
  (a table constructor's last field, a call's last argument) behaves as on 0.1.4. A module that
  returns nothing yields `true` on its first `require` as on every later one (0.1.4 returns `nil`
  the first time).
- `airsl` and `airsl-cli` 0.1.5 are published together, so agentstacks can require
  `airsl >= 0.1.5` and its hooks run on a binary that has the fix.
- The book tells a module author that `...` holds the module name, with a run example.

## Constraints

- **No regression for modules that work on 0.1.4.** Today a module is evaluated
  expression-first: a file holding only an expression with no `return` (for example
  `{ v = "bare" }`) loads and returns that value. Probed on this checkout: `airsl run` of a
  script requiring such a module prints `bare`. The fix the issue proposes (switching to a plain
  call) turns that file into a syntax error; probed against mlua 0.12.0, "call bare: Err(syntax
  error: ... unexpected symbol near '{')". That behaviour must keep working.
- Containment, caching (one load per canonical path), cycle detection and error wrapping stay as
  they are.
- The surface rules (`RequireDisposition`) are untouched. `Full` keeps Lua's own `require`.
- `airsl` and `airsl-cli` publish together (org rule: the binary's lockfile pins the library).
- 0.1.5 is a patch release: the change must be additive. A module that ignores `...` behaves as
  before.
- Every claim about mlua or Lua behaviour is proven with a probe against mlua 0.12.0 (the
  version in `Cargo.lock`), the raw output saved in this chain.

## Non-goals

- `require` under the `pure` / `Minimal` surface ([#12](https://github.com/airsstack/airsl/issues/12)).
  That needs its own design decision and gets its own chain.
- Changing what `require` resolves against (the entry script's directory) or adding
  module-relative resolution to the loader itself.
- Lua's own `require` under `Full`, which already passes its arguments.
- Hardening the chunk mode (text vs. binary) of required modules.
- Bumping agentstacks. That happens in the agentstacks repo after 0.1.5 is published.

Settled in `design` (2026-11-23): the chunk also receives the file path as Lua's second loader
argument, and one file reached under two module names still loads once (the cache is keyed by
path), seeing the first name. `require` does not return the path as a second value: stock Lua's
second value spills into multi-value positions and breaks scripts that work on 0.1.4.
