# integers-and-json

Shows that Lua 5.4 keeps an integer and a float apart, and that `json` keeps them apart too. It
runs under Lua 5.4 only.

LuaJIT has one number type, so it cannot say any of this; its side is
[`numbers-and-json`](../../luajit/numbers-and-json/).

## Install

```bash
luarocks --lua-version 5.4 install --local airsstack
eval "$(luarocks --lua-version 5.4 path)"
```

## Run

```bash
lua5.4 main.lua
```

From a checkout, build the module once and put it on the interpreter's search path:

```bash
cargo make native-load
LUA_CPATH="$PWD/target/native/lua54/?.so;;" lua5.4 crates/airsl-lua/examples/lua54/integers-and-json/main.lua
```

## Output

```
Lua version: Lua 5.4
{"big":9007199254740993,"count":1,"half":0.5,"ratio":1.0}
whole:    integer 1
fraction: float 1.0
integer survives: true
integer division: [3,3.5]
```

## What it demonstrates

- **`1` and `1.0` stay what they were.** The encoder writes an integer without a point and a float with one, so a table that holds both round-trips to the text it came from.
- **Decoding goes the other way.** A JSON `1` comes back as an integer and `1.0` as a float, which `math.type` tells apart.
- **Large integers survive.** `9007199254740993` is the first integer above 2^53 that a double cannot hold. As a Lua 5.4 integer it passes through `json` unchanged.
- **`airsl` behaves this way too**, because it runs Lua 5.4. A script that depends on it belongs to Lua 5.4, not to the examples that run under both.

## See also

- [How it differs from `airsl`](https://airsstack.github.io/airsl/ch01-05-stock-lua.html#how-it-differs-from-airsl) — the differences between stock Lua 5.4, LuaJIT and `airsl`.
- [Module Reference](https://airsstack.github.io/airsl/appendix-02-modules.html#json) — `json`.
