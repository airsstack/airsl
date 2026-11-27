# Running Scripts in Stock Lua

`airsl` is not the only place an `airsstack` script can run. The same modules are also a native
module for the Lua you already have: Lua 5.4 or LuaJIT, which includes the Lua inside Neovim. You
install it with LuaRocks, and then `require("airsstack")` works in that Lua.

Use it when you want `json`, `fs`, `regex` and the rest in a script, a test, or a plugin that runs
outside `airsl`.

## What the module is

In stock Lua, `require("airsstack")` returns a table with every `airsl` module except `ext`:
`json`, `path`, `fs`, `env`, `proc`, `regex`, `hash`, `time`, `glob`, `stdio` and `hook`. `ext` only
exists inside `airsl`, which is the program that fires extension events.

It sets no global, so keep the table in a local. Save this as `hello.lua`:

```lua
local airsstack = require("airsstack")

print(airsstack.json.encode({ name = "airsl", numbers = { 1, 2.5 } }))
print(airsstack.path.join("src", "main.lua"))
print(airsstack.hash.sha256("abc"))
```

It runs with the same authority as the Lua that loaded it. There are no grants to give, and no
memory or instruction ceiling. Stock Lua has no sandbox for the module to join. If you need limits,
run the script with `airsl`.

## The two packages

There is one package for each Lua, and a Lua loads only its own:

| Lua | Package |
|---|---|
| Lua 5.4 | `airsstack` |
| LuaJIT, and Neovim | `airsstack-luajit` |

## Installing

If you have `airsl`, it can run LuaRocks for you:

```console
$ airsl lua setup --native
```

It asks LuaRocks which Lua it targets and installs the matching package into your own tree, which
needs no administrator rights. Name the Lua with `--lua 5.4` or `--lua luajit` to choose one. The
command also writes the editor files that [Editor Support](ch01-04-editor-support.md) describes.

Without `airsl`, ask LuaRocks directly:

```console
$ luarocks install --local airsstack
$ luarocks install --local airsstack-luajit
```

A `--local` install is outside the folders Lua searches. Tell your shell where it is. On macOS and
Linux:

```console
$ eval "$(luarocks path)"
```

On Windows, in `cmd`:

```console
> luarocks path > lrpath.bat
> call lrpath.bat
```

In PowerShell:

```console
> $env:LUA_PATH = luarocks path --lr-path
> $env:LUA_CPATH = luarocks path --lr-cpath
```

These set the paths for the current terminal only. For LuaJIT, give luarocks the version it
installed for: `luarocks --lua-version 5.1 path`.

