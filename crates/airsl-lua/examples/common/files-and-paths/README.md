# files-and-paths

Writes a small tree into a scratch directory, lists and globs it, and removes it. It runs unchanged under Lua 5.4 and under LuaJIT and prints the same thing under both.

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
LUA_CPATH="$PWD/target/native/lua54/?.so;;" lua5.4 crates/airsl-lua/examples/common/files-and-paths/main.lua
```

For LuaJIT, use `target/native/luajit` and `luajit` in place of those two.

## Output

```
README.md holds: # scratch
src/main.lua is a file: true
src/util is a directory: true
top level: README.md, src
walk: README.md, src, src/main.lua, src/util, src/util/notes.txt, src/util/strings.lua
lua files: src/main.lua, src/util/strings.lua
lua files at the top: 0
does src/util/strings.lua match **/*.lua: true
src/util/strings.lua -> src/util + strings.lua
stem and extension: strings lua
scratch directory still exists: false
```

Both interpreters print exactly this; the gate runs the example under each and requires it.

## What it demonstrates

- **A script in stock Lua has the process's authority.** There is no grant to give and no root to name: `fs` reads and writes where the process can. The example still works in a fresh temporary directory and removes it, so it leaves nothing behind.
- **Nothing machine-specific is printed.** `fs.tempdir` returns a path that differs on every run, so the script prints only names relative to it.
- **Listings are sorted.** `fs.list`, `fs.walk` and `glob.walk` return their names in byte order, so two machines print the same lines.
- **`glob` follows one rule everywhere.** `*` stops at a `/`, and `**/` matches any number of folders, including none, which is why `*.lua` finds nothing at the top while `**/*.lua` finds both files.
- **`path` is text.** `dirname`, `basename`, `stem` and `ext` split a string and never ask the disk.

## See also

- [Running Scripts in Stock Lua](https://airsstack.github.io/airsl/ch01-05-stock-lua.html) — what the module is.
- [Module Reference](https://airsstack.github.io/airsl/appendix-02-modules.html#fs) — `fs`, `path` and `glob`.
