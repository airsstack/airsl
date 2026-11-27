# hello-require

Loads the native module and looks at what `require` returned. It runs unchanged under Lua 5.4 and under LuaJIT and prints the same thing under both.

## Install

```bash
luarocks --lua-version 5.4 install --local airsstack
luarocks --lua-version 5.1 install --local airsstack-luajit
```

Install the package for the Lua you run: `airsstack` for Lua 5.4 and `airsstack-luajit` for LuaJIT.
Then tell your shell where LuaRocks put it, as
[Installing](https://airsstack.github.io/airsl/ch01-05-stock-lua.html#installing) describes:

```bash
eval "$(luarocks --lua-version 5.4 path)"   # Lua 5.4
eval "$(luarocks --lua-version 5.1 path)"   # LuaJIT
```

## Run

```bash
lua5.4 main.lua
luajit main.lua
```

From a checkout, build the module once and put it on the interpreter's search path:

```bash
cargo make native-load
LUA_CPATH="$PWD/target/native/lua54/?.so;;" lua5.4 crates/airsl-lua/examples/common/hello-require/main.lua
```

For LuaJIT, use `target/native/luajit` and `luajit` in place of those two.

## Output

```
modules: env, fs, glob, hash, hook, json, path, proc, regex, stdio, time
ext: nil
global airsstack: nil
package.loaded holds the same table: true
a second require returns it again: true
{"name":"airsl","numbers":[1,2.5]}
src/main.lua
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
```

Both interpreters print exactly this; the gate runs the example under each and requires it.

## What it demonstrates

- **One table, no global.** `require("airsstack")` returns the table of modules and sets nothing else, so the script keeps it in a local. `package.loaded` holds the same table, which is why a second `require` is free.
- **Every module but `ext`.** `ext` only exists inside the `airsl` program, which fires extension events, so it is `nil` here. The module names are sorted before they are printed, because the order a table is walked in differs between runs.
- **The same on both Lua versions.** `{ 1, 2.5 }` has no whole-number float in it, so Lua 5.4 and LuaJIT print the same JSON. The numbers that differ are the subject of [`integers-and-json`](../../lua54/integers-and-json/) and [`numbers-and-json`](../../luajit/numbers-and-json/).
- **One output channel.** Everything goes through `print`. Under LuaJIT, `print` and `airsstack.stdio.write` are separate buffers, and a run piped into a file can reorder their lines, so an example never mixes them.

## See also

- [Running Scripts in Stock Lua](https://airsstack.github.io/airsl/ch01-05-stock-lua.html) — what the module is and how it differs from `airsl`.
- [Module Reference](https://airsstack.github.io/airsl/appendix-02-modules.html) — every function the modules offer.
