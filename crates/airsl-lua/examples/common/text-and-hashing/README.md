# text-and-hashing

Splits and matches a log with `regex`, and prints digests with `hash`. It runs unchanged under Lua 5.4 and under LuaJIT and prints the same thing under both.

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
LUA_CPATH="$PWD/target/native/lua54/?.so;;" lua5.4 crates/airsl-lua/examples/common/text-and-hashing/main.lua
```

For LuaJIT, use `target/native/luajit` and `luajit` in place of those two.

## Output

```
lines: 3
INFO  engine 2023-11-14T22:13:20Z
WARN  fs     2023-11-14T22:13:21Z
ERROR proc   2023-11-14T22:13:22Z
timestamps found: 3
2023-11-14T22:13:21Z WARN  fs denied: <path> is outside the granted read roots
sha256: ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
sha1:   a9993e364706816aba3e25717850c26c9cd0d89d
hex:    616972736c
```

Both interpreters print exactly this; the gate runs the example under each and requires it.

## What it demonstrates

- **`regex` says what a Lua pattern cannot.** The pattern uses alternation (`INFO|WARN|ERROR`) and named groups. It is compiled once and then reused for each line.
- **Captures arrive by number and by name**, so `caught.level` and `caught[2]` are the same text.
- **Digests are lowercase hex**, which is what `shasum` and `sha256sum` print, so they compare directly with the tools. `sha1` ships for compatibility with names that already exist, not for strength.
- **The same on both Lua versions.** Nothing here involves a number that the two Lua versions spell differently.

## See also

- [Running Scripts in Stock Lua](https://airsstack.github.io/airsl/ch01-05-stock-lua.html) — what the module is.
- [Module Reference](https://airsstack.github.io/airsl/appendix-02-modules.html#regex) — `regex` and `hash`.
