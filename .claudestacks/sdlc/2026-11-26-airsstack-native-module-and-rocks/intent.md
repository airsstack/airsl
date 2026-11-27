---
status: done
created: 2026-11-26
milestone: lua-ecosystem/03
---

# Intent: The `airsstack` modules as a native Lua module on LuaRocks

## Problem

The `airsstack` modules exist only inside airsl. Milestone 01 gave editors a type file and 02 made
`local airsstack = require("airsstack")` work in airsl, but in stock Lua 5.4, LuaJIT or Neovim the
same line fails: there is no module to load. A script cannot run, be tested or be debugged outside
airsl, and the roadmap rules out a debugger inside airsl, so stock Lua is the only place to debug.

Three gaps remain, from milestones 03, 04 and 05 (04 and 05 merged into 03 on 2026-11-26):

| Gap | Today |
|---|---|
| No native module | nothing builds a `luaopen_airsstack` library; airsl links mlua with `vendored` and `send`, which mlua's `module` mode rejects |
| No distribution | no rockspec, no rock on luarocks.org, no prebuilt binary for any platform |
| No setup | `airsl lua setup` writes the type file and `.luarc` only; nothing installs the module at airsl's version |

## Affected systems

- `crates/airsl`: mlua features are fixed in the root `Cargo.toml` (`lua54,vendored,serialize,send`);
  the path that installs modules into a `Lua` is private (`InstallContext::new` is `pub(crate)`,
  `install_modules` in `builder.rs` is private, `ModuleSet` cannot leave `ext` out);
  `LanguageSurface` uses `StdLib::UTF8` and `StdLib::COROUTINE`, which LuaJIT lacks.
- A new workspace crate, `crates/airsl-lua`, named by the roadmap.
- `crates/airsl-cli`: `airsl lua setup` (`LuaCommand::Setup`, `lua.rs`).
- Gate and release: `Makefile.toml` (`--workspace --all-features` in every step), `deny.toml`
  (`all-features = true`), `.github/workflows/` (ci, release), `dist-workspace.toml`.
- Docs: the book (editor support, CLI appendix, the three-layers chapter's "rather than LuaJIT",
  installation), the airsl-cli README, the values-and-json example, `CLAUDE.md`.

## Desired outcome

- `require("airsstack")` in stock Lua 5.4 and in LuaJIT returns a table holding every airsl module
  except `ext`, running with trusted authority. No global is set.
- `luarocks install airsstack` (Lua 5.4) and `luarocks install airsstack-luajit` (LuaJIT) work from
  luarocks.org: from source with cargo, or from a prebuilt binary rock where one exists for the
  platform.
- Every airsl release, starting with the roadmap's release, uploads the rockspec and the binary
  rocks of both packages, at that release's version, for every supported platform. A platform
  without a binary rock installs from the rockspec, which needs git and cargo.
- `airsl lua setup --native` installs the package matching the Lua that luarocks targets, at the
  running airsl's version, and `--lua 5.4|luajit` overrides the detection. When luarocks is
  missing it prints the exact command and exits non-zero.
- `cargo make dod` stays the one gate and also builds and tests the native module against a real
  Lua 5.4 and a real LuaJIT; CI runs the same.
- The book documents the native module, the two packages, the LuaJIT JSON difference (`2` against
  `2.0`), the missing `utf8` library on LuaJIT, that stock Lua has no memory ceiling, and that
  `ext` exists only in airsl.

## Constraints

Decisions the author accepted on 2026-11-26; the design keeps them:

1. Names: crate `airsl-lua`; LuaRocks packages `airsstack` (Lua 5.4, depends on `lua == 5.4`) and
   `airsstack-luajit` (LuaJIT, depends on `lua == 5.1`); the module is `airsstack` in both.
2. The module returns the root table and sets no global.
3. Distribution is luarocks.org only: a rockspec and binary rocks for both packages, no source
   rock, and no rocks server of our own. Several binary rocks per version go through the
   luarocks.org upload API.
4. LuaJIT is supported, for Neovim; its differences are documented, not hidden.
5. Platforms are a subset of airsl's release targets: Linux glibc with a 2.17 floor and no musl
   rocks; macOS one rock per arch (`macosx-aarch64`, `macosx-x86_64`); `win32-x86_64` only if a
   probe proves an MSVC-built module loads.
6. `--native` detects the Lua version from luarocks, with a `--lua 5.4|luajit` override; the
   version installed is `airsl::VERSION`; a missing luarocks prints the command and exits
   non-zero; only this flag touches the network.
7. The type file is unchanged: `ext` stays typed and is documented as airsl-only.
8. Nothing is published before the roadmap's single release; until then the rocks are tested with
   a local `luarocks make`. From that release on, every release publishes the rocks.

Technical limits found during exploration, each to be proven by a probe in design:

- mlua's `module` feature cannot be combined with `vendored` or `send`, and a build carries one Lua
  version: the native module is one library per Lua ABI, and the workspace-wide `--all-features`
  runs cannot include it.
- airsl-cli needs `send` (its clap parser requires `airsl::Error: Send + Sync`), and airsl's
  default features must not change for existing dependents.
- Module mode links no executable, so the native module is tested by loading it into a real
  interpreter.
- Outside airsl there is no memory ceiling: `set_memory_limit` is unavailable in module mode.
- A luarocks.org binary rock's file name tells neither Lua 5.4 from LuaJIT nor glibc from musl,
  which is why there are two packages, each pinned by its `lua` dependency.

Every dependency claim (mlua 0.12 module mode, luarocks-build-rust-mlua, LuaRocks manifest and
binary selection, MSVC loading) is proven in design by a probe with saved output; exploration notes
are not evidence. The workspace lints, the module doc shape, inline tests and `cargo make dod`
apply to the new crate.

## Non-goals

- Sandboxing, grants or resource limits outside airsl.
- `ext`, or any stand-in for it, outside airsl.
- Publishing airsl itself on LuaRocks.
- A rocks server of our own.
- musl rocks, and platforms airsl does not release for.
- Installing the native module without LuaRocks.
- Changing the type file.
- A debugger in airsl.
- Supporting PUC Lua 5.1, 5.2 or 5.3; what `airsstack-luajit` does on PUC Lua 5.1 is for the design
  to settle.
