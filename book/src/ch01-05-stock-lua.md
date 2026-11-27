# Running Scripts in Stock Lua

`airsl` is not the only place an `airsstack` script can run. The same modules are also a native
module for the Lua you already have: Lua 5.4 or LuaJIT, which includes the Lua inside Neovim. You
install it with LuaRocks, and then `require("airsstack")` works in that Lua.

Use it when you want `json`, `fs`, `regex` and the rest in a script, a test, or a plugin that runs
outside `airsl`. The scripts in this chapter, and a few more, are in the
[examples folder](https://github.com/airsstack/airsl/tree/main/crates/airsl-lua/examples) of the
repository, each with the output it prints.

## What the module is

In stock Lua, `require("airsstack")` returns a table with every `airsl` module except `ext`:
`json`, `path`, `fs`, `env`, `proc`, `regex`, `hash`, `time`, `glob`, `stdio` and `hook`. `ext` only
exists inside `airsl`, which is the program that fires extension events.

It sets no global, so keep the table in a local. Save this as `hello.lua`:

```lua
{{#include ../../crates/airsl-lua/examples/common/hello-require/main.lua}}
```

Once the module is installed, as the next sections show, Lua 5.4 and LuaJIT print the same lines:

```console
$ lua5.4 hello.lua
modules: env, fs, glob, hash, hook, json, path, proc, regex, stdio, time
ext: nil
global airsstack: nil
package.loaded holds the same table: true
a second require returns it again: true
{"name":"airsl","numbers":[1,2.5]}
src/main.lua
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
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

LuaRocks files each package under the Lua it is for: `airsstack` under 5.4, and `airsstack-luajit`
under 5.1, the version LuaJIT implements. That is why the commands below name the version.

## Installing

If you have `airsl`, it can run LuaRocks for you:

```console
$ airsl lua setup --native
created .airsl/types/airsstack.lua
created .luarc.json
Installing https://luarocks.org/airsstack-0.3.0-1.macosx-aarch64.rock

airsstack 0.3.0-1 depends on lua 5.4 (5.4-1 provided by VM: success)
No existing manifest. Attempting to rebuild...
airsstack 0.3.0-1 is now installed in ~/.luarocks (license: Apache-2.0)

require("airsstack") works once you run: eval "$(luarocks path)"
$ airsl lua setup --native --lua luajit
unchanged .airsl/types/airsstack.lua
unchanged .luarc.json
Installing https://luarocks.org/airsstack-luajit-0.3.0-1.macosx-aarch64.rock

airsstack-luajit 0.3.0-1 depends on lua 5.1 (5.1-1 provided by VM: success)
airsstack-luajit 0.3.0-1 depends on luajit >= 2.1 (2.1.1770848219-1 provided by VM: success)
No existing manifest. Attempting to rebuild...
airsstack-luajit 0.3.0-1 is now installed in ~/.luarocks (license: Apache-2.0)

require("airsstack") works once you run: eval "$(luarocks --lua-version 5.1 path)"
```

The first two lines are the editor files, then LuaRocks prints its own install output, and the last
line is `airsl`'s hint for `luarocks path`. Both commands ran in the same folder, so the second one
finds the editor files already written and reports them as `unchanged`. The transcripts in this
chapter show your home folder as `~`, where LuaRocks prints the full path. They are from macOS on
Apple Silicon, so the package file name ends in `macosx-aarch64`; yours names your platform. The
command asks LuaRocks which Lua it targets and installs the matching package into your own tree,
which needs no administrator rights. Name the Lua with `--lua 5.4` or `--lua luajit` to choose one.
It installs the package version that matches your `airsl`, so run it again after you upgrade
`airsl`.

The command also writes the editor files that [Editor Support](ch01-04-editor-support.md)
describes, into the current folder or into `DIR` if you give one, and it writes them even if the
install then fails. If you only want the module, use LuaRocks directly.

Without `airsl`, ask LuaRocks directly. `--lua-version` names the Lua the package is for, and
`airsstack-luajit` is only listed for 5.1, so the second line fails without it unless LuaRocks
already targets 5.1:

```console
$ luarocks --lua-version 5.4 install --local airsstack
Installing https://luarocks.org/airsstack-0.3.0-1.macosx-aarch64.rock

airsstack 0.3.0-1 depends on lua 5.4 (5.4-1 provided by VM: success)
No existing manifest. Attempting to rebuild...
airsstack 0.3.0-1 is now installed in ~/.luarocks (license: Apache-2.0)

$ luarocks --lua-version 5.1 install --local airsstack-luajit
Installing https://luarocks.org/airsstack-luajit-0.3.0-1.macosx-aarch64.rock

airsstack-luajit 0.3.0-1 depends on lua 5.1 (5.1-1 provided by VM: success)
airsstack-luajit 0.3.0-1 depends on luajit >= 2.1 (2.1.1770848219-1 provided by VM: success)
No existing manifest. Attempting to rebuild...
airsstack-luajit 0.3.0-1 is now installed in ~/.luarocks (license: Apache-2.0)

```

A `--local` install is outside the folders Lua searches. Tell your shell where it is. On macOS and
Linux:

```console
$ eval "$(luarocks --lua-version 5.4 path)"   # Lua 5.4
$ eval "$(luarocks --lua-version 5.1 path)"   # LuaJIT
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
installed for, as in the Unix lines above: `luarocks --lua-version 5.1 path`.

Where a binary package is published, LuaRocks installs it without compiling anything. That covers
macOS on Apple Silicon and Intel, Windows on x64, and Linux with glibc 2.17 or newer on x86_64 and
aarch64. See [No binary package](#no-binary-package) for everything else, and
[If `require` fails](#if-require-fails) if the module does not load.

## Building from the repository

You can also build a package from a clone. A build, from a clone or from LuaRocks' own recipe,
needs:

- git,
- Rust 1.94 or newer, with `cargo`. If you use rustup, you need do nothing: the repository names
  Rust 1.94 in its `rust-toolchain.toml`, and rustup fetches it for the build,
- a C compiler,
- the Lua headers for the Lua you target, and
- LuaRocks. It fetches the build tool of the package, `luarocks-build-rust-mlua`, by itself, as
  the output below shows. The exception is a LuaRocks that runs on Lua 5.1, such as the one on
  Ubuntu: see [Ubuntu with apt](#ubuntu-with-apt).

Run LuaRocks from the clone, and give it a tree to install into. This builds the Lua 5.4 package:

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
point your shell at the tree and run the `hello-require` example from the clone:

```console
$ eval "$(luarocks --lua-version 5.4 --tree lua54-tree path)"
$ lua5.4 crates/airsl-lua/examples/common/hello-require/main.lua
modules: env, fs, glob, hash, hook, json, path, proc, regex, stdio, time
ext: nil
global airsstack: nil
package.loaded holds the same table: true
a second require returns it again: true
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

Then point your shell at that tree and run the same example:

```console
$ eval "$(luarocks --lua-version 5.1 --tree luajit-tree path)"
$ luajit crates/airsl-lua/examples/common/hello-require/main.lua
modules: env, fs, glob, hash, hook, json, path, proc, regex, stdio, time
ext: nil
global airsstack: nil
package.loaded holds the same table: true
a second require returns it again: true
{"name":"airsl","numbers":[1,2.5]}
src/main.lua
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
```

The `Missing dependencies` list at the end is a LuaRocks quirk on the first build into a tree. You
can ignore it: the line before it says the package is installed, and it is.

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
  so it writes the first form. The examples
  [`integers-and-json`](https://github.com/airsstack/airsl/tree/main/crates/airsl-lua/examples/lua54/integers-and-json)
  and
  [`numbers-and-json`](https://github.com/airsstack/airsl/tree/main/crates/airsl-lua/examples/luajit/numbers-and-json)
  show this on its own for each Lua.
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

`airsl lua setup --native` takes no `--lua-dir`, and it takes `--lua 5.4` or `--lua luajit` for the
version. To have both work, store the folder in LuaRocks' configuration once, as the next section
shows for Homebrew. The `config` command with `lua_dir` writes your user configuration for that
version, so the flag is no longer needed for it.

### Homebrew on macOS

Homebrew's `lua` formula is Lua 5.5 or newer, and `lua@5.4` is keg-only: Homebrew installs it but
does not link it onto your `PATH`. So on a new install LuaRocks targets the newer Lua and does not
find Lua 5.4. A Mac whose `lua` is still a 5.4 that was never upgraded needs no `--lua-dir` for it,
as the transcripts under [Installing](#installing) show. Where LuaRocks does not find the Lua, give
it the folder:

```console
$ luarocks --lua-version 5.4 --lua-dir "$(brew --prefix lua@5.4)" install --local airsstack
Installing https://luarocks.org/airsstack-0.3.0-1.macosx-aarch64.rock

airsstack 0.3.0-1 depends on lua 5.4 (5.4-1 provided by VM: success)
No existing manifest. Attempting to rebuild...
airsstack 0.3.0-1 is now installed in ~/.luarocks (license: Apache-2.0)

$ luarocks --lua-version 5.1 --lua-dir "$(brew --prefix luajit)" install --local airsstack-luajit
Installing https://luarocks.org/airsstack-luajit-0.3.0-1.macosx-aarch64.rock

airsstack-luajit 0.3.0-1 depends on lua 5.1 (5.1-1 provided by VM: success)
airsstack-luajit 0.3.0-1 depends on luajit >= 2.1 (2.1.1770848219-1 provided by VM: success)
No existing manifest. Attempting to rebuild...
airsstack-luajit 0.3.0-1 is now installed in ~/.luarocks (license: Apache-2.0)

```

Give `luarocks path` the same two flags, and run your script with the interpreter in that folder,
because no `lua5.4` is on your `PATH`:

```console
$ eval "$(luarocks --lua-version 5.4 --lua-dir "$(brew --prefix lua@5.4)" path)"
$ "$(brew --prefix lua@5.4)/bin/lua5.4" hello.lua
modules: env, fs, glob, hash, hook, json, path, proc, regex, stdio, time
ext: nil
global airsstack: nil
package.loaded holds the same table: true
a second require returns it again: true
{"name":"airsl","numbers":[1,2.5]}
src/main.lua
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
```

To stop repeating `--lua-dir`, store the folder once for each Lua:

```console
$ luarocks --lua-version 5.4 config --scope user lua_dir "$(brew --prefix lua@5.4)"
Wrote
	variables.LUA = "/opt/homebrew/opt/lua@5.4/bin/lua5.4"
	variables.LUA_BINDIR = "/opt/homebrew/opt/lua@5.4/bin"
	variables.LUA_DIR = "/opt/homebrew/opt/lua@5.4"
	variables.LUA_INCDIR = "/opt/homebrew/opt/lua@5.4/include/lua5.4"
	variables.LUA_LIBDIR = "/opt/homebrew/opt/lua@5.4/lib"
to
	~/.luarocks/config-5.4.lua
$ luarocks --lua-version 5.1 config --scope user lua_dir "$(brew --prefix luajit)"
Wrote
	variables.LUA = "/opt/homebrew/opt/luajit/bin/luajit"
	variables.LUA_BINDIR = "/opt/homebrew/opt/luajit/bin"
	variables.LUA_DIR = "/opt/homebrew/opt/luajit"
	variables.LUA_INCDIR = "/opt/homebrew/opt/luajit/include/luajit-2.1"
	variables.LUA_LIBDIR = "/opt/homebrew/opt/luajit/lib"
to
	~/.luarocks/config-5.1.lua
```

LuaRocks lists what it stored. The folders above are the Apple Silicon ones: Homebrew's prefix is
`/opt/homebrew` there and `/usr/local` on an Intel Mac. Each command makes LuaRocks remember the
folder for that version, and also makes that version the
one LuaRocks targets when you name none, so the last one you run decides. After that,
`airsl lua setup --native --lua 5.4` or `--lua luajit` works without `--lua-dir`.

### Ubuntu with apt

The `luarocks` package targets Lua 5.1 from the `lua5.1` package, not LuaJIT. A Lua 5.1 that is not
LuaJIT cannot load `airsstack-luajit`, and the package needs LuaJIT 2.1, which LuaRocks does not
find in that Lua. `airsl lua setup --native` sees this before it writes anything and refuses:

```text
airsl: `luarocks` found no LuaJIT for Lua 5.1: its Lua 5.1 is not LuaJIT, or it has none, and `airsstack-luajit` needs LuaJIT
       install Lua 5.4 and run again with `--lua 5.4` to install it for Lua 5.4
       or run `luarocks --lua-version 5.1 config --scope user lua_dir <LuaJIT prefix>` to point `luarocks` at LuaJIT
       https://airsstack.github.io/airsl/ch01-05-stock-lua.html#pointing-luarocks-at-lua-54-or-luajit shows how to point `luarocks` at Lua 5.4 or LuaJIT
```

`luarocks install` on its own stops in the same place, later, with a LuaRocks error. LuaRocks 3.12
prints:

```text
Error: Could not satisfy dependency luajit >= 2.1: No results matching query were found for Lua 5.1.
```

So on Ubuntu, name the Lua yourself: `airsl lua setup --native --lua 5.4`.

For LuaJIT, point LuaRocks at a folder that holds only LuaJIT: a `bin/luajit` and the LuaJIT
headers in `include`. Pass it as `--lua-dir`, or store it once with
`luarocks --lua-version 5.1 config --scope user lua_dir <LuaJIT prefix>`, as
[Homebrew on macOS](#homebrew-on-macos) does. After that, `airsl lua setup --native --lua luajit`
works. For Lua 5.4, `--lua-version 5.4` is enough after `apt install lua5.4 liblua5.4-dev`.

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

Two more things matter on Windows.

- The packages load the Lua library by its name: `lua54.dll` for `airsstack` and `lua51.dll` for
  `airsstack-luajit`. They also need the Visual C++ 2015 or newer runtime. Use a Lua that ships
  its DLL under that name. A Lua linked statically into its `lua.exe` has no DLL for the module to
  find.
- LuaRocks finds the interpreter through your `PATH`. Put the `bin` folder of your Lua on it, which
  is the folder you gave to `--lua-dir` plus `\bin`, before you run `luarocks path`. Without it,
  LuaRocks reports for LuaJIT that it could not find Lua 5.1 in `PATH`.

## Neovim

Neovim embeds LuaJIT, so it needs `airsstack-luajit`. Neovim does not search LuaRocks' folders, so
the script asks LuaRocks for them and adds them to `package.cpath`, but only when `require` cannot
already find the module. This works on every platform, because LuaRocks writes the folders with
the right separators and file extension. Save this as `nvim-airsstack.lua`:

```lua
{{#include ../../crates/airsl-lua/examples/neovim/require-in-nvim/main.lua}}
```

`luarocks` must be on your `PATH`, as it is when you install the package. On Windows, your Lua's
`bin` folder must be on it too, as [Windows](#windows) says. Run the file with Neovim's `-l`
option, which needs Neovim 0.9 or newer:

```console
$ nvim -l nvim-airsstack.lua
{"editor":"neovim","jit":true,"lua":"Lua 5.1"}
sha256: ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
path: lua/plugin/init.lua
global airsstack: nil
```

The script writes with `io.write`, not `print`. Under `nvim -l`, `print` writes to standard error,
so a pipe or a redirect would not capture it, while `io.write` and `airsstack.stdio.write` write to
standard output. Those two are separate buffers, so pick one of them and keep to it if the order of
the lines matters.

Official Neovim builds embed LuaJIT, but some distributions build Neovim with plain Lua 5.1, which
cannot load the package. To check yours, run `:lua print(jit and jit.version)` in Neovim. It
prints a LuaJIT version when the package can load, and `nil` when it cannot.

## No binary package

Some platforms have no binary package. Those are musl Linux, such as Alpine, and any platform not
listed under [Installing](#installing), such as 32-bit systems.

The native module is not supported on musl Linux. No package is built for it. LuaRocks there does
not tell musl from glibc Linux, so `luarocks install` installs the glibc package, which cannot load
on musl, and the failure shows only when you `require` the module.

`airsl lua setup --native` stops you before that. An `airsl` from a musl
[archive](appendix-04-installation.md#archives) refuses `--native` before it writes any file or
starts LuaRocks:

```text
airsl: this `airsl` is built for musl Linux, and `airsstack` has no package for musl: the glibc package `luarocks` installs cannot load there
       on a glibc system, install the package with `luarocks` yourself:
       luarocks --lua-version 5.4 install --local airsstack 0.3.0
       luarocks --lua-version 5.1 install --local airsstack-luajit 0.3.0
       https://airsstack.github.io/airsl/ch01-05-stock-lua.html#no-binary-package says what works on musl and where no package exists
```

The refusal goes by how `airsl` was built, not by the system it runs on, so a musl `airsl` on a
glibc system is refused too. The [archives](appendix-04-installation.md#archives) say when the
installer picks that build. There the `luarocks` lines in the message work.

On any other platform without a binary package, LuaRocks builds the package from its recipe. The
requirements are the ones listed under [Building from the repository](#building-from-the-repository).

## If `require` fails

- **`module 'airsstack' not found`.** Lua is not looking where LuaRocks installed the package.
  Lua prints the list of places it looked after that line. Run the `luarocks path` line for your
  shell from [Installing](#installing), in the same terminal as the script.
- **An error from the dynamic linker.** The package was built for another Lua. Loading `airsstack`,
  the Lua 5.4 package, into Neovim on macOS ends with `symbol not found in flat namespace
  '_luaL_tolstring'`. Other systems word it differently. Install the package for your Lua, as
  [The two packages](#the-two-packages) shows, and run it with that Lua.
- **A failure on Windows.** Check the Lua DLL name and the Visual C++ runtime under
  [Windows](#windows).
- **Alpine or another musl Linux.** There is no package for it, and `airsl lua setup --native`
  refuses to install one, as [No binary package](#no-binary-package) says.

## Summary

- `require("airsstack")` in stock Lua 5.4 or LuaJIT returns the `airsl` modules except `ext`, with
  no global, full authority and no limits.
- `airsstack` is for Lua 5.4, `airsstack-luajit` is for LuaJIT and Neovim.
- `airsl lua setup --native` or `luarocks install --local` installs it, and `luarocks path` makes
  your shell find it.
- LuaJIT writes whole-number floats as integers, has no `utf8`, and can reorder `stdio.write`
  output against `print`.
- Where LuaRocks targets the wrong Lua, pass `--lua-version` and `--lua-dir`, or store the folder
  with `luarocks config`.