Where a binary package is published, LuaRocks installs it without compiling anything. That covers
macOS on Apple Silicon and Intel, Windows on x64, and Linux with glibc on x86_64 and aarch64. See
[No binary package](#no-binary-package) for everything else.

## Building from the repository

You can also build a package from a clone. This needs a Rust toolchain, a C compiler, and
LuaRocks. Run LuaRocks from the clone, and give it a tree to install into. This builds the Lua 5.4
package:

```console
$ git clone https://github.com/airsstack/airsl
$ cd airsl
$ luarocks --lua-version 5.4 make --tree lua54-tree crates/airsl-lua/rockspecs/airsstack-[0-9]*-1.rockspec
Missing dependencies for airsstack 0.3.0-1:
   luarocks-build-rust-mlua 0.2.7 (not installed)

airsstack 0.3.0-1 depends on luarocks-build-rust-mlua 0.2.7 (not installed)
Installing https://luarocks.org/luarocks-build-rust-mlua-0.2.7-1.src.rock


No existing manifest. Attempting to rebuild...
   Compiling airsl-lua v0.3.0 (~/airsl/crates/airsl-lua)
   Compiling airsl v0.3.0 (~/airsl/crates/airsl)
    Finished `release` profile [optimized] target(s) in 16.30s
luarocks-build-rust-mlua 0.2.7-1 is now installed in ~/airsl/lua54-tree (license: MIT)


airsstack 0.3.0-1 depends on lua 5.4 (5.4-1 provided by VM: success)
airsstack 0.3.0-1 is now installed in ~/airsl/lua54-tree (license: Apache-2.0)
```

The first lines are LuaRocks fetching the build tool for the package. This run had Rust's build
cache warm, so it shows two `Compiling` lines, one for `airsl-lua` and one for `airsl`. The first
build in a fresh clone prints many more, one for each crate it compiles, and takes longer. Now
point your shell at the tree, save `hello.lua` from above in the clone, and run it:

```console
$ eval "$(luarocks --lua-version 5.4 --tree lua54-tree path)"
$ lua5.4 hello.lua
{"name":"airsl","numbers":[1,2.5]}
src/main.lua
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
```

For LuaJIT, use the LuaJIT rockspec and tell LuaRocks to build for Lua 5.1, the version LuaJIT
implements:

```console
$ luarocks --lua-version 5.1 make --tree luajit-tree crates/airsl-lua/rockspecs/airsstack-luajit-[0-9]*-1.rockspec
Missing dependencies for airsstack-luajit 0.3.0-1:
   luarocks-build-rust-mlua 0.2.7 (not installed)

airsstack-luajit 0.3.0-1 depends on luarocks-build-rust-mlua 0.2.7 (not installed)
Installing https://luarocks.org/luarocks-build-rust-mlua-0.2.7-1.src.rock


No existing manifest. Attempting to rebuild...
   Compiling airsl-lua v0.3.0 (~/airsl/crates/airsl-lua)
   Compiling airsl v0.3.0 (~/airsl/crates/airsl)
    Finished `release` profile [optimized] target(s) in 17.45s
luarocks-build-rust-mlua 0.2.7-1 is now installed in ~/airsl/luajit-tree (license: MIT)


airsstack-luajit 0.3.0-1 depends on lua 5.1 (5.1-1 provided by VM: success)
airsstack-luajit 0.3.0-1 depends on luajit >= 2.1 (2.1.1770848219-1 provided by VM: success)
No existing manifest. Attempting to rebuild...
airsstack-luajit 0.3.0-1 is now installed in ~/airsl/luajit-tree (license: Apache-2.0)

Missing dependencies for airsstack-luajit 0.3.0-1:
   lua 5.1 (not installed)
   luajit >= 2.1 (not installed)
```

Then point your shell at that tree and run the same script:

```console
$ eval "$(luarocks --lua-version 5.1 --tree luajit-tree path)"
$ luajit hello.lua
{"name":"airsl","numbers":[1,2.5]}
src/main.lua
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
```

The `Missing dependencies` list at the end is a LuaRocks quirk on the first build into a tree. You
can ignore it: the line before it says the package is installed, and it is.

`cargo make rocks` does the same for both packages in one step. It builds each into `target/rocks`,
packs a `.rock` file for each into `target/rocks/dist`, and loads each build in its own Lua to check
that it works. It needs LuaRocks and a network connection, as the commands above do.

## How it differs from `airsl`

A script that runs in `airsl` can behave a little differently in stock Lua. Save this as
`differences.lua`:

```lua
local airsstack = require("airsstack")

print(airsstack.json.encode({ 2.0, 2 ^ 53 }))
print(utf8 ~= nil)

airsstack.env.set("GREETING", "hello")
print(airsstack.env.get("GREETING"))
print(os.getenv("GREETING"))
print(airsstack.proc.run({ arg[-1], "-e", "io.write(os.getenv('GREETING'))" }).stdout)
```

In Lua 5.4:

```console
$ lua5.4 differences.lua
[2.0,9007199254740992.0]
true
hello
nil
hello
```

In LuaJIT:

```console
$ luajit differences.lua
[2,9007199254740992]
false
hello
nil
hello
```

- **JSON of a whole-number float.** Lua 5.4 tells `2` from `2.0`, so `2.0` stays `2.0`. LuaJIT
  has one kind of number, so it writes `2`, and `2^53` as `9007199254740992`. `airsl` runs Lua 5.4,
  so it writes the first form.
- **No `utf8` in LuaJIT.** The `utf8` library is part of Lua 5.3 and later.
- **No limits.** There is no memory or instruction ceiling, as described above.
- **`env.set` and `os.getenv`.** `env.set` changes what `env.get` and the programs `proc.run`
  starts see. Lua's own `os.getenv` does not see it, as the `nil` shows. The script starts the Lua
  that runs it, `arg[-1]`, so it needs no shell.
- **Output order in LuaJIT.** When standard output goes to a file or a pipe, text from `stdio.write`
  and `hook` can come out before `print` lines that ran earlier. Lua 5.4 keeps the order. Save this
  as `order.lua`:

  ```lua
  local airsstack = require("airsstack")

  print("one")
  airsstack.stdio.write("two\n")
  print("three")
  ```

  ```console
  $ lua5.4 order.lua > out.txt; cat out.txt
  one
  two
  three
  $ luajit order.lua > out.txt; cat out.txt
  two
  one
  three
  ```

  Write everything through one of the two, `print` or `stdio.write`, if the order matters.

## Pointing LuaRocks at Lua 5.4 or LuaJIT

LuaRocks builds for one Lua at a time, and some systems set it up for another. If `luarocks
install` or `airsl lua setup --native` reports that LuaRocks installs for a Lua the module does not
support, or cannot find the Lua you named, tell LuaRocks where your Lua lives. Two flags do it:
`--lua-version` names the version (`5.4`, or `5.1` for LuaJIT) and `--lua-dir` names the folder that
holds the Lua's `bin` and `include` folders.

### Homebrew on macOS

`lua@5.4` is installed but not linked, so LuaRocks does not find it. LuaRocks itself targets the
newest Lua Homebrew has, which can be Lua 5.5. Give it the folder:

```console
$ luarocks --lua-version 5.4 --lua-dir "$(brew --prefix lua@5.4)" install --local airsstack
$ luarocks --lua-version 5.1 --lua-dir "$(brew --prefix luajit)" install --local airsstack-luajit
```

### Ubuntu with apt

The `luarocks` package targets Lua 5.1 from the `lua5.1` package, not LuaJIT. For LuaJIT, point
`--lua-dir` at a folder that holds only LuaJIT: a `bin/luajit` and the LuaJIT headers in `include`.
For Lua 5.4, `--lua-version 5.4` is enough after `apt install lua5.4 liblua5.4-dev`.

If you build from source, LuaRocks needs the build tool in its own Lua 5.1 tree first:

```console
$ luarocks --lua-version 5.1 install --local luarocks-build-rust-mlua 0.2.7
```

### Windows

Download the LuaRocks zip from the LuaRocks site and use its `luarocks.exe`. A `luarocks.bat`
wrapper is not enough, because `airsl lua setup --native` starts `luarocks.exe` itself. Give it the
folder of your Lua build:

```console
> luarocks --lua-version 5.4 --lua-dir C:\lua54 install --local airsstack
> luarocks --lua-version 5.1 --lua-dir C:\luajit install --local airsstack-luajit
```

## Neovim

Neovim embeds LuaJIT, so it needs `airsstack-luajit`. Neovim does not search LuaRocks' folders, so
ask LuaRocks for them and add them to `package.cpath`. This works on every platform, because
LuaRocks writes the folders with the right separators and file extension. Save this as
`nvim-airsstack.lua`:

```lua
local cpath = vim.fn.system({ "luarocks", "--lua-version", "5.1", "path", "--lr-cpath" })
assert(vim.v.shell_error == 0, cpath)
package.cpath = package.cpath .. ";" .. vim.trim(cpath)

local airsstack = require("airsstack")
print(airsstack.json.encode({ editor = "neovim" }))
```

`luarocks` must be on your `PATH`, as it is when you install the package. On Windows, the `bin`
folder of your LuaJIT must be on your `PATH` too: that is the folder you gave to `--lua-dir` when
you installed the package, plus `\bin`. Without it, LuaRocks reports that it could not find
Lua 5.1 in `PATH`. Run the file with Neovim's `-l` option:

```console
$ nvim -l nvim-airsstack.lua
{"editor":"neovim"}
```

## No binary package

Some platforms have no binary package. Those are musl Linux, such as Alpine, and any platform not
listed under [Installing](#installing), such as 32-bit systems.

The native module is not supported on musl Linux. No package is built for it, and LuaRocks there
does not tell musl from glibc Linux, so it can fetch the glibc package.

On any other platform without a binary package, LuaRocks builds the package from its recipe. That
needs git, `cargo` with Rust 1.94 or newer, a C compiler, the Lua headers for the Lua you target,
and the LuaRocks build tool.

## Summary

- `require("airsstack")` in stock Lua 5.4 or LuaJIT returns the `airsl` modules except `ext`, with
  no global, full authority and no limits.
- `airsstack` is for Lua 5.4, `airsstack-luajit` is for LuaJIT and Neovim.
- `airsl lua setup --native` or `luarocks install --local` installs it, and `luarocks path` makes
  your shell find it.
- LuaJIT writes whole-number floats as integers, has no `utf8`, and can reorder `stdio.write`
  output against `print`.
- Where LuaRocks targets the wrong Lua, pass `--lua-version` and `--lua-dir`.
