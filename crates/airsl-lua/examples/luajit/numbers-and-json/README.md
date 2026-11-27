# numbers-and-json

Shows what `json` gets from a Lua with no integer type: `1.0` and `1` are the same number, and
the text says `1`. It runs under LuaJIT only.

Lua 5.4's side is [`integers-and-json`](../../lua54/integers-and-json/).

## Install

```bash
luarocks --lua-version 5.1 install --local airsstack-luajit
eval "$(luarocks --lua-version 5.1 path)"
```

## Run

```bash
luajit main.lua
```

From a checkout, build the module once and put it on the interpreter's search path:

```bash
cargo make native-load
LUA_CPATH="$PWD/target/native/luajit/?.so;;" luajit crates/airsl-lua/examples/luajit/numbers-and-json/main.lua
```

## Output

```
Lua version: Lua 5.1
jit: true
math.type: nil
{"count":1,"half":0.5,"ratio":1}
whole:    1
fraction: 1
same value: true
2^53 + 1 is the same number as 2^53: true
["9007199254740993"]
```

## What it demonstrates

- **One number type.** There is no `math.type` and no integer subtype, so `1.0` is the same value as `1`. The encoder has nothing to tell them apart with and writes `1`, where Lua 5.4 writes `1.0`.
- **`tostring` agrees.** A decoded `1.0` prints as `1`, and compares equal to `1`.
- **Large integers round.** A double holds every integer up to 2^53 exactly and rounds above it, so `2^53 + 1` is the same number as `2^53`. An identifier that large belongs in a string, which the last line shows.
- **Neovim is this Lua.** What is true here is true in the editor; see [`require-in-nvim`](../../neovim/require-in-nvim/).

## See also

- [How it differs from `airsl`](https://airsstack.github.io/airsl/ch01-05-stock-lua.html#how-it-differs-from-airsl) — the differences between stock Lua 5.4, LuaJIT and `airsl`.
- [Module Reference](https://airsstack.github.io/airsl/appendix-02-modules.html#json) — `json`.
