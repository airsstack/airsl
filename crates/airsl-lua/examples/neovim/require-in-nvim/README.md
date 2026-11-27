# require-in-nvim

Loads the native module inside Neovim and uses `json`, `hash` and `path`. It needs Neovim 0.9 or
newer, which added `nvim -l`.

Neovim embeds LuaJIT, so the package is `airsstack-luajit`. This example is not run by
`cargo make dod`, which needs no editor; `cargo make nvim-examples` runs it.

## Install

```bash
luarocks --lua-version 5.1 install --local airsstack-luajit
```

Neovim does not search the folders LuaRocks installs into. The script asks LuaRocks for them with
`luarocks --lua-version 5.1 path --lr-cpath` and adds the answer to `package.cpath`, as
[Neovim](https://airsstack.github.io/airsl/ch01-05-stock-lua.html#neovim) describes, so `luarocks` must be on your `PATH`.

## Run

```bash
nvim --headless -u NONE -l main.lua
```

`-l` runs a Lua file and exits. `--headless -u NONE` keeps the run from loading a UI or your
configuration.

From a checkout, there is nothing installed for LuaRocks to find, so put the built module on the
search path yourself. The script asks LuaRocks only when `require` cannot already find the module:

```bash
cargo make nvim-examples
```

or, by hand:

```bash
cargo make native-load
LUA_CPATH="$PWD/target/native/luajit/?.so;;" nvim --headless -u NONE -l crates/airsl-lua/examples/neovim/require-in-nvim/main.lua
```

`AIRSL_NVIM` names the Neovim that `cargo make nvim-examples` runs, `nvim` by default.

## Output

```
{"editor":"neovim","jit":true,"lua":"Lua 5.1"}
sha256: ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
path: lua/plugin/init.lua
global airsstack: nil
```

## What it demonstrates

- **Neovim is LuaJIT.** `_VERSION` is `Lua 5.1` and `jit` is a table, so the module that loads is the LuaJIT one. A Lua 5.4 module in Neovim fails to load with a dynamic-linker error, because the Lua functions it needs are not there.
- **No global.** `require` is the only way in, as everywhere else.
- **One output channel.** Under `nvim -l`, `print` writes to standard error, while `io.write` and `airsstack.stdio.write` write to standard output. Those two are separate buffers, so the script uses `io.write` only and its lines stay in order.
- **JSON numbers follow LuaJIT**, because it is LuaJIT's module that is loaded: see [`numbers-and-json`](../../luajit/numbers-and-json/).

## See also

- [Neovim](https://airsstack.github.io/airsl/ch01-05-stock-lua.html#neovim) — the same steps in the book, with the Windows notes.
- [The two packages](https://airsstack.github.io/airsl/ch01-05-stock-lua.html#the-two-packages) — which package goes with which Lua.
