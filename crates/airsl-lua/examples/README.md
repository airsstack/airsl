# Examples

Runnable scripts for the native module, the one that LuaRocks installs as `airsstack` for Lua 5.4
and `airsstack-luajit` for LuaJIT, and that `require("airsstack")` loads in stock Lua. Each example
is a directory with a `main.lua` and its own `README.md`, which shows what it prints.

These are scripts for the Lua you already have. The examples of the `airsl` program and of the
Rust library are in [`crates/airsl/examples`](../../airsl/examples/).

## Install

```bash
luarocks --lua-version 5.4 install --local airsstack
luarocks --lua-version 5.1 install --local airsstack-luajit
```

Install the package for the Lua you run. Then tell your shell where LuaRocks put it:

```bash
eval "$(luarocks --lua-version 5.4 path)"   # Lua 5.4
eval "$(luarocks --lua-version 5.1 path)"   # LuaJIT
```

[Installing](https://airsstack.github.io/airsl/ch01-05-stock-lua.html#installing) has the Windows
forms, and `airsl lua setup --native` does the install for you.

Run one from its own directory:

```bash
cd common/hello-require
lua5.4 main.lua
luajit main.lua
```

## Run from a checkout

An example does not set `package.cpath`; the interpreter finds the module the way it would for a
reader who installed the rock. From a checkout nothing is installed, so build the module and point
`LUA_CPATH` at it. The `;;` at the end keeps the interpreter's own search path:

```bash
cargo make native-load
LUA_CPATH="$PWD/target/native/lua54/?.so;;" lua5.4 crates/airsl-lua/examples/common/hello-require/main.lua
LUA_CPATH="$PWD/target/native/luajit/?.so;;" luajit crates/airsl-lua/examples/common/hello-require/main.lua
```

A module built for one Lua does not load into the other, so the directory has to match the
interpreter: `lua54` for Lua 5.4, and `luajit` for LuaJIT and for Neovim.

## The groups

The directory an example is in says which interpreter runs it.

### `common/` — the same under Lua 5.4 and LuaJIT

One `## Output` block, and both interpreters print it.

| Example | What it shows |
| --- | --- |
| [`hello-require`](common/hello-require/) | What `require("airsstack")` returns: the module names, no global, the same table every time. |
| [`files-and-paths`](common/files-and-paths/) | `fs`, `path` and `glob` on a scratch directory that is removed at the end. |
| [`text-and-hashing`](common/text-and-hashing/) | `regex` with alternation and named groups, and `hash` digests. |

### `lua54/` — Lua 5.4 only

| Example | What it shows |
| --- | --- |
| [`integers-and-json`](lua54/integers-and-json/) | An integer and a float, kept apart through `json`. |

### `luajit/` — LuaJIT only

| Example | What it shows |
| --- | --- |
| [`numbers-and-json`](luajit/numbers-and-json/) | What `json` gets from a Lua with one number type. |

### `neovim/` — Neovim, which embeds LuaJIT

| Example | What it shows |
| --- | --- |
| [`require-in-nvim`](neovim/require-in-nvim/) | Putting the rock on `package.cpath` and using `json`, `hash` and `path` from `nvim -l`. |

## Conventions these examples follow

- **Output is deterministic and ordered.** No timestamps, no absolute paths, no temporary names.
  `print` and `io.write` share one buffer, so mixing them keeps their order, but
  `airsstack.stdio.write` is a buffer of its own, and under LuaJIT a run piped into a file can
  reorder its lines against theirs. So an example never writes with `airsstack.stdio.write`, and
  the book's [How it differs from `airsl`](https://airsstack.github.io/airsl/ch01-05-stock-lua.html#how-it-differs-from-airsl)
  shows the reordering. Under `nvim -l`, `print` writes to standard error, which the comparison
  does not read, so the Neovim example writes with `io.write` and the rest with `print`.
- **Nothing is written inside the repository.** An example that needs a directory makes a temporary
  one and removes it.
- **The gate keeps the `## Output` blocks true.** `cargo make dod` runs each `common` example under
  both interpreters and each `lua54` and `luajit` example under its own, through the `docs-check`
  crate, and fails unless what it printed is exactly the block. A missing interpreter or an unbuilt
  module fails it with the fix named; it never skips. `AIRSL_LUA54` and `AIRSL_LUAJIT` name the
  interpreters, the same variables `cargo make native-load` reads.
- **Neovim is checked on its own.** `cargo make dod` needs no editor, so the Neovim example is
  compared by `cargo make nvim-examples`, which CI runs on Linux and macOS. `AIRSL_NVIM` names the
  Neovim, `nvim` by default.

## See also

- [Running Scripts in Stock Lua](https://airsstack.github.io/airsl/ch01-05-stock-lua.html) — what the
  native module is, the two packages, and how it differs from `airsl`.
- [Module Reference](https://airsstack.github.io/airsl/appendix-02-modules.html) — every function a
  script can call.
