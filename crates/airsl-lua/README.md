# airsl-lua

The `airsstack` modules as a native module for the Lua you already have. `require("airsstack")`
returns `json`, `path`, `fs`, `env`, `proc`, `regex`, `hash`, `time`, `glob`, `stdio` and `hook`,
and sets no global. It runs with the authority of the Lua that loaded it, with no grants and no
limits.

It is not on crates.io. It ships as two LuaRocks packages, one for each Lua: `airsstack` for
Lua 5.4, and `airsstack-luajit` for LuaJIT and Neovim.

## Install

```sh
luarocks --lua-version 5.4 install --local airsstack
luarocks --lua-version 5.1 install --local airsstack-luajit
```

[Running Scripts in Stock Lua](https://airsstack.github.io/airsl/ch01-05-stock-lua.html) has what
comes next: how to make your shell find the package, what to do where LuaRocks targets another
Lua, the Neovim steps, and what to check when `require` fails. `airsl lua setup --native` does the
install for you.

## Examples

[`examples/`](examples) holds runnable scripts, each with a `README.md` that shows the output it
prints.
