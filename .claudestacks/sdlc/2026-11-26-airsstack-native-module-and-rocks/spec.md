---
status: approved
created: 2026-11-26
built-on: 26e234fff7b3
---

# Spec: The `airsstack` modules as a native Lua module on LuaRocks

This spec makes `local airsstack = require("airsstack")` work in stock Lua 5.4 and in LuaJIT, so a
script written for airsl can be run, tested and debugged outside airsl. It adds a fourth workspace
crate, `crates/airsl-lua`, that builds airsl's host modules (all but `ext`) as a loadable library;
two LuaRocks packages, `airsstack` (Lua 5.4) and `airsstack-luajit` (LuaJIT), published to
luarocks.org with prebuilt binary rocks for five platforms from the roadmap's release onwards;
`airsl lua setup --native [--lua 5.4|luajit]`, which installs the package matching the Lua that
luarocks targets at airsl's own version; a gate that loads the module into a real Lua 5.4 and a
real LuaJIT; and the book pages that explain the differences. airsl itself does not change for its
users: the engine stays Lua 5.4, vendored and `Send + Sync`.

**How proof is written.** `probes/NN-name.txt` is a file in this chain's `probes/` directory; its
header names the question, tool versions and date, and the quoted line is from its raw output.
`file:line` is a line of the repository at HEAD `9d054c4`, opened while writing this spec. Every
config example below was parsed by its real tool in the cited probe and is pasted byte for byte.
§13 lists every probe this spec relies on.

## 1. Premises and settled decisions

### 1.1 Facts the design stands on

| # | Fact | Proof |
|---|---|---|
| F1 | mlua's `module` feature cannot be combined with `vendored` or with `send`, and a build carries exactly one Lua | `probes/01-mlua-module-feature-conflicts.txt`: "`vendored`, `external` and `module` features are mutually exclusive", "`send` feature is not supported in module mode", "You can enable only one of the features: lua55, lua54, …" |
| F2 | airsl's library already compiles in module mode; only test/example `Send + Sync` assertions and airsl-cli need `send` | `probes/02-airsl-lua54-module-mode.txt` (lib `exit=0`; all-targets: "`Rc<…RawLua>>` cannot be sent between threads safely"; airsl-cli: "`(dyn std::error::Error + 'static)` cannot be shared between threads safely") |
| F3 | Under LuaJIT the only compile errors are `StdLib::UTF8`/`COROUTINE` in `LanguageSurface::libraries` | `probes/03-airsl-luajit-compile.txt`: "no associated item named `UTF8` found for struct `LuaStdLib`"; `probes/04-airsl-luajit-hidden-errors.txt` (lib clean after those) |
| F4 | A Lua-touching unit test cannot link or run in a module-mode crate, and rustdoc cannot test a cdylib-only crate | `probes/10-module-unit-test-link.txt`: "`symbol not found in flat namespace '_luaopen_base'`" … "SIGABRT"; `probes/05-feature-split-workspace.txt` (j): "doc tests are not supported for crate type(s) `cdylib`" |
| F5 | One library per Lua ABI: a 5.4 build fails to load in LuaJIT and the reverse | `probes/06-macos-native-load.txt`: "symbol not found in flat namespace '_luaL_tolstring'" / "'_lua_insert'"; `probes/164-y2-ubuntu-apt.txt`: "undefined symbol: lua_rawgetp" / "undefined symbol: lua_pcall" |
| F6 | Module mode has no memory ceiling | `probes/07-module-memory-limit.txt`: "Err(Display=memory control is not available; Debug=MemoryControlNotAvailable)" under both ABIs |
| F7 | JSON of an integral float differs: Lua 5.4 `2.0`, LuaJIT `2`; `2^53` is `9007199254740992.0` on 5.4 and `9007199254740992` on LuaJIT | `probes/08-module-json-encode.txt` (5.4: "2", "2.0", "9007199254740992.0"; LuaJIT: "2", "2", "9007199254740992"); `probes/149-luajit-json-number.txt`: "2^53 + 0.0 => {\"v\":9007199254740992}" (LuaJIT) vs "{\"v\":9007199254740992.0}" (5.4) |
| F8 | Stock LuaJIT has no `utf8` library; stock Lua 5.4 has one | `probes/203-luajit-utf8-coroutine.txt`: "Lua 5.1	nil	table	table" (LuaJIT) / "Lua 5.4	table	table	nil" |
| F9 | A `#[mlua::lua_module]` fn in a private `mod entry;` exports `luaopen_airsstack` and is clean under the workspace lints and clippy `-D warnings` | `probes/140-lua-module-private-mod.txt`: "T _luaopen_airsstack"; clippy `[exit 0]`; lua5.4 load prints "{\"observed_at\":1700000000,…}" |
| F10 | macOS needs `-undefined dynamic_lookup` for the cdylib (a `build.rs`) | `probes/06-macos-native-load.txt` (question: "with build.rs -undefined dynamic_lookup, does lua5.4 / luajit require() it"; source `probes/06-macos-native-load/build.rs:6` "cargo:rustc-cdylib-link-arg=dynamic_lookup"), then "T _luaopen_airsstack" |
| F11 | The release profile keeps the export | `probes/145-release-profile-export.txt`: "T _luaopen_airsstack" with lto thin, codegen-units 1, strip symbols |
| F12 | Two ABIs built in one target dir overwrite the one library; only the needed units rebuild | `probes/148-two-abi-target-dir.txt` (later switches: one `Compiling airsl-lua`) |
| F13 | `require` of the module returns a table and sets no global | `probes/06-macos-native-load.txt`: "table	nil"; `probes/188-require-twice.txt`: "global airsstack:	nil" |
| F14 | The real set (`standalone()` through `ModuleSet::open`, the §2.4 manifests, release profile) loads in stock Lua 5.4 and in LuaJIT: exactly the eleven modules, no `ext`, no global, the table in `package.loaded`, and one host call per module succeeds with trusted authority | `probes/230-real-module-set-stock-lua.txt`, under both `lua5.4` and `luajit`: "keys (11):	env:table fs:table glob:table hash:table hook:table json:table path:table proc:table regex:table stdio:table time:table", "ext:	nil", "global airsstack:	nil", "package.loaded is the table:	true", "fs       ok   hello from fs \| -- probe 2", "proc     ok   0 \| hi from proc\n \| ", every row `ok`, "[exit 0]"; json "[2,2.0]" (5.4) / "[2,2]" (LuaJIT) |
| F15 | A bare `cargo check`/`build`/`test` at the root fails once `airsl-lua` is a member, and passes with `default-members` leaving it out; `--workspace` still fails | `probes/232-default-members.txt`: control "`vendored`, `external` and `module` features are mutually exclusive" "[exit 101]"; with `default-members` "[exit 0]"; `--workspace` "[exit 101]" |

### 1.2 Decisions the author accepted on 2026-11-26

| # | Decision |
|---|---|
| S1 | Intent constraints 1–8 as written in `intent.md`. |
| S2 | Lockstep versioning: airsl, airsl-cli and airsl-lua share `[workspace.package] version`. Each airsl-cli release publishes both rocks at that version with rockspec revision `-1`; `--native` installs that version. |
| S3 | `--native` installs into the user tree (`--local`), then prints a path hint and no separate "installed" line. It also writes the type file and `.luarc` as plain `lua setup` does. Order: read-only luarocks detection → type file and `.luarc` → report → install → hint. |
| S4 | Lua `5.4` → `airsstack`; `5.1` → `airsstack-luajit` with no further LuaJIT check (the rockspec's `luajit >= 2.1` refuses PUC 5.1 and airsl relays it); any other `X.Y` is refused, naming `--lua 5.4`. `--lua 5.4\|luajit` (requires `--native`) becomes `--lua-version 5.4\|5.1` on both luarocks calls. Lua 5.5 is a non-goal. |
| S5 | airsl relays luarocks' failures rather than diagnosing the user's Lua setup; the book has a per-OS section on pointing luarocks at Lua 5.4 or LuaJIT, and the unsupported-version message names that page. |
| S6 | The error and edge tables of §5 and §6, with these defaults: a missing luarocks prints both install commands when `--lua` is absent; `config`'s stderr passes through; an empty answer and a non-version answer are separate messages; success prints the hint and no "installed" line; the report is printed and flushed before the install; rocks build in a dist global job; docs-check also checks `source.tag`; luarocks runs in airsl's working directory; the publish job fails on a taken name; an install failure adds no second line. |
| S7 | The module: every module except `ext`, trusted authority, no global, no memory ceiling; the LuaJIT JSON difference is documented; the values-and-json example's `1.7e9` claim is corrected. |
| S8 | Two packages with `build.type = "rust-mlua"`, `luarocks-build-rust-mlua == 0.2.7`, `cargo_extra_args = {"--package","airsl-lua","--locked"}`, `[lib] name = "airsstack"`; rockspecs checked in and renamed on every version bump; docs-check gains a fifth check. Binary rocks for `linux-x86_64`, `linux-aarch64` (manylinux2014 via `docker run`), `macosx-aarch64`, `macosx-x86_64`, `win32-x86_64` (MSVC); no musl. Binary rocks carrying `doc/` files is accepted. |
| S9 | Release: dist `precise-builds = true`; rocks built in a `global-artifacts-jobs` entry so a failed leg blocks the release; uploaded by a `publish-jobs` entry after `host`. Per package: upload the rockspec, then at once every rock. The publish job checks the API key first and fails when the name belongs to another account. Prerequisite: a luarocks.org account without upload two-factor, its key in the secret `LUAROCKS_API_KEY`. |
| S10 | Gate: `dod` drops `--all-features`; `airsl-lua` is excluded from `--workspace` runs; a native-load step builds each ABI and runs a check script in real `lua5.4` and `luajit`, failing (never skipping) when an interpreter is missing; a `rocks` task sits outside `dod`. CI installs interpreters with apt / Homebrew / MSVC source builds, not gh-actions-lua. `deny.toml`: skip `syn@2` and `allow-wildcard-paths`. `airsl-lua` depends on `airsl` by path. |
| S11 | Docs: book chapters, READMEs and `CLAUDE.md` as listed in §11; Neovim gets a book note only. |
| S12 | (2026-11-26, review finding 4) No source rock. A release uploads per package the rockspec, which creates the version, and the five binary rocks. A platform without a binary rock installs from the rockspec alone: luarocks clones `source.url` at `source.tag` and builds with cargo, so it needs git and cargo. This replaces M12's source packing. It departs from the approved intent, whose outcome (`intent.md:44` "uploads the source rock and the binary rocks") and constraint 3 (`intent.md:62` "source and binary rocks for both packages") name a source rock; the intent's other outcome, "from source with cargo" (`intent.md:41-42`), still holds through the rockspec. The intent is amended through the `intent` skill before this spec is approved, and `built-on` then re-confirmed. |
| S13 | (2026-11-26, review finding 4) The binary rocks do not carry `CLAUDE.md`. The rockspecs set `copy_directories = { "crates/airsl-lua/doc" }`, a directory holding only `LICENSE`; this replaces S8's "binary rocks carrying `doc/` files is accepted". |
| S14 | (2026-11-26) The Windows success hint names a cmd form and a PowerShell form (§4), the two that worked in `probes/221-windows-local-install-path.txt`. |
| S15 | (2026-11-26) The publish job uploads in two phases: phase 1 uploads both rockspecs, each followed by its root-manifest name check, and stops before any binary rock if either check fails; phase 2 then uploads all ten binary rocks (§10.3). This replaces S9's "per package: upload the rockspec, then at once every rock". |
| S16 | (2026-11-26) Library-only releases are forbidden: every release is a full lockstep release started by an `airsl-cli-v<version>` tag, with `airsl-v<version>` tagged on the same commit (§2.5). |
| S17 | (2026-11-26, review finding 5) The stock-Lua chapter ships with this change; its `luarocks install` and `airsl lua setup --native` sections carry no output block until the release, when a release step pastes the real output (§11). |
| S18 | (2026-11-26) Risks R12 and R13 are accepted. |

### 1.3 Tool and dependency versions this spec was proven against

mlua 0.12.0 (`Cargo.lock:485-488`), mlua-sys 0.11.0, mlua_derive 0.12.0 and syn 2.0.119 once
`module` is on (`probes/201-lockstep-manifests.txt`: "Adding mlua_derive v0.12.0"; "syn v2.0.119");
clap 4.6.6 (`Cargo.lock:148-149`); rustc/cargo 1.94.1 (`rust-toolchain.toml` channel 1.94; probe
headers); LuaRocks 3.12.2 locally and on the Windows/manylinux legs, 3.13.0 on Homebrew CI, 3.8.0 on
Ubuntu apt (`probes/162-y1-docker-manylinux-x86_64.txt`: "/opt/lr54/bin/luarocks 3.12.2";
`probes/167-y4-windows-msvc-luarocks.txt`: "luarocks 3.12.2"; `probes/165-y3-brew-macos-arm64.txt`:
"luarocks 3.13.0_1"; `probes/164-y2-ubuntu-apt.txt`: "luarocks	3.8.0+dfsg1-2"); luarocks-build-rust-mlua 0.2.7-1 (`probes/146-rockspecs-make-server.txt`);
cargo-deny 0.20.2 (`probes/144-cargo-deny-features.txt` header); dist 0.33.1 (`dist-workspace.toml:7`);
cargo-make 0.37.24 locally (`probes/233-native-load-task-form.txt`; the repo requires 0.37.0, `Makefile.toml:27`);
jq 1.7.1 locally, 1.8.1 on the Ubuntu 26.04 runner (`probes/234-publish-jq-checks.txt` header).

## 2. Architecture

**Approved by the author 2026-11-26** as a draft. This section is that draft with the amendments of
§2.10 applied (each amendment closes an item the draft marked unprobed, or applies a decision of
§1.2). **Amendments approved by the author 2026-11-26**: the amended tree of §2.1 with M1–M15,
including `default-members`, `ci/msvc-lua.cmd`, the root-manifest name check and
`crates/airsl-lua/doc/LICENSE`, was shown on its own and accepted.

### 2.1 Module tree

New `+`, changed `~`, removed `-`, unchanged context without a mark.

```
Cargo.toml                         ~ [workspace.package] version (lockstep); members += crates/airsl-lua;
                                     default-members = the other three (F15); mlua entry keeps only
                                     "serialize"; mlua comment rewritten
Cargo.lock                         ~ + mlua_derive 0.12.0 → syn 2.0.119 (pulled in by `module`)
deny.toml                          ~ [bans] allow-wildcard-paths = true; skip syn@2 with a reason
dist-workspace.toml                ~ global-artifacts-jobs += "./build-rocks"; precise-builds = true;
                                     publish-jobs = ["./publish-rocks"]
Makefile.toml                      ~ clippy/doc/test/test-doc/clippy-fix/dod-crate: no --all-features,
                                     airsl-lua excluded from --workspace and run per ABI;
                                     + task native-load (part of test); + task rocks (outside dod)
CHANGELOG.md                       ~ lockstep wording replaces "versioned independently"; the release
                                     entry carries a "### Breaking" item for airsl's features (§2.9 D2)
CLAUDE.md                          ~ build requirement, commands, docs-check's five checks, architecture
README.md                          ~ crate table, build and gate needs, versioning, releases (§11)
.github/workflows/
  ci.yml                           ~ dod job installs Lua 5.4 and LuaJIT per OS before `cargo make dod`
+ build-rocks.yml                    dist global-artifacts job: five platform legs, each builds, packs
                                     and loads both binary rocks; uploads them as `rocks-<arch>` artifacts
+ publish-rocks.yml                  dist publish job: key check, file check, upload of the rockspecs and
                                     the binary rocks to luarocks.org, root check (no source rock, S12)
  airsl-cli-release.yml            ~ regenerated by `dist generate`
  airsl-cli-smoke.yml, pages.yml     unchanged

crates/airsl/
  Cargo.toml                       ~ version.workspace = true; [features] default/lua54/luajit/vendored/send/module
  src/lib.rs                       ~ crate docs gain a "Cargo features" section (still export-only)
  src/version.rs                   ~ the manifest-equality test leaves (the number now lives in the workspace)
  src/builder.rs                   ~ install_modules delegates its loop to ModuleSet::install
  src/engine.rs                    ~ rustdoc: Send + Sync with the default `send` feature
  src/modules/mod.rs               ~ pub use stdlib::{standalone, stdlib}
  src/modules/registry.rs          ~ + ModuleSet::open (pub), ModuleSet::install (pub(crate))
  src/modules/stdlib.rs            ~ + standalone(); stdlib() = standalone() + Ext last
  src/sandbox/language_surface.rs  ~ libraries(): cfg(feature = "luajit") arms
  examples/values-and-json/main.rs, README.md  ~ LuaJIT claim corrected

+ crates/airsl-lua/                  the native module; cdylib; publish = false; no README
+   Cargo.toml                       version.workspace; [lib] name = "airsstack", crate-type = ["cdylib"];
                                     features lua54, luajit (no default); airsl by path, default-features = false,
                                     features = ["module"]; [package.metadata.dist] dist = false
+   build.rs                         macOS: -undefined dynamic_lookup
+   src/lib.rs                       crate docs + `mod entry;` only
+   src/entry.rs                     the one #[mlua::lua_module] fn → luaopen_airsstack
+   lua/check.lua                    assertions run by lua5.4 and luajit against a build
+   doc/LICENSE                      a copy of the root LICENSE, as crates/airsl and crates/airsl-cli carry;
                                     the rocks' only doc file (`copy_directories`, §8.1, S13)
+   rockspecs/airsstack-0.2.0-1.rockspec
+   rockspecs/airsstack-luajit-0.2.0-1.rockspec
+   ci/manylinux.sh                  the in-container build of the Linux rock legs
+   ci/msvc-lua.cmd                  builds Lua 5.4.8 and LuaJIT with MSVC; called by ci.yml and the
                                     Windows leg of build-rocks.yml (§9.4)

crates/airsl-cli/
  Cargo.toml                       ~ version.workspace = true; + thiserror = { workspace = true }
  src/main.rs                        unchanged (`mod lua;` now resolves to the folder)
  src/cli.rs                       ~ + LuaAbi (ValueEnum "5.4" | "luajit"); Setup gains --native, --lua
- src/lua.rs                         moved into the folder below
+ src/lua/mod.rs                     docs (with the category-1 exemption line), `mod command; mod luarocks;
                                     mod native;`, `pub(crate) use command::run;`
+ src/lua/command.rs                 `lua setup` dispatch, ordering, printing, exit code (today's lua.rs body)
+ src/lua/native.rs                 detection parse, package choice, install request, NativeError and messages
+ src/lua/luarocks.rs               trait Luarocks + System (std::process::Command), LuarocksError

crates/docs-check/
  Cargo.toml                       ~ + toml as a dev-dependency (workspace)
  src/lib.rs                       ~ + `#[cfg(test)] mod rockspecs;`; crate docs list the fifth guard
+ src/rockspecs.rs                   the fifth check: rockspecs and member versions match the workspace version
  src/examples.rs                  ~ the inner `cargo run` drops `--all-features`

book/src/                          ~ docs only (§11)
```

### 2.2 Why each new or reshaped file exists

| File | Why it exists | Shaped by |
|---|---|---|
| `crates/airsl-lua/` | `module` cannot share a build with `vendored`/`send` or a second Lua (F1), so the loadable library is its own build unit with its own features | M-SMALLER-CRATES (microsoft-guidelines.md); workspace.md § Directory layout, § Member shape |
| `airsl-lua/src/lib.rs` | crate root: docs and `mod entry;` only | mod-rs-export-only.md § The rule; unit-test-mandate.md exempt category 1 |
| `airsl-lua/src/entry.rs` | holds the one `#[mlua::lua_module]` fn (F9); `use airsl::mlua::{self, …}` because the expansion names `mlua` unqualified (`probes/05-feature-split-workspace.txt` (k1): "cannot find module or crate `mlua`"; (k2) `exit=0`) | mod-rs-export-only.md § Naming siblings; `CLAUDE.md` "mlua is re-exported as `airsl::mlua` deliberately" |
| `airsl-lua/build.rs` | macOS link args (F10) | unit-test-mandate.md exempt category 5 |
| `airsl-lua/lua/check.lua` | the only place the cdylib can run (F4) | `CLAUDE.md`: no `tests/` directories; the name matches neither `*_test.lua` nor `test_*.lua` (`crates/airsl-cli/src/test_runner.rs:51`), so `airsl test` never collects it |
| `airsl-lua/rockspecs/*.rockspec` | what luarocks builds and what luarocks.org lists; the `lua` dependency routes each package to its manifest (§8.2) | user layout rule: live with their context |
| `airsl-lua/ci/manylinux.sh` | the Linux legs build inside `docker run` because a job-level container cannot run GitHub's node actions on glibc 2.17 (§8.4) | `probes/161-y1-container-aarch64.txt`: "`GLIBC_2.28' not found (required by /__e/node20/bin/node)" |
| `airsl-lua/doc/` | luarocks copies every root `README*`, `LICENSE*`, `COPYING*` and `*.md` (so `CLAUDE.md`) into a rock's `doc/` unless `copy_directories` names a directory that exists; naming this one ships the licence alone | `probes/29-luarocks-3.12.2-src/build.lua:314-330` (`install_default_docs`, patterns `{ "readme", "license", "copying", ".*%.md" }` at :315), `:342-364` (defaults only `if not any_docs`); `probes/235-rockspec-only-install-and-docs.txt` |
| `airsl-lua/ci/msvc-lua.cmd` | the MSVC builds of Lua 5.4.8 and LuaJIT are needed twice (the `dod` job's interpreters, the Windows rock leg); one script keeps them identical | the steps of `probes/160-ci-probe-src/.github/workflows/probe-ci.yml` job Y4 (`probes/167-y4-windows-msvc-luarocks.txt`: "cl exit=0", "link dll exit=0", "msvcbuild exit=0") |
| root `Cargo.toml` `default-members` | a bare `cargo build`/`test`/`check` would otherwise unify `module` with `vendored`/`send` and fail for every contributor | F15 |
| `airsl/src/modules/registry.rs` (`open`, `install`) | the install loop, today private in `builder.rs:240-262`, becomes a `ModuleSet` operation so the engine and a foreign `mlua::Lua` share one loop | modularity.md Rule 2; M-ESSENTIAL-FN-INHERENT; static-dispatch.md (reuses the `// dyn:` seam at `registry.rs:151`) |
| `airsl/src/modules/stdlib.rs` (`standalone`) | the set without `ext` sits beside the one list of built-ins and `stdlib()` is built from it | `CLAUDE.md` "`modules::stdlib()` … is the single list of built-ins"; modularity.md Rule 2 |
| `airsl/src/sandbox/language_surface.rs` | LuaJIT's `StdLib` lacks `UTF8`/`COROUTINE` (F3); two match arms gain a cfg | user layout rule (no `language_surface_luajit.rs`); deviation D6 |
| `airsl/Cargo.toml` `[features]` | one crate serves the vendored engine (default, unchanged) and the module build | `probes/201-lockstep-manifests.txt` (§2.4); deviation D2 |
| `airsl-cli/src/lua/` (folder) | `lua setup` gains a second concept with distinct roles (dispatch, the native flow, the process boundary) and about thirty failure paths with tests | user layout rule ("folder only when distinct roles or large"; role names, no `lua/lua.rs`); not `lua_native.rs` beside `lua.rs` |
| `lua/mod.rs` | declarations and the one re-export | mod-rs-export-only.md § The rule |
| `lua/command.rs` | today's `lua.rs` (`crates/airsl-cli/src/lua.rs:31-52`) plus ordering the native steps; streams injected | M-IMPL-IO; modularity.md Rule 1 |
| `lua/native.rs` | decides what to install: parse, choose, build the request, own `NativeError` and its messages | modularity.md Rule 1; strong-types.md; errors live with their context |
| `lua/luarocks.rs` | the only code in the CLI that spawns a process; a trait so every failure path is unit-tested without luarocks | M-MOCKABLE-SYSCALLS; static-dispatch.md § The rule; M-DOCUMENTED-MAGIC |
| `cli.rs` `LuaAbi` | the flag value and the detection result are one concept, one type; flag enums live in `cli.rs` (precedent `PolicyName`, `crates/airsl-cli/src/cli.rs:64-73`) | strong-types.md § No bool; modularity.md Rule 2 |
| `docs-check/src/rockspecs.rs` | the rockspec names, `package`, `version` and `source.tag` must follow the workspace version, and luarocks refuses a file whose name disagrees with its contents (`probes/202-final-rockspecs.txt`: "Inconsistency between rockspec filename (airsstack-0.3.0-1.rockspec) and its contents (airsstack-0.2.0-1.rockspec)") | docs-check reads the whole repository, which is why it is its own crate (`crates/docs-check/src/lib.rs:1-11`) |
| `build-rocks.yml` | one definition of "build every binary rock", run on pull requests and at release, so a broken rock fails a PR (§10) | the repo's reason for `pr-run-mode = "upload"` (`dist-workspace.toml:32-35`) |
| `publish-rocks.yml` | the upload half only; `luarocks upload` sends one rock at most, so the job uses the HTTP API | `probes/41-luarocks-upload.txt` (upload.lua:33-46: args `rockspec [src_rock]`, no binary-rock loop) |

### 2.3 Dependency direction

```
              airsl  (lib; features lua54 luajit vendored send module; default = lua54 + vendored + send)
             ^      ^
   default   |      |   path dep, default-features = false, features = ["module"] (+ lua54 | luajit)
        airsl-cli   airsl-lua  (cdylib "airsstack"; publish = false; nothing depends on it)

   docs-check depends on no workspace crate (unchanged)
```

No edge between `airsl-cli` and `airsl-lua`: the CLI knows the rocks only by package name and
version, strings it hands to luarocks. `airsl-lua` is excluded from every `--workspace` command,
because a workspace build unifies its `module` with the others' `vendored`/`send`
(`probes/05-feature-split-workspace.txt` (a): "`vendored`, `external` and `module` features are
mutually exclusive"; (c) with `--exclude`: `exit=0`; `probes/46-clippy-toy-member.txt`).
`[workspace] default-members = ["crates/airsl", "crates/airsl-cli", "crates/docs-check"]` keeps a
bare `cargo build`/`test`/`check` working; it does not change `--workspace`, which still needs
`--exclude airsl-lua` (F15, `probes/232-default-members.txt`). An editor whose check passes
`--workspace` meets the same mlua error; `CLAUDE.md` says so (§11).

### 2.4 Features and how they reach mlua

The manifests, as parsed by cargo in `probes/201-lockstep-manifests.txt` (files in
`probes/201-lockstep-manifests/`, byte for byte). Root `Cargo.toml`:

```diff
@@ -1,9 +1,11 @@
 [workspace]
 resolver = "3"
 # `docs-check` is unpublished and test-only; the other two are the crates.io releases.
-members  = ["crates/airsl", "crates/airsl-cli", "crates/docs-check"]
+members  = ["crates/airsl", "crates/airsl-cli", "crates/airsl-lua", "crates/docs-check"]
 
 [workspace.package]
+# One number for airsl, airsl-cli and airsl-lua: the CLI installs the rocks at its own version.
+version       = "0.2.0"
 edition       = "2024"
 rust-version  = "1.94"
 license       = "Apache-2.0"
@@ -18,7 +20,7 @@
 # `serialize` bridges Lua values through serde.
 # Lua 5.4 (not 5.1/LuaJIT) because only 5.3+ distinguishes integers from floats,
 # which the JSON that host programs exchange with scripts depends on.
-mlua       = { version = "0.12", features = ["lua54", "vendored", "serialize", "send"] }
+mlua       = { version = "0.12", features = ["serialize"] }
 # Serialization
 serde      = { version = "1.0.228", features = ["derive"] }
 serde_json = "1.0.150"
```

Plus, under `members`, the two lines cargo parsed in `probes/232-default-members.txt` (byte for byte;
the comment is reworded in the file's own style):

```toml
# Bare `cargo build`/`test`/`check` leave out airsl-lua, whose `module` feature cannot share a build.
default-members = ["crates/airsl", "crates/airsl-cli", "crates/docs-check"]
```

`crates/airsl/Cargo.toml` (and `crates/airsl-cli/Cargo.toml` gets the same `version` line change):

```diff
@@ -1,7 +1,7 @@
 [package]
 name        = "airsl"
 description = "Embeddable Lua 5.4 runtime with a capability-gated sandbox and a host standard library"
-version     = "0.2.0"
+version.workspace      = true
 
 edition.workspace      = true
 rust-version.workspace = true
@@ -28,6 +28,14 @@
 path = "examples/notes-module/setup.rs"
 test = true
 
+[features]
+default  = ["lua54", "vendored", "send"]
+lua54    = ["mlua/lua54"]
+luajit   = ["mlua/luajit"]
+vendored = ["mlua/vendored"]
+send     = ["mlua/send"]
+module   = ["mlua/module"]
+
 [dependencies]
 mlua       = { workspace = true }
 serde      = { workspace = true }
```

`crates/airsl-lua/Cargo.toml`:

```toml
[package]
name        = "airsl-lua"
description = "The airsstack host modules as a native Lua module, built and shipped as LuaRocks rocks"
version.workspace      = true
publish     = false

edition.workspace      = true
rust-version.workspace = true
license.workspace      = true
repository.workspace   = true
homepage.workspace     = true
authors.workspace      = true

[lib]
name       = "airsstack"
crate-type = ["cdylib"]

[lints]
workspace = true

[features]
lua54  = ["airsl/lua54"]
luajit = ["airsl/luajit"]

[dependencies]
airsl = { path = "../airsl", default-features = false, features = ["module"] }

[package.metadata.dist]
dist = false
```

What each build resolves to (`probes/201-lockstep-manifests.txt`, `cargo tree -e features -i mlua`):

| Build | mlua features | Proof line |
|---|---|---|
| `-p airsl` (default) | default, error-send, lua54, send, serde, serialize, vendored | "mlua feature \"vendored\"" under `-p airsl` |
| `-p airsl-cli` | the same seven (unchanged from today, `Cargo.toml:21`) | same list under `-p airsl-cli` |
| `--workspace --exclude airsl-lua` | the same seven | same list |
| `-p airsl-lua --features lua54` | default, lua54, mlua_derive, module, serde, serialize | "mlua feature \"module\"" |
| `-p airsl-lua --features luajit` | default, luajit, mlua_derive, module, serde, serialize | "mlua feature \"luajit\"" |
| `-p airsl-lua` (no feature) | no Lua: the build fails with mlua's own error | tree lists no Lua feature; `probes/05-feature-split-workspace.txt` (f): "No Lua feature enabled. Please enable one of: …" |

The rock build adds `--features lua54` or `--features luajit` itself, chosen from luarocks' target
Lua (`probes/20-rust-mlua-backend-source.txt`, rust-mlua.lua:16-36; `probes/146-rockspecs-make-server.txt`:
"args=build --release --package airsl-lua --locked --target-dir=target --features lua54" and "… --features luajit").

`airsl-lua` depends on `airsl` by path because the workspace form cannot turn the defaults off:
`probes/141-workspace-default-features.txt`: "`default-features = false` cannot override
workspace's `default-features`" (exit 101). As an unpublished member it needs no `version` on that
dependency (`probes/140-lua-module-private-mod.txt` built it so).

Supported feature sets, documented in `airsl`'s crate docs: the default; `lua54,module` and
`luajit,module`, both only through `airsl-lua`. `luajit` without `module` compiles
(`probes/143-luajit-stdlib-cfg.txt`: `cargo check -p airsl --no-default-features --features
luajit,module --lib` `[exit 0]` with form A) but is not gated and not supported; it is not refused
with a `compile_error!`.

### 2.5 Versions (lockstep)

- One number, `[workspace.package] version`, today `0.2.0`; `airsl`, `airsl-cli` and `airsl-lua`
  inherit it with `version.workspace = true`.
- The workspace dependency `airsl = { version = "0.2.0", path = "crates/airsl" }` (`Cargo.toml:57`)
  is bumped with it, as today.
- The two rockspec files are renamed with it (`<package>-<version>-1.rockspec`).
- `docs-check` keeps its own `0.0.0` and `publish = false` (`crates/docs-check/Cargo.toml:4,7`).
- Both tags stay: `airsl-cli-v<version>` starts dist (`dist-workspace.toml:22-24`) and is the
  rockspecs' `source.tag`; `airsl-v<version>` names the library's release.
- Library-only releases are forbidden (S16). Every release bumps the shared number, publishes
  `airsl` and `airsl-cli` at it, tags `airsl-v<version>` and `airsl-cli-v<version>` on the same
  commit, and the `airsl-cli-v*` tag runs dist, which builds and uploads the rocks (§10).
  - Where the rule is written: `README.md` § Releases (`:78-79` today says the crates are tagged on
    their own "because the two are versioned independently") and `CHANGELOG.md:7-9` ("The two crates
    are versioned independently …") are rewritten to state it (§11), and so are the two crate
    READMEs' Releases sections (`crates/airsl/README.md:101-102`, `crates/airsl-cli/README.md:94-97`).
    `CLAUDE.md` does not describe releases (its one mention, `CLAUDE.md:143`, is about release
    archives), so it gains nothing for this rule.
  - What enforces it: the number is enforced, the tagging is not. docs-check's fifth check fails
    unless `airsl`, `airsl-cli` and `airsl-lua` inherit the workspace version and both rockspecs
    name it and the tag `airsl-cli-v<version>` (§5.6.3), so the crates cannot carry two numbers and
    the rockspecs cannot name a tag other than the CLI's. Nothing in the repository can stop a
    maintainer pushing an `airsl-v*` tag alone or publishing `airsl` alone to crates.io; such a tag
    starts no workflow (`README.md:85` "An `airsl-v*` tag starts nothing"; `dist-workspace.toml:22-24`),
    so the written rule is the guard.
- The version bump of the release that ships this change is a middle-number bump, because adding
  `[features]` to `airsl` breaks a dependent that disables its defaults (D2;
  `CHANGELOG.md:3-6`: "for a `0.y.z` version the *middle* number is the slot Cargo treats as
  breaking"), and its CHANGELOG entry carries a `### Breaking` item (the heading the 0.2.0 entry
  uses, `CHANGELOG.md:24`): `default-features = false` now needs `lua54` or `luajit`, and
  `Engine` is `Send + Sync` only with the default `send` feature.

Cargo gives each member the workspace number (`probes/201-lockstep-manifests.txt`: "airsl 0.2.0",
"airsl-cli 0.2.0", "airsl-lua 0.2.0", "docs-check 0.0.0"). The rockspecs carry today's `0.2.0`
until the roadmap's release bumps the workspace version; nothing is published before that release
(intent constraint 8), so the rockspecs' `airsl-cli-v0.2.0` tag is never fetched in the meantime.

### 2.6 The public `airsl` API for a foreign `mlua::Lua`

Owner: `crates/airsl/src/modules/registry.rs` and `crates/airsl/src/modules/stdlib.rs`.

1. `ModuleSet::open(&self, lua: &mlua::Lua, root_table: &RootTable) -> Result<mlua::Table>` (`pub`).
   Installs every module of the set, in set order, into a state this crate did not create, with
   trusted authority (`Policy::trusted()`, `crates/airsl/src/sandbox/policy.rs:61`), and returns the
   root table holding one subtable per module. It builds the `InstallContext` itself, so
   `InstallContext::new` stays `pub(crate)` (`registry.rs:41`) and the type keeps
   `#[non_exhaustive]` (`registry.rs:33`). It does not set a global, register the table for airsl's
   `require` (`require::root::register`, called at `builder.rs:259`), arm a memory or instruction
   ceiling, or load, withhold or reshape any Lua library. Errors: `Error::ModuleInstall` when a
   table cannot be created or set, or whatever a module's `install` returns. Its shape is the one
   probe 140 compiled and loaded (`probes/140-lua-module-private-mod/crates_airsl_src_modules_registry.rs.diff`).
2. `ModuleSet::install(&self, lua, policy: &Policy, root_table) -> Result<mlua::Table>`
   (`pub(crate)`): the loop moved out of `builder.rs:251-258`. `open` calls it with
   `Policy::trusted()`; `install_modules` calls it with the engine's policy and then does what stays
   engine-only: `require::root::register` (`builder.rs:259`) and the global (`builder.rs:260`).
3. `modules::standalone() -> Result<ModuleSet>` (`pub`, in `stdlib.rs`, re-exported beside
   `stdlib` at `modules/mod.rs:62`): every built-in except `ext`, in `stdlib()` order. `stdlib()`
   becomes `standalone()` plus `Ext::new()` last, which is today's order (`stdlib.rs:22-35`, `Ext`
   inserted last at `:35`).

The native entry point is one expression with no decision in it:
`Ok(airsl::modules::standalone()?.open(lua, &RootTable::default())?)`, each `?` converting through
the existing `impl From<Error> for mlua::Error` (`crates/airsl/src/error.rs:465`). That exact body
built under both ABIs and loaded the eleven modules (F14; source
`probes/230-real-module-set-stock-lua/crates/airsl-lua/src/entry.rs`).

Why this shape: `open` takes no `&Policy` because a `Policy` carries a `LanguageSurface` and
`ResourceLimits` that cannot be applied to a state airsl did not create (F6); accepting one would
let a caller pass `Policy::confined()` and believe in a ceiling that does not exist
(strong-types.md § The bar). Concrete parameter types, no new `dyn` (static-dispatch.md § The rule).
`mlua::Lua`/`mlua::Table` in the signature are already the public contract (`crates/airsl/src/lib.rs:65`
`pub use mlua;`; M-DONT-LEAK-TYPES). `open` and `standalone` get happy-path doctests under the
default features (strict-quality.md § Doctest requirements).

### 2.7 How logic is tested (summary; §9 has the full list)

- Inline `#[cfg(test)] mod tests` under default features for every logic-bearing file: registry,
  stdlib, CLI parse, the native flow through a scripted fake `Luarocks`, the process boundary,
  ordering in `command.rs`, and the fifth docs-check guard.
- `airsl-lua` has no Rust tests (F4); it is exercised by `lua/check.lua` in a real `lua5.4` and a
  real `luajit` inside `cargo make dod` (native-load), on every CI OS.
- The rocks themselves are built, packed and loaded on every pull request by `build-rocks.yml`
  (dist runs global jobs on PRs under `pr-run-mode = "upload"`; `probes/190-dist-rocks-global-job.txt`:
  "custom-build-rocks: needs [plan, build-local-artifacts], no `if` (runs on PRs too …)").

### 2.8 Guideline rules applied

Guideline: `claudestacks-guideline-rust` (rust-guidelines skill and every reference it indexes).

| Rule | Reference | How applied |
|---|---|---|
| DoD gate: fmt, clippy, doc, test, doctests, `-D warnings` | SKILL.md § Definition of Done; strict-quality.md § Zero warnings | the five steps stay; each covers the workspace and both ABIs of `airsl-lua` (§9.3) |
| `--all-targets` for clippy and test | SKILL.md § Definition of Done | kept on every clippy run including `airsl-lua` |
| Workspace DoD: metadata, `cargo tree -d`, members listed, lints inherited | workspace.md § Definition of Done | `airsl-lua` in explicit `members`; `[lints] workspace = true`; `*.workspace = true`; the one new duplicate (`syn` 2 beside 3, `probes/201-lockstep-manifests.txt`: "syn v2.0.119" / "syn v3.0.3") acknowledged in `deny.toml` |
| No `[profile.*]` in members; members under `crates/`; name == directory | workspace.md § Root shape, § Directory layout, § Naming | `crates/airsl-lua`, package `airsl-lua`; the release profile stays at the root and rock builds use it (F11) |
| lib.rs / mod.rs export-only | mod-rs-export-only.md § The rule | `airsl-lua/src/lib.rs` and `airsl-cli/src/lua/mod.rs` hold docs, `mod`, `pub(crate) use` only |
| Four-question `//!` on every module file | mod-rs-export-only.md § Required module documentation; M-MODULE-DOCS; `CLAUDE.md` module doc shape | every new file; `command.rs`'s doc no longer says nothing runs (it now spawns luarocks) |
| Files named for what they own; tests inline at the bottom | mod-rs-export-only.md § Naming siblings | `entry.rs`, `command.rs`, `native.rs`, `luarocks.rs`, `rockspecs.rs`; no `*_tests.rs` |
| No God object | modularity.md Rule 1 | process (`luarocks.rs`) ≠ decisions (`native.rs`) ≠ dispatch and output (`command.rs`) |
| No duplicate types or lists | modularity.md Rule 2 | one install loop, one built-in list, one `LuaAbi` for flag and detection, `Package` derived from `LuaAbi` |
| Static dispatch first | static-dispatch.md § The rule, § DoD grep | `L: Luarocks` generic; `ModuleSet::open` concrete; no new `Box<dyn`/`Arc<dyn` |
| Domain types, no `bool` params | strong-types.md § The bar, § No bool parameters | `LuaAbi`, `Package`, `NativeRequest` (clap's `native: bool` converted at the boundary into `Option<NativeRequest>`), `RootTable` in `open`, `Flavour` for the platform-specific message |
| Errors are enums/structs | strong-types.md § Errors from validating constructors; M-ERRORS-CANONICAL-STRUCTS | `LuarocksError` in `luarocks.rs`, `NativeError` in `native.rs`; nothing added to airsl's `error.rs` |
| Mockable syscalls | M-MOCKABLE-SYSCALLS | `Luarocks` trait; every luarocks failure path unit-tested |
| Sans-IO streams | M-IMPL-IO | `run(command, &mut impl Write, &mut impl Write)` kept (`lua.rs:31`) |
| Magic strings documented | M-DOCUMENTED-MAGIC | argv parts (`config`, `lua_version`, `install`, `--local`, `--lua-version`, `path`) as commented consts naming the luarocks behaviour they rely on |
| `Debug` on public types, `Display` on human-facing ones | M-PUBLIC-DEBUG, M-PUBLIC-DISPLAY | no new public types in airsl beyond methods; CLI errors implement `Display` |
| Documented pub items with `# Errors` and doctests | strict-quality.md § Doctest requirements; M-CANONICAL-DOCS | `ModuleSet::open`, `modules::standalone` |
| Inline unit tests for logic-bearing files | unit-test-mandate.md § The mandate; :25 (exempt files carry a `//!` line naming the category) | §9.1; exemptions, each with that `//!` line: `airsl-lua/src/lib.rs` and `airsl-cli/src/lua/mod.rs` (category 1), `airsl-lua/build.rs` (category 5); `entry.rs` is D4 |
| Panics never cross the C boundary | M-PANIC-IS-STOP; workspace `panic = deny` | `entry.rs` returns `mlua::Result`; no `unwrap`/`expect` |
| DLL state isolation | M-ISOLATE-DLL-STATE; M-AVOID-STATICS | airsl-lua adds no static; the existing process statics (`env::overlay`, `crates/airsl/src/modules/env.rs:300-303`; `time::ORIGIN`, `crates/airsl/src/modules/time.rs:37`) are shared by every Lua state in the process, as every engine shares them today; airsl-lua's crate docs say so |
| `unsafe_code = forbid` | M-UNSAFE; workspace lints | `#[mlua::lua_module]` in a submodule passes `forbid(unsafe_code)` and clippy (F9) |
| No leakage; quoted output is run | doc-comment-discipline.md § The leakage rule, § The accuracy rule | no chain/probe references in source or rockspec text; luarocks text quoted in rustdoc or the book is pasted from a run |
| Feature-gated `#[allow]` only when a lint fires in one configuration | strict-quality.md § Lint suppressions; M-LINT-OVERRIDE-EXPECT | form A gates whole match arms, so no lint fires per ABI (`probes/143-luajit-stdlib-cfg.txt`: both ABIs `[exit 0]`, default build `[exit 0]`) |
| No flat `<concept>_<qualifier>`; role names; errors with context | user layout rule (`~/.claude/CLAUDE.md` § Module layout) | `airsl-cli/src/lua/{command,native,luarocks}.rs`; no `lua_native.rs`; no `language_surface_luajit.rs` |

### 2.9 Deviations, each with its reason

- **D1 — `--all-features` leaves every gate command** (SKILL.md § Definition of Done "Keep it in
  every command"; strict-quality.md § All tests green). `airsl` must forward mlua's mutually
  exclusive features (F1), so an all-features build is a compile error by construction
  (`probes/46-clippy-toy-member.txt`: workspace clippy "exit=101"). Coverage instead: the default
  build over the workspace plus one clippy/doc/build/interpreter run per ABI. docs-check's inner
  `cargo run` drops it too (`crates/docs-check/src/examples.rs:202`, test `:473`), keeping its stated
  purpose that the inner build is the outer build (`examples.rs:26-34`). `deny.toml` keeps
  `[graph] all-features = true` (`deny.toml:17`), because cargo-deny resolves the union without
  compiling it (`probes/144-cargo-deny-features.txt` (d): "advisories ok, bans ok, licenses ok,
  sources ok", exit 0).
- **D2 — non-additive features on `airsl`** (M-FEATURES-ADDITIVE). `lua54`/`luajit` are exclusive
  and `module` removes `Send + Sync` from `Engine` (F2). Forced by mlua (F1) and by reusing the same
  modules in the native library. Contained: the default is today's build (§2.4 table); two
  non-default sets are supported, only through `airsl-lua`. Side effect: a dependent that writes
  `airsl = { default-features = false }` now gets no Lua and fails with "No Lua feature enabled"
  (`probes/141-workspace-default-features.txt` V2d).
- **D3 — no `cargo test` and no doctests on `airsl-lua`** (SKILL.md § Definition of Done). F4.
  Covered by the interpreter check (§9.2).
- **D4 — `airsl-lua/src/entry.rs` has no inline tests** (unit-test-mandate.md § The mandate,
  § Required justification). The test cannot be built at all (F4), not merely "covered elsewhere";
  the file is one delegating expression whose logic is unit-tested in `airsl`. Its `//!` says so.
  Permanent, not a deferral, so no tracking reference applies.
- **D5 — `dod` needs two Lua interpreters** (M-OOBE). `cargo test -p airsl` and
  `cargo test --workspace --exclude airsl-lua` still need nothing; only `cargo make dod` and its
  `test` step do, by the decision that the gate tests the module in real interpreters (S10). The
  Makefile's own comments that the gate is exactly the guideline's five commands are rewritten.
- **D6 — cfg-gated match arms in `language_surface.rs`, not a separate file**
  (mod-rs-export-only.md § Why: "Feature-gated implementation lives in its own file"). The gated
  part is two arms of one `match`; a sibling file is the flat `<concept>_<qualifier>` the user's
  layout rule forbids. The user's rule wins.
- **D7 — members inherit `version` from `[workspace.package]`** (workspace.md § Member shape:
  "members carry their own `version`"). Lockstep is S2: the rock version must equal the CLI that
  installs it. `docs-check` keeps `0.0.0`.
- **D8 — `airsl-lua` depends on `airsl` by path** (workspace.md § Inter-crate dependencies "prefer
  `{ workspace = true }`"). The workspace form is a hard error (§2.4).
- **D9 — the cdylib's target name differs from the package name** (`[lib] name = "airsstack"` in
  `airsl-lua`; workspace.md § Naming). The backend copies `lib<name>.<ext>` on unix and
  `<module key>.dll` on Windows (`probes/20-rust-mlua-backend-source.txt`, rust-mlua.lua:86-94), and
  Lua looks for `luaopen_<module>`, so the lib name must be the module name on all three OSes.
- **D10 — `bans.skip` for syn 2 and `allow-wildcard-paths`** (workspace.md § DoD "no unexpected
  duplicates"). `module` brings `mlua_derive` → syn 2 beside syn 3, and the path dependency has no
  version (`probes/144-cargo-deny-features.txt` (a): "bans FAILED" with `error[wildcard]` and
  `error[duplicate]`); (b) graph exclude alone and (c) skip alone each still fail; (d) passes.
- **D11 — `dod` no longer equals the guideline's five literal commands**: each step runs more than
  one cargo command (default workspace + per ABI) and `test` also runs native-load. The step names
  and their order stay.

### 2.10 Amendments to the approved draft

| # | Draft said | Now | Proof |
|---|---|---|---|
| M1 | path dependency "[UNPROBED]" | proven required | `probes/141-workspace-default-features.txt` |
| M2 | rockspec drift guard in `airsl-cli/src/lua/native.rs` tests | a fifth docs-check check (`rockspecs.rs`), which also checks member version inheritance | S8; `crates/docs-check/src/lib.rs:1-11` (reads the whole repository) |
| M3 | `rocks.yml` reusable workflow called by `publish-rocks.yml` | `build-rocks.yml` as a dist global-artifacts job (`host` waits for it); `publish-rocks.yml` uploads only | S9; `probes/190-dist-rocks-global-job.txt`: "`(needs.custom-build-rocks.result == 'skipped' \|\| needs.custom-build-rocks.result == 'success')`" in `host`'s `if` |
| M4 | deny: skip syn 2, "graph unchanged" | skip syn@2 **and** `allow-wildcard-paths = true` | `probes/144-cargo-deny-features.txt` (d) |
| M5 | `language_surface.rs` cfg form open | form A (cfg on whole arms) | `probes/143-luajit-stdlib-cfg.txt` form A `[exit 0]` on both ABIs and the default build |
| M6 | possibly an extra `clippy -p airsl --no-default-features …` step | not needed: `clippy -p airsl-lua` lints `airsl` in module mode | `probes/142-clippy-lints-dependency.txt` (planted lint in airsl fails `-p airsl-lua`, exit 101; `--no-deps` passes) |
| M7 | `build_dependencies = { "luarocks-build-rust-mlua >= 0.2.7" }` | `== 0.2.7` | `probes/146-rockspecs-make-server.txt`: "depends on luarocks-build-rust-mlua 0.2.7 (0.2.7-1 installed: success)" |
| M8 | Q1–Q10 open | settled: Q1 yes (S3); Q2 no extra check (S4); Q3 docs-check (M2); Q4 build-rocks on PRs (M3); Q5 fail, never skip (S10); Q6 capture `config` stdout, inherit stderr, inherit both for `install`, stdin null (§4); Q7 first lockstep number is today's `0.2.0`, both tags kept (§2.5); Q8 no README for airsl-lua; Q9 Neovim book note (S11); Q10 not refused (§2.4) | §1.2 |
| M9 | CI interpreters "[UNPROBED]" | apt / Homebrew / MSVC source builds; gh-actions-lua dropped | §9.4 |
| M10 | Linux rocks "in manylinux2014 containers" | `docker run` inside an ordinary job; a job-level `container:` cannot run node actions | `probes/160-y1-container-x86_64.txt`, `probes/161-y1-container-aarch64.txt` |
| M11 | `version.rs` test reads the root manifest | the manifest-equality test is removed from `version.rs` (its `package.version` is no longer a string, §2.4 diff) and docs-check owns the lockstep check; `the_version_has_three_numeric_components` stays | `crates/airsl/src/version.rs:35-40` (the test; `:37-38` read `manifest["package"]["version"].as_str()`) |
| M12 | src rocks packed in the rocks workflow | no source rock at all (author, S12): a release uploads the rockspecs and the binary rocks; a platform without a binary rock installs from the rockspec, which clones `source.url` at `source.tag` | `probes/235-rockspec-only-install-and-docs.txt` |
| M13 | (not in the draft) | root `default-members` without `airsl-lua` | F15 |
| M14 | (not in the draft) | `ci/msvc-lua.cmd` is new, shared by `ci.yml` and `build-rocks.yml`; `crates/airsl-lua/doc/LICENSE` is new (S13) | §9.4, §8.1 |
| M15 | publish job checks "`is_new` with empty `manifests`" | the root-manifest check of §10.3 step 5, on every upload | `probes/231-luarocks-site-root-membership.txt` |

## 3. Components

### 3.1 `crates/airsl`

- **`[features]`** as in §2.4. Crate docs (`src/lib.rs`) gain a "Cargo features" section naming the
  default and the two module sets, and that `default-features = false` without a Lua feature does
  not build.
- **`sandbox/language_surface.rs`**: `libraries()` keeps today's arms under
  `cfg(not(feature = "luajit"))` and adds LuaJIT arms (`Restricted` = string, table, math, os;
  `Minimal` = string, table, math), exactly the form A diff of `probes/143-luajit-stdlib-cfg/formA.diff`.
  These arms are only compiled in a LuaJIT build, which never creates a `LanguageSurface` state
  (`open` loads no library), so their content affects no shipped behaviour.
- **`modules/registry.rs`**: `ModuleSet::open` and `ModuleSet::install` (§2.6).
- **`modules/stdlib.rs`**: `standalone()`; `stdlib()` built from it.
- **`builder.rs`**: `install_modules` calls `ModuleSet::install`, then `register` and the global.
- **`engine.rs`**: rustdoc states `Engine` is `Send + Sync` with the default `send` feature.
- **`version.rs`**: M11.

### 3.2 `crates/airsl-lua`

- **`Cargo.toml`** as in §2.4.
- **`build.rs`**: on `target_os = "macos"` prints `cargo:rustc-cdylib-link-arg=-undefined` and
  `cargo:rustc-cdylib-link-arg=dynamic_lookup` (the shape of
  `probes/140-lua-module-private-mod/crates/airsl-lua/build.rs`). Its `//!` names the unit-test
  exemption, category 5 (build script), and why: it only prints link arguments
  (unit-test-mandate.md:25 "When the file ships under an exemption, it MUST carry a `//!` line near
  the top citing which category and why"). Nothing on Linux, where the build
  links (`probes/162-y1-docker-manylinux-x86_64.txt`: "make exit=0"), or on Windows, where mlua links
  `lua54`/`lua51` through `raw-dylib` in module mode (`probes/63-windows-module-linking.txt`,
  mlua-sys build/main_inner.rs:29-36).
- **`src/lib.rs`**: crate `//!` (what the library is, that it is installed through LuaRocks, that it
  runs with trusted authority and no ceilings, that process statics are shared between states, that
  it has no Rust tests and why) and `mod entry;`. A `//!` line names the unit-test exemption,
  category 1 (declarations only), as unit-test-mandate.md:25 requires.
- **`src/entry.rs`**: one `#[mlua::lua_module] fn airsstack(lua: &Lua) -> Result<Table>` whose body
  is `Ok(airsl::modules::standalone()?.open(lua, &RootTable::default())?)` (§2.6; the module
  function name `airsstack` yields the `luaopen_airsstack` symbol, F9 and
  `probes/230-real-module-set-stock-lua.txt` "T _luaopen_airsstack" for both builds). Its `//!` says
  why it has no tests (D4).
- **The module's behaviour in stock Lua** (the contract the book documents and `check.lua` checks),
  proven for the real set under `lua5.4` and `luajit` (F14, `probes/230-real-module-set-stock-lua.txt`):
  `require("airsstack")` returns a table with exactly the subtables `json`, `path`, `fs`, `env`,
  `proc`, `regex`, `hash`, `time`, `glob`, `stdio`, `hook` (`stdlib.rs:22-35` minus `ext`); `ext` is
  absent; no global is set (F13); `package.loaded.airsstack` holds that table and a second `require`
  returns the same table without calling `luaopen` again (`probes/188-require-twice.txt`:
  "rawequal(a, b)=true	opens=1"); host functions run with trusted authority (probe 230: `fs.write`
  then `fs.read` of a temp file, `proc.run({"echo", …})` with status 0, `env.set`/`env.get`,
  `glob.walk` of a temp directory, all `ok`) and no memory or instruction ceiling (F6).
- **`lua/check.lua`**: §9.2.
- **`rockspecs/`**: §8.1.
- **`ci/manylinux.sh`**: §8.4.

### 3.3 `crates/airsl-cli`

- **`cli.rs`**: `LuaAbi` ValueEnum with `#[value(name = "5.4")] Lua54` ("Lua 5.4 (PUC-Rio).") and
  `#[value(name = "luajit")] LuaJit` ("LuaJIT 2.1 (the Lua 5.1 ABI)."). `LuaCommand::Setup` gains
  `#[arg(long)] native: bool` ("Also install the native module with LuaRocks.") and
  `#[arg(long, value_enum, requires = "native")] lua: Option<LuaAbi>` ("The Lua to install the
  native module for, instead of the one luarocks targets."). Rendered help, from the mirror in
  `probes/180-clap-lua-flag.txt` (clap 4.6.6), `airsl lua setup -h`:

  ```
  Write a description of the built-in modules and add it to the language server's settings

  Usage: airsl lua setup [OPTIONS] [DIR]

  Arguments:
    [DIR]  Project directory to set up. Defaults to the working directory [default: .]

  Options:
        --native     Also install the native module with LuaRocks
        --lua <LUA>  The Lua to install the native module for, instead of the one luarocks targets [possible values: 5.4, luajit]
    -h, --help       Print help (see more with '--help')
  ```

  `--help` lists the values with their docs (`probes/180-clap-lua-flag.txt`: "- 5.4:    Lua 5.4
  (PUC-Rio)" / "- luajit: LuaJIT 2.1 (the Lua 5.1 ABI)"). The mirror's `Setup` doc carried only the first
  paragraph of today's doc comment (`crates/airsl-cli/src/cli.rs:290-295`), which is the line `-h` shows.
- **`lua/luarocks.rs`**: `pub(crate) trait Luarocks { fn query(&self, args: &[&str]) ->
  Result<Captured, LuarocksError>; fn run(&self, args: &[&str]) -> Result<Outcome, LuarocksError>; }`.
  `query` captures stdout, inherits stderr, gives stdin null; `run` inherits stdout and stderr,
  stdin null. `Captured { stdout: Vec<u8>, code: Option<i32> }`, `Outcome { code: Option<i32> }`
  (crate types, because tests cannot build a `std::process::ExitStatus` portably). `System {
  program: OsString }`, default `"luarocks"`, implements it with `std::process::Command`.
  `LuarocksError::Missing` for `io::ErrorKind::NotFound`, `LuarocksError::Spawn { source }` for any
  other spawn error (`probes/09-process-command-errors.txt`: "kind=NotFound", "kind=PermissionDenied").
- **`lua/native.rs`**: `Package` (`airsstack` / `airsstack-luajit`, from `LuaAbi`), `NativeRequest
  { lua: Option<LuaAbi> }`, `Flavour` (`Unix` / `Windows`, picked with `cfg!(windows)` at the call
  site and passed in so both texts are tested on every OS), the detection parse (§6.1), the install
  argv, the hint, and `NativeError` whose `Display` is the §5.2 text. `fn setup_native<L:
  Luarocks>(…)` runs detection and returns the chosen `Package`; the install step is a second
  function, so `command.rs` can write the type file and report in between.
- **`lua/command.rs`**: `pub(crate) fn run(command, stdout, stderr) -> i32` calls the private
  `run_with<L: Luarocks>(command, &System::default(), stdout, stderr)`; the flow of §4; `fail` as
  today (`lua.rs:49-52`).
- **`Cargo.toml`**: `version.workspace = true`; `thiserror = { workspace = true }` (`Cargo.toml:26`
  declares it for the workspace).

### 3.4 `crates/docs-check`

- **`src/rockspecs.rs`** (fifth check; test-only like the rest of the crate, `lib.rs:28-33`): reads
  `[workspace.package].version` of the root `Cargo.toml` with `toml` (as `crates/airsl/src/version.rs:37`
  already parses a manifest), the `[package] version` of `crates/airsl`, `crates/airsl-cli` and
  `crates/airsl-lua`, and every entry of `crates/airsl-lua/rockspecs/`. Rockspec lines are read as
  text: the first line starting `package = `, `version = ` and `   tag = `. Messages in §5.6.3.
- **`src/examples.rs`**: the inner `cargo run` no longer passes `--all-features`; the module doc
  (`examples.rs:26-34`) and the test at `:473` follow.
- **`Cargo.toml`**: `toml = { workspace = true }` under `[dev-dependencies]` (`Cargo.toml:48`
  declares it for the workspace).

### 3.5 Gate and release configuration

- `Makefile.toml`: §9.3. `deny.toml`: §9.5. `dist-workspace.toml`: §10.1. Workflows: §9.4, §10.

## 4. Data flow: `airsl lua setup [DIR] --native [--lua 5.4|luajit]`

```
S0  clap parse                         cli.rs        nothing written
S1  detect: spawn `config lua_version` luarocks.rs   read-only; stdout captured, stderr inherited, stdin null
    parse stdout, choose the package    native.rs     nothing written
S2  type file + .luarc                  command.rs →  airsl::luals::setup with stdlib() (unchanged; ext stays typed)
S3  print the report, flush stdout      command.rs    both files written
S4  spawn `install`                     luarocks.rs   stdout and stderr inherited, stdin null; luarocks writes its user tree
S5  print the hint                      command.rs    rock installed
```

The run stops at the first failure; later stages never start. A detection failure leaves the
project untouched; a type-file failure installs nothing; a failed report write does not install.
Detection runs also when `--lua` is given, so a missing luarocks fails before anything is written.
Without `--native` the flow is today's (`lua.rs:31-41`) and luarocks is never run.

Commands, exactly (`{version}` = `airsl::VERSION`, which lockstep makes airsl-cli's own version):

| Stage | Without `--lua` | With `--lua 5.4` | With `--lua luajit` |
|---|---|---|---|
| S1 | `luarocks config lua_version` | `luarocks --lua-version 5.4 config lua_version` | `luarocks --lua-version 5.1 config lua_version` |
| S4 | `luarocks install --local {package} {version}` | `luarocks --lua-version 5.4 install --local airsstack {version}` | `luarocks --lua-version 5.1 install --local airsstack-luajit {version}` |
| S5 hint, unix | `require("airsstack") works once you run: eval "$(luarocks path)"` | `… run: eval "$(luarocks --lua-version 5.4 path)"` | `… run: eval "$(luarocks --lua-version 5.1 path)"` |
| S5 hint, Windows (six lines) | `require("airsstack") works once luarocks' paths are set. In cmd:` / `  luarocks path > lrpath.bat` / `  call lrpath.bat` / `In PowerShell:` / `  $env:LUA_PATH = luarocks path --lr-path` / `  $env:LUA_CPATH = luarocks path --lr-cpath` | the same with `luarocks --lua-version 5.4 path` in all four commands | the same with `luarocks --lua-version 5.1 path` |

Why these commands hold:
- `config lua_version` prints `major.minor` on stdout, and an error goes to stderr
  (`probes/205-luarocks-streams.txt`: stdout "5.4"; `config no_such_key` stdout empty, stderr
  "Error: Unknown entry no_such_key", exit 1). `--lua-version` before the command overrides the
  default and is echoed (`probes/184-luarocks-lua51-unconfigured.txt`: "5.1"; `probes/185-luarocks-target-lua-missing.txt`:
  with `lua_version = "5.5"` configured, `luarocks config lua_version` prints "5.5" and
  `luarocks --lua-version 5.4 config lua_version` prints "5.4").
- `install --local <package> <version>` takes a version without revision and installs the highest
  revision of it (`probes/26-version-selection.txt`: `install … airsstack 0.0.1` prints "Installing …/airsstack-0.0.1-1.macosx-aarch64.rock";
  `probes/183-luarocks-revision-pick.txt`: "Installing …/airsstack-0.0.1-2.macosx-aarch64.rock").
  `--local` installs into `~/.luarocks` even inside a `luarocks init` project
  (`probes/187-luarocks-init-project-vs-local.txt`: "airsstack 0.0.2-1 is now installed in …/home-p7/.luarocks").
- `luarocks path` includes the user tree for the selected Lua: `probes/147-homebrew-luarocks-local.txt`
  (after `--local install`, `eval "$(luarocks path)"` then `require` prints
  ".luarocks/lib/lua/5.4/airsstack.so	0.2.0"); `probes/200-luarocks-path-luajit-hint.txt`
  (`--lua-version 5.1 path`: "export LUA_CPATH='…;<S>/p200/home/.luarocks/lib/lua/5.1/?.so'").
  On Windows `--local` installs into `%APPDATA%\luarocks` and `luarocks path` prints three
  `SET "VAR=…"` lines whose `LUA_CPATH` ends in that tree (`probes/221-windows-local-install-path.txt`:749
  "airsstack 0.0.0-1 is now installed in D:\a\_temp\zhome\AppData\Roaming\luarocks (license: Apache-2.0)";
  :835 `SET "LUA_CPATH=…;D:\a\_temp\zhome\AppData\Roaming\luarocks\lib\lua\5.4\?.dll"`; plain
  `luarocks path` printed the same lines, :839-841, as `--lua-version 5.4 path`, :834-836). The two
  forms the Windows hint names are the ones that made `require` work there: in cmd, saving the output
  to `lrpath.bat` and `call`ing it (:922-925, then "true" :980); in PowerShell, assigning
  `path --lr-path` and `path --lr-cpath` to `$env:LUA_PATH`/`$env:LUA_CPATH` (:1186-1191, then
  "LUA_CPATH=[D:\a\_temp\zhome\AppData\Roaming\luarocks\lib\lua\5.4\?.dll]" :1243 and "true" :1244).
  Piping `luarocks path` to `Invoke-Expression` does not work in PowerShell (:1100
  "LUA_CPATH env after iex: []", then :1101 "module 'airsstack' not found"), which is why the hint
  does not say "run the SET commands".

What a successful run prints: on stdout, today's two report lines (`crates/airsl/src/luals/setup.rs:98-107`;
test `prints_the_report_and_exits_zero` expects `"created .airsl/types/airsstack.lua\ncreated .luarc.json\n"`);
then luarocks' own install output straight on the terminal, ending in a line of the form
`airsstack <version>-1 is now installed in <home>/.luarocks (license: Apache-2.0)` (shape from
`probes/147-homebrew-luarocks-local.txt`: "airsstack 0.0.0-1 depends on lua 5.4 (5.4-1 provided by
VM: success)" and `probes/146-rockspecs-make-server.txt`: "… is now installed in … (license: Apache-2.0)");
then the hint of the table above (one line on unix, six on Windows). A second run prints `unchanged …` for both files,
luarocks' "already installed" notice, the hint, and exits 0 (§6.2).

## 5. Error handling

### 5.1 Location rules

- Every `lua setup` failure prints `airsl: {message}` on stderr and exits `1`
  (`crates/airsl-cli/src/lua.rs:49-51`). A hint is a further line indented seven spaces under the
  message (`crates/airsl-cli/src/test_runner.rs:50-51`). Messages start lowercase, have no trailing
  period, and put names and commands in backticks (`crates/airsl/src/error.rs:332`
  `` "{operation} failed on `{path}`: {source}" ``). One exception: a line that is a whole command
  for the user to copy and run (E4's install lines, the S5 hint's commands) is printed bare, so the
  line runs exactly as printed when pasted. A command or name inside a
  sentence keeps its backticks (E11).
- The success hint of S5 goes to stdout, starts at column 0, and indents a command under its shell
  by two spaces (§4).
- `2` is only clap's usage error (`book/src/appendix-01-cli.md:46-47`); no airsl code returns 2.
- `{command}` in a message is the exact luarocks command line of §4, as a user would type it.
- luarocks' own output is never captured or rewritten except `config`'s stdout; it appears above
  airsl's line.

### 5.2 Messages of `airsl lua setup --native`

`{version}` is `airsl::VERSION`; examples use `0.2.0`.

| # | Failure | Stage / owner | Message (stderr) | Exit | Written when it fails | Relayed |
|---|---|---|---|---|---|---|
| E1 | `--lua` value not `5.4`/`luajit` | S0 `cli.rs` (clap) | `probes/180-clap-lua-flag.txt`, e.g. for `5.1`: `error: invalid value '5.1' for '--lua <LUA>'` / `  [possible values: 5.4, luajit]` / blank / `  tip: a similar value exists: '5.4'` / blank / `For more information, try '--help'.` — `5.3` the same with `'5.3'`; `LuaJIT` without the tip line | 2 | nothing | — |
| E2 | `--lua` without `--native` | S0 clap `requires` | `error: the following required arguments were not provided:` / `  --native` / blank / `Usage: airsl lua setup --native --lua <LUA> [DIR]` / blank / `For more information, try '--help'.` (probe 180) | 2 | nothing | — |
| E3 | `--lua` twice | S0 clap | `error: the argument '--lua <LUA>' cannot be used multiple times` / blank / `Usage: airsl lua setup [OPTIONS] [DIR]` / blank / `For more information, try '--help'.` (probe 180) | 2 | nothing | — |
| E3b | `--lua` with no value or `--lua=` | S0 clap | `error: a value is required for '--lua <LUA>' but none was supplied` / `  [possible values: 5.4, luajit]` / blank / `For more information, try '--help'.` (probe 180) | 2 | nothing | — |
| E4 | `luarocks` not found (spawn `NotFound`, S1 or S4) | `luarocks.rs` → `LuarocksError::Missing`; text in `native.rs` | unix, no `--lua`:<br>`` airsl: `luarocks` was not found on PATH ``<br>`       install LuaRocks, then run the line for your Lua:`<br>`       luarocks --lua-version 5.4 install --local airsstack 0.2.0`<br>`       luarocks --lua-version 5.1 install --local airsstack-luajit 0.2.0`<br>unix, `--lua 5.4` (`luajit` alike):<br>`` airsl: `luarocks` was not found on PATH ``<br>`       install LuaRocks, then run: luarocks --lua-version 5.4 install --local airsstack 0.2.0`<br>Windows: the first line reads `` airsl: `luarocks.exe` was not found on PATH `` and a line ``        airsl runs `luarocks.exe` itself, not a `luarocks.bat` wrapper `` follows it; the rest is the same | 1 | S1: nothing; S4: both files, report printed | — |
| E5 | spawn fails, permission denied | S1/S4 `luarocks.rs` → `Spawn` | `` airsl: run failed on `luarocks`: Permission denied (os error 13) `` (io text: `probes/09-process-command-errors.txt` "Display=\"Permission denied (os error 13)\""); on Windows the program reads `` `luarocks.exe` ``, as in E4, because both name the program airsl spawned (the `Flavour` picks it) | 1 | as E4 | — |
| E6 | any other spawn I/O error | S1/S4 | `` airsl: run failed on `luarocks`: {io error} `` (`luarocks.exe` on Windows, as E5) | 1 | as E4 | — |
| E7 | `config` stopped by a signal (`code` is `None`, `probes/09-process-command-errors.txt` "status.code()=None") | S1 `native.rs` | `` airsl: `luarocks config lua_version` was stopped by a signal `` (the `{command}` of §4) | 1 | nothing | luarocks' stderr, if any |
| E8 | `config` exits non-zero | S1 `native.rs` | `` airsl: `luarocks config lua_version` failed with exit status 1 `` | 1 | nothing | luarocks' `Error: …` on stderr, above (probe 205) |
| E9 | `config` stdout empty after trimming | S1 `native.rs` | `` airsl: `luarocks config lua_version` printed no Lua version `` | 1 | nothing | — |
| E10 | `config` stdout not `digits.digits` (§6.1) | S1 `native.rs` | `` airsl: `luarocks config lua_version` printed `5.4.8`, which is not a Lua version `` (value shown per §6.1) | 1 | nothing | — |
| E11 | a version other than 5.4/5.1, without `--lua` | S1 `native.rs` | ``airsl: `luarocks` installs for Lua 5.5, and `airsstack` is built for Lua 5.4 and LuaJIT``<br>``        run again with `--lua 5.4` to install it for Lua 5.4 ``<br>``       https://airsstack.github.io/airsl/ch01-05-stock-lua.html shows how to point `luarocks` at Lua 5.4 or LuaJIT`` | 1 | nothing | — |
| E12 | `--lua` given and the printed version is another valid version | S1 `native.rs` | `` airsl: `luarocks --lua-version 5.1 config lua_version` printed `5.4`, not the 5.1 that `--lua luajit` asks for `` (for `--lua 5.4`: `` …, not the 5.4 that `--lua 5.4` asks for ``) | 1 | nothing | — |
| E13 | type file or `.luarc` fails | S2 `airsl::luals::setup` | today's texts unchanged, e.g. `` airsl: open project directory failed on `nope`: No such file or directory (os error 2) ``, `` airsl: will not write `{path}`: it is read-only ``, `` airsl: will not edit `{path}`: {reason}; add ".airsl/types" to `workspace.library` by hand `` (`crates/airsl/src/error.rs:256-258`, `:268`, `:332`) | 1 | as today | — |
| E14 | the report write to stdout fails | S3 `command.rs` | `airsl: {io error}` (today's test `a_failed_report_write_exits_one` expects `airsl: boom\n`) | 1 | both files; install not run | — |
| E16 | `install` exits non-zero, any reason | S4 `native.rs` | `` airsl: `luarocks install --local airsstack 0.2.0` failed with exit status 1 `` (the `{command}` of §4; the status is luarocks') | 1, never luarocks' status | both files, report printed; luarocks' tree as luarocks left it | everything luarocks printed, live (§5.4) |
| E17 | `install` stopped by a signal | S4 `native.rs` | `` airsl: `luarocks install --local airsstack 0.2.0` was stopped by a signal `` | 1 | as E16 | output up to the signal |
| E18 | the hint write fails | S5 `command.rs` | `airsl: {io error}` | 1 | files and rock installed | — |

There is no E15: an install that cannot be spawned is E4, E5 or E6 at S4. Not errors: luarocks warnings with exit
0 pass through; LuaJIT installs may print a spurious `Missing dependencies … lua 5.1 (not
installed)` and still exit 0 (`probes/146-rockspecs-make-server.txt`: those lines, then "[exit 0]").

### 5.3 Generic shapes

| Shape | Where it can occur | Message |
|---|---|---|
| required | `--lua` needs `--native` | E2 |
| wrong type / unknown value | `--lua` value outside the two; a luarocks answer that is not a version; a version that is not supported | E1; E10; E11 |
| empty | `--lua` with no value; `config` printed nothing | E3b; E9 |
| duplicate | `--lua` twice | E3 |
| disagreement | `--lua` and luarocks' answer differ | E12 |
| I/O | spawn, project files, stdout | E4–E6; E13; E14, E18 |
| child failure | luarocks exits non-zero or is killed | E7, E8, E16, E17 |

### 5.4 What luarocks prints for relayed failures (verbatim; airsl adds only its E8/E16 line)

| Cause | luarocks output | Its exit | Proof |
|---|---|---|---|
| version not on luarocks.org (any build before the release) | `Error: No results matching query were found for Lua 5.4.` / `To check if it is available for other Lua versions, use --check-lua-versions.` | 1 | `probes/26-version-selection.txt` |
| offline / server down | `Warning: Failed searching manifest: Failed downloading …/manifest-5.4 …` then the two lines above | 1 | `probes/26-version-selection.txt` |
| `airsstack-luajit` when luarocks' 5.1 is PUC Lua 5.1 | `Error: Could not satisfy dependency luajit >= 2.1: No results matching query were found for Lua 5.1.` | 1 | `probes/164-y2-ubuntu-apt.txt` (apt default `lua_version: 5.1  LUA=/usr/bin/lua5.1`) |
| no Lua 5.1 interpreter found | `Error: Could not find Lua 5.1 in PATH.` / `Please set your Lua interpreter with:` / blank / `   luarocks --lua-version=5.1 --local config variables.LUA </path/lua>` | 1 | `probes/184-luarocks-lua51-unconfigured.txt` |
| configured Lua missing (Homebrew 5.5 default not installed) | `Error: Lua 5.5 interpreter not found at /opt/homebrew/opt/lua` / blank / `Please set your Lua interpreter with:` / blank / `   luarocks --local config variables.LUA </path/lua>` | 1 | `probes/185-luarocks-target-lua-missing.txt` |
| no binary rock, no git | `Error: 'git' program not found. Make sure Git is installed and is available in your PATH (or you may want to edit the 'variables.GIT' value in file '<luarocks config>')` | 1 | `probes/235-rockspec-only-install-and-docs.txt` (git off `PATH`; the config path printed there is `/opt/homebrew/etc/luarocks/config-5.4.lua`) |
| no binary rock, no cargo | `Error: Build error: 'cargo' is not installed.` / `This rock is written in Rust: make sure you have a Rust` / `development environment installed and try again.` (after the clone) | 1 | `probes/235-rockspec-only-install-and-docs.txt` (rockspec-only server, cargo off `PATH`); `probes/24-local-server.txt`; `probes/20-rust-mlua-backend-source.txt` rust-mlua.lua:10-13 |
| no binary rock, no Lua headers | `` Error: Failed finding Lua header lua.h (searched at <dir>). You may need to install Lua development headers. You can use `luarocks config variables.LUA_INCDIR <path>` to set the correct location. `` | 1 | `probes/181-toy-rocks-and-no-headers.txt` |
| HOME unset or empty | `Error: /.luarocks/lib/luarocks/rocks-5.4 does not exist` / `and your user does not have write permissions in /` / blank / `Please check your permissions.` | 2 | `probes/186-luarocks-local-home-root.txt` |
| run as root (luarocks 3.12.2) | `The --local flag is meant for operating in a user's home directory.` / `You are running as a superuser, which is intended for system-wide operation.` / `To force using the superuser's home, use --tree explicitly.` | 1 | `probes/186-luarocks-local-home-root.txt` (cmd.lua:88-90 and the run) |

### 5.5 The module in stock Lua: what Lua shows

| Failure | Lua's text | Proof |
|---|---|---|
| LuaJIT build in Lua 5.4 / 5.4 build in LuaJIT (macOS) | `error loading module 'airsstack' from file '…/airsstack.so':` / `dlopen(…, 0x0006): symbol not found in flat namespace '_lua_insert'` (or `'_luaL_tolstring'`) | `probes/06-macos-native-load.txt` |
| the same on Linux | `…: undefined symbol: lua_rawgetp` / `…: undefined symbol: lua_pcall` | `probes/164-y2-ubuntu-apt.txt` |
| wrong ABI or the Lua DLL it links absent (Windows) | `error loading module 'airsstack' from file 'C:\mods54\airsstack.dll':` / `The specified module could not be found.` | `probes/100-msvc-module-load.txt` |
| built against a newer glibc than the system | `` /lib64/libc.so.6: version `GLIBC_2.28' not found (required by …/airsstack.so) `` | `probes/60-glibc-linux-aarch64.txt` (why rocks build on glibc 2.17) |
| Neovim loading the 5.4 build | `symbol not found in flat namespace '_luaL_tolstring'` from a catchable `pcall` | `probes/65-neovim-luajit-load.txt` |

The book quotes none of these except to say which package a Lua needs. What `require` shows when
`luaopen_airsstack` returns an error is not documented (it needs an out-of-memory table creation).

### 5.6 Gate, CI and release messages

#### 5.6.1 native-load (Makefile task)

- Interpreter missing: `` native-load: `{program}` was not found; install Lua 5.4 or set AIRSL_LUA54 to its interpreter ``
  and `` native-load: `{program}` was not found; install LuaJIT or set AIRSL_LUAJIT to its interpreter ``,
  `{program}` being the variable's value (defaults `lua5.4`, `luajit`). The task fails; it never skips.
  "Missing" means duckscript's `which` answers empty, for a bare name and for a path alike
  (`probes/233-native-load-task-form.txt`: "which nope-lua -> []" and "which /no/such/lua5.4 -> []",
  each followed by the message and "[exit 1]"; "which /opt/homebrew/bin/lua5.4 ->
  [/opt/homebrew/bin/lua5.4]" for an absolute path that exists).
- Build failure: cargo's own output, and the task fails (`exec --fail-on-error`).
- Copy failure: duckscript's own error, and the task fails: `[cargo-make] ERROR - Error while running
  duckscript: Source: Unknown Line: {n} - Path does not exist.` (`probes/233-native-load-task-form.txt`,
  the `PROBE_FROM=…/missing.dylib` run, "[exit 1]"). The task adds no message of its own: `cp` raises
  before a return value could be checked.
- `check.lua` failure: the interpreter's own error and exit status 1 (§5.6.2), and the task fails.

#### 5.6.2 `crates/airsl-lua/lua/check.lua`

Failures are raised with `error("check.lua: …")`; the interpreter prints it with its own prefix
and a traceback and exits 1 (`probes/206-lua-arg-and-error-exit.txt`: "lua5.4: sub/check.lua:2:
check.lua: example failure" … "[exit 1]", LuaJIT the same). Texts:

| Check | Message |
|---|---|
| missing or unknown argument | `check.lua: usage: <interpreter> check.lua <directory holding the built airsstack library> <lua54\|luajit>` |
| wrong interpreter (checked before `require`) | `check.lua: this check is for lua54, but the interpreter is {_VERSION}` when `arg[2]` is `lua54` and `_VERSION` is not `Lua 5.4` or a `jit` table exists; `check.lua: this check is for luajit, but the interpreter is {_VERSION} without jit` when `arg[2]` is `luajit` and `rawget(_G, "jit")` is not a table |
| `require` raised | `check.lua: require("airsstack") failed: {error}` |
| not a table | `check.lua: require("airsstack") returned a {type}, not a table` |
| global set | `check.lua: require("airsstack") set the global airsstack` |
| `package.loaded` | `check.lua: package.loaded.airsstack is not the table require returned` |
| `ext` present | `check.lua: ext is present, but it exists only inside airsl` |
| a module missing | `check.lua: module {name} is missing` (each of the eleven names of §3.2) |
| trusted authority | `check.lua: fs.read could not read the check script itself: {error}` |
| JSON | `check.lua: json.encode({2, 2.0}) gave {got}, expected {want}` (`want` is `[2,2.0]` under Lua 5.4 and `[2,2]` under LuaJIT, F7) |
| `utf8` | `check.lua: utf8 is a {type} under LuaJIT, expected nil` (F8) |

Success prints one line: `check.lua: ok (Lua 5.4)` or `check.lua: ok (Lua 5.1, {jit.version})`, `jit.version`
being already `LuaJIT 2.1.…` (`probes/230-real-module-set-stock-lua.txt`: "interpreter:	Lua 5.1	LuaJIT 2.1.1770848219").

#### 5.6.3 docs-check fifth check (`crates/docs-check/src/rockspecs.rs`)

Problems are collected one per line and asserted empty, as the citation guards do
(`crates/docs-check/src/citations/source.rs:111-132`: `"{shown}:{line}: cites …, which …"`).
With workspace version `0.2.0`:

```
crates/airsl-lua/rockspecs: has no `airsstack-0.2.0-1.rockspec`, which the workspace version 0.2.0 needs; a version bump renames both rockspecs
crates/airsl-lua/rockspecs/airsstack-0.2.0-1.rockspec:2: declares `package = "airsstack-lua"`, which does not match its file name; luarocks refuses such a rockspec
crates/airsl-lua/rockspecs/airsstack-0.2.0-1.rockspec:3: declares `version = "0.1.0-1"`, which does not match its file name; luarocks refuses such a rockspec
crates/airsl-lua/rockspecs/airsstack-0.2.0-1.rockspec:6: declares `tag = "airsl-cli-v0.1.0"`, which is not the release tag `airsl-cli-v0.2.0`
crates/airsl-lua/rockspecs/airsstack-0.2.0-1.rockspec: declares no `version = "…"` line
crates/airsl-lua/rockspecs/airsstack-0.1.0-1.rockspec: names version 0.1.0, which is not the workspace version 0.2.0; rename it rather than keep a copy
crates/airsl-lua/rockspecs/airsstack-0.2.0-2.rockspec: has revision 2; a release publishes revision 1 only
crates/airsl-lua/rockspecs/other-0.2.0-1.rockspec: names package other, which is neither airsstack nor airsstack-luajit
crates/airsl-lua/rockspecs/notes.txt: is not a rockspec; only the two rockspecs live here
crates/airsl/Cargo.toml: sets its own version, which must come from the workspace (`version.workspace = true`) so airsl, airsl-cli and airsl-lua share one number
```

(The `:2`, `:3`, `:6` are the lines the §8.1 layout puts those keys on; the check reports the line
where it found the key.)

Which checks a file gets: only a file named `<package>-<workspace version>-1.rockspec`, for one of the
two packages, gets the per-line checks (`package`, `version`, `tag`, missing line). Any other entry
gets exactly one message, the first of these that applies: not ending `.rockspec` ("is not a
rockspec"); a package other than the two ("names package"); another version ("names version …;
rename it rather than keep a copy"); another revision ("has revision"). So a leftover
`airsstack-0.1.0-1.rockspec` is reported once, by name, and its contents are not read.

Inputs the check cannot read, each one line, after which that input's other checks are skipped:

```
Cargo.toml: could not be read: {io error}
Cargo.toml: is not valid TOML: {toml error}
Cargo.toml: has no string `version` under `[workspace.package]`, which the rockspecs follow
crates/airsl/Cargo.toml: could not be read: {io error}
crates/airsl-lua/rockspecs: could not be listed: {io error}
crates/airsl-lua/rockspecs/airsstack-0.2.0-1.rockspec: could not be read: {io error}
```

A member manifest that is not valid TOML is reported as `{path}: is not valid TOML: {toml error}`.

#### 5.6.4 `publish-rocks.yml`

Every message is printed by the job's shell step, which then exits 1. The step stops at the first
failure; nothing after it runs.

| Check | When | Message |
|---|---|---|
| API key | first, before any download or request: the variable is tested for the empty string, which is what an undefined secret arrives as under `secrets: inherit` (`probes/220-publish-downloads-build-artifacts.txt`:302 "LUAROCKS_API_KEY length: 0") | `publish-rocks: LUAROCKS_API_KEY is empty; add the luarocks.org API key as a repository secret` |
| every file present | after the download, before any request | `publish-rocks: {file} is missing; nothing was uploaded` |
| transport | `curl` exits non-zero (no HTTP answer) | `publish-rocks: could not reach luarocks.org to upload {file}: curl exit status {code}` (curl's own `-sS` error line above it) |
| HTTP refusal | an answer outside 200–299 | `publish-rocks: luarocks.org refused {file} (HTTP {code}): {response body}` |
| no version id | `/upload` answers 2xx but `jq -e '.version.id \| numbers'` fails | `publish-rocks: luarocks.org answered {file} without a version id: {response body}` |
| name owned by another account | phase 1, after each `/upload`, before any binary rock of either package: the answer's `manifests` holds no entry whose `name` is `root` (`jq -e '.manifests \| any(.name == "root")'`) | `publish-rocks: luarocks.org did not put {package} in its root manifest, so the name {package} belongs to another account; no binary rock was uploaded for either package` |

Both `jq` tests were run on answers of that shape (`probes/234-publish-jq-checks.txt`): the root
test prints "true" "[exit 0]" for `manifests: [{"name": "root"}]` and "false" "[exit 1]" for an empty
list with `is_new` true or false and for a non-root manifest; the id test exits 4 for a string id or
none; a non-JSON body exits 5 ("jq: parse error"), which the step reports as "no version id" with the
body. A jq failure of either test fails the step.

Partial state: when any of the last four fails after an earlier upload of the same run succeeded,
a second line follows the message: `publish-rocks: uploaded before this failure: {files, in upload
order}; re-running the job replaces them` (§6.4). A name refusal lists its own rockspec there: the
`/upload` that revealed the refusal stored it.

**The name check, and why it holds on a re-run and on every later release**
(`probes/231-luarocks-site-root-membership.txt`, luarocks-site at the commit of probe 40):

- `/upload` recomputes `manifests` from the module's manifest memberships on every call
  (api.moon:285 `ManifestModules\select "where module_id = ?", module.id`, :288, :290 `json: { :module,
  :version, :module_url, :manifests, :is_new }`), not only when the module is new.
- The server tries to add a module to the root manifest only when it creates the module for this
  account (uploaders.moon:115-119 `new_module = true` when `Modules\find user_id: user.id, name:`
  misses; :157-160 `if new_module` → `ManifestModules\create root_manifest, mod`). The root holds one
  module per name (manifest_modules.moon:14 unique `(manifest_id, module_name)`; :26-27 "Manifest
  already has a module named …", which the uploader ignores). So a module refused at creation is
  never added later, and every later `/upload` of it, a re-run or the next release, answers
  `manifests` without `root`; the check fails each time.
- The root is the manifest named `root` (manifests.moon:43-44 `Manifests\find name: "root"`, :160-161
  `is_root: => @name == "root"`; modules.moon:238-243 `in_root_manifest` tests `m.name == "root"`),
  and a manifest row carries `name` (manifests.moon:11). The answer serializes model rows with their
  columns: the site's own test reads `res.version.id` and `res.module.id` and asserts
  `1, #res.manifests` for a first upload (spec/api_spec.moon:150, :155, :157-158).
- Not `#manifests == 0` (the client's warning, upload.lua:124, which also requires `is_new`): a
  module can sit in a non-root manifest (`/m/:manifest/manifest`, manifest.moon:125) and still be
  outside the root, which is the only manifest `luarocks install` reads by default; `/manifest` is
  built from the root's own modules (manifest.moon:80, :97-104; manifests.moon:100 `where
  manifest_id = ?`).
- No server answer names a module's owner before an upload: `check_rockspec` and `modules` look only
  at the caller's own modules (api.moon:215-228; the site test for a never-uploaded name gets `{}`,
  spec/api_spec.moon:125-133), and every module page route takes `:user` (modules.moon:67, :168). The
  check therefore runs after `/upload`. On a taken name, the first run leaves one rockspec in a
  module of our account outside the root manifest, which `luarocks install` from luarocks.org does
  not see (previous point). A re-run stores that rockspec again (updated in place, §6.4) and fails the
  same way, uploading no rock, until the name question is settled by hand.

## 6. Edge cases

### 6.1 The detection rule (owner `native.rs`)

Order, first match wins: a spawn error (E4–E6); no exit code, i.e. a signal (E7), whatever stdout
holds; a non-zero exit code (E8), whatever stdout holds, so `config` exiting 1 after printing `5.4`
is E8, never `airsstack`; then the stdout rule below (empty E9, shape E10, version E11, `--lua`
agreement E12).

Rule: take `config`'s stdout bytes; trim ASCII whitespace at both ends (`<[u8]>::trim_ascii`).
Empty → E9. Not valid UTF-8, or not ASCII digits `.` ASCII digits → E10, showing the value decoded
lossily and passed through `escape_debug`. Exactly `5.4` → `airsstack`; exactly `5.1` →
`airsstack-luajit`; any other `digits.digits` → E11 without `--lua`, E12 with it. With `--lua`, the
value must equal the requested `5.4`/`5.1`. No first-line parsing, no prefix match, no Unicode
trimming. luarocks prints `major.minor` (`probes/25-luarocks-config-keys.txt`, `probes/147-homebrew-luarocks-local.txt`,
`probes/185-luarocks-target-lua-missing.txt`: "5.5").

Checked examples, std behaviour from `probes/204-std-trim-escape.txt` (columns: trimmed bytes,
`utf8`, shown, `digits.digits`):

| stdout bytes | std result (probe 204) | Outcome |
|---|---|---|
| `5.4\n` | `"5.4"`, utf8, shown `5.4`, true | `airsstack` |
| `5.4` | `"5.4"` | `airsstack` |
| ` 5.4 \r\n` (Windows) | `"5.4"` | `airsstack` |
| `5.1\n` | `"5.1"` | `airsstack-luajit`; on PUC 5.1 the install fails with luarocks' `luajit >= 2.1` text (§5.4), E16 |
| `5.5\n` | `"5.5"`, true | E11 ``` `luarocks` installs for Lua 5.5, … ``` |
| `` / `\n` / `  \t ` | empty | E9 |
| `5.4.8\n` | `"5.4.8"`, false | E10 `` printed `5.4.8`, which is not a Lua version `` |
| breaker `5.4\n5.1\n` | `"5.4\n5.1"`, shown `5.4\n5.1`, false | E10 `` printed `5.4\n5.1`, … `` — never `airsstack` |
| breaker `5.4` + NBSP | not trimmed, shown `5.4\u{a0}`, false | E10 |
| breaker BOM + `5.4` | shown `\u{feff}5.4`, false | E10 |
| breaker `5.04\n` | `"5.04"`, true | E11 `… for Lua 5.04, …` — never `airsstack` |
| breaker `\xff5.4\n` | utf8 false, shown `�5.4` | E10 `` printed `�5.4`, … `` |
| breaker fullwidth `５.４` | utf8, shown `５.４`, false | E10 |
| `10.0\n` | `"10.0"`, true | E11 |
| `--lua luajit`, stdout `5.4` | — | E12 |
| breaker: exit 1, stdout `5.4\n` | — | E8 (status before stdout) |
| breaker: no exit code, stdout `5.4\n` | — | E7 (signal before stdout) |

### 6.2 Flow edges

| Case | Rule | Checked example |
|---|---|---|
| `--lua` and luarocks' default differ | `--lua` wins: `--lua-version` goes on both calls; the default is never read | Homebrew luarocks defaults to 5.5 (`probes/165-y3-brew-macos-arm64.txt`: "default lua_version: 5.5"); `--lua 5.4` runs `luarocks --lua-version 5.4 config lua_version` → `5.4` (`probes/185-luarocks-target-lua-missing.txt`: "5.4", then `--lua-version 5.4 install --local` "airsstack 0.0.2-1 is now installed in …") → installs `airsstack` |
| Homebrew without `--lua` | the 5.5 default is refused | E11 names `--lua 5.4` and the book page |
| `--lua 5.4` on Homebrew with keg-only `lua@5.4` | airsl passes no `--lua-dir`; luarocks may not find the interpreter (`probes/165-y3-brew-macos-arm64.txt`: "--lua-version 5.4 (no --lua-dir): … Error: Unknown entry LUA"); a binary-rock install needs no interpreter (`probes/185-luarocks-target-lua-missing.txt` header: "binary-rock install succeeded in both (a binary install runs no interpreter)"); any luarocks failure is E16 | the book's Homebrew section shows the `--lua-dir $(brew --prefix lua@5.4)` form that `probes/165-y3-brew-macos-arm64.txt` ran ("lua_version=5.4  LUA=/opt/homebrew/opt/lua@5.4/bin/lua5.4") |
| Ubuntu apt luarocks (3.8, default 5.1 = PUC) | detection `5.1` → `airsstack-luajit`; the install fails, relayed | `probes/164-y2-ubuntu-apt.txt`: "Could not satisfy dependency luajit >= 2.1 …" → E16; the book's apt section uses a LuaJIT-only directory with `--lua-dir`, the form that built and loaded there ("require(\"airsstack\").ok = true  json = [2,2]") |
| `--lua luajit`, luarocks finds no LuaJIT | airsl passes `--lua-version 5.1`; `config` still answers `5.1` (config is exempt from the interpreter check); install fails, relayed | `probes/184-luarocks-lua51-unconfigured.txt`: "5.1" then "Error: Could not find Lua 5.1 in PATH." → E16 |
| `--lua 5.1` | refused by clap: the flag names the VM, not the version | E1 with the tip `'5.4'` |
| run twice | type file `unchanged`; airsl passes no `--force`; luarocks prints a notice and exits 0 | `probes/182-luarocks-reinstall-downgrade.txt`: "airsstack 0.0.2-1 is already installed in …" / "Use --force to reinstall." `[exit 0]` → hint, exit 0 |
| an older version installed | luarocks installs the requested one and removes others (no `--keep`) | `probes/182-luarocks-reinstall-downgrade.txt`: "Removing airsstack 0.0.2-1..." `[exit 0]` |
| two revisions published | the highest revision of the version wins, even a rockspec-only one | `probes/183-luarocks-revision-pick.txt` (`-2` rockspec chosen over `-1` binary → source build) — why §10.3 uploads the binary rocks right after the rockspecs and never publishes a revision other than `-1` |
| `DIR` missing | detection runs (read-only), then E13; no install | `airsl lua setup nope --native` → `` airsl: open project directory failed on `nope`: No such file or directory (os error 2) `` |
| `DIR` relative or absolute | resolved against the working directory as today; luarocks always runs in airsl's working directory, never in `DIR`; `--local` makes the tree independent of both | `airsl lua setup ../proj --native` writes `../proj/.airsl/…` and installs into `~/.luarocks` |
| inside a `luarocks init` project | detection follows the project's Lua; `--local` still installs into `~/.luarocks` | `probes/187-luarocks-init-project-vs-local.txt`: `config lua_version` prints `5.1` inside a `--lua-version 5.1` project, `5.4` in a 5.4 one; `--local` install lands in `home-p7/.luarocks` |
| airsl version not on luarocks.org (any dev build, every build before the release) | passed as is; E16 with luarocks' "No results matching query" | `probes/26-version-selection.txt`; prereleases cannot exist: `crates/airsl/src/version.rs:43-50` requires three numeric parts |
| offline | detection and type file work; install fails, E16 | `probes/26-version-selection.txt` "Warning: Failed searching manifest …" |
| `HOME` unset or empty | airsl does not read `HOME`; luarocks fails with exit 2; E16 | `probes/186-luarocks-local-home-root.txt` "[exit 2]" → `… failed with exit status 2` |
| run as root | luarocks 3.12.2 refuses `--local`; E16 | `probes/186-luarocks-local-home-root.txt` "[exit 1]"; Debian's 3.8.0 decides "root" from `$USER` only and installs into `/root/.luarocks` when `USER` is unset (same probe header: Debian `fs/unix.lua:182-184`, "3.8.0 does not (run 3 (a) installs into /root/.luarocks)") |
| several luarocks on PATH | airsl does not choose: both calls spawn the bare name `luarocks` under the same `PATH`, and the printed commands name bare `luarocks`, which the user's shell resolves the same way | a design rule, nothing to run |
| Windows: `luarocks.exe` on PATH | found by `Command::new("luarocks")` | `probes/167-y4-windows-msvc-luarocks.txt`: `Command::new("luarocks") -> Ok status=Some(0)`; the official zip ships `luarocks.exe` ("C:\luarocks-dl\luarocks-3.12.2-windows-64\luarocks.exe") |
| Windows: only a `luarocks.bat` wrapper | not found; E4 with the Windows lines, which name the wrapper | `probes/167-y4-windows-msvc-luarocks.txt`: (b) `Command::new("luarocks") -> Err kind=NotFound`; airsl does not run `.bat` files, as `proc` does not (`crates/airsl/src/modules/proc.rs:283-291`) |
| no binary rock for the platform (musl, 32-bit, other OS) | luarocks installs from the rockspec: clones the repository at the tag (needs git), then builds (needs cargo, a C compiler, Lua headers and the backend); failures relayed (S12) | §5.4 rows "no cargo", "no Lua headers"; with apt's luarocks 3.8 (hosted on Lua 5.1) a 5.4 source build also needs the backend installed into its 5.1 tree first (`probes/164-y2-ubuntu-apt.txt`: "Failed initializing build back-end for build type 'rust-mlua'" until "preinstall backend into the 5.1 user tree") |
| musl Linux | luarocks names glibc and musl both `linux-<arch>`, so a musl luarocks takes the glibc rock | intent "Technical limits"; no load was run (`probes/191-musl-load-skipped.txt`); the book says musl is not supported |

### 6.3 The module in stock Lua

| Case | Rule | Checked example |
|---|---|---|
| `require` twice | Lua's cache returns the same table; `luaopen` runs once per state | `probes/188-require-twice.txt`: "2nd require: … rawequal(a, b)=true	opens=1" |
| `package.loaded.airsstack = nil`, then `require` | a new table, no error | same probe: "require ok=true … rawequal(a, c)=false	opens=2" |
| two processes | each opens its own copy | same probe: "process A: … opens=1", "process B: … opens=1" |
| no global | a script that relies on airsl's global must add `local airsstack = require("airsstack")` | F13 |
| `ext` | absent; the type file still declares it (intent constraint 7) | `check.lua` asserts it; the book says `ext` exists only in airsl |
| JSON integral float | documented, not compensated | F7: `json.encode(2.0)` is `2.0` on 5.4 and `2` on LuaJIT |
| `utf8` | stock LuaJIT has none; the module adds none | F8 |
| `env.set` and `os.getenv` | `env.set` writes airsl's process-wide overlay (§2.8 "DLL state isolation"), which `env.get` and `proc.run` read; Lua's own `os.getenv` does not see it; documented, not changed | `probes/230-real-module-set-stock-lua.txt`: "env      ok   set-by-env \| string \| nil" (`env.get` after `env.set`, `type(env.get("HOME"))`, `os.getenv` of the same name) under both interpreters |
| `print` mixed with `stdio.write`/`hook` under LuaJIT, stdout redirected | the airsl output can appear before `print` lines printed earlier; Lua 5.4 keeps program order; documented, not changed | `probes/230-real-module-set-stock-lua.txt`: in the LuaJIT run "[stdio.write reached stdout]" and the hook JSON come first, before "interpreter:	Lua 5.1 …"; in the 5.4 run they sit in program order |
| memory or instruction ceiling | none; `open` arms nothing | F6 |
| Neovim | its LuaJIT loads the `airsstack-luajit` build and refuses the 5.4 build | `probes/65-neovim-luajit-load.txt`: "ok=true jit=LuaJIT 2.1.1770848219 _VERSION=Lua 5.1"; book note only |
| glibc floor | the Linux rocks need glibc 2.14 (x86_64) or 2.17 (aarch64) | `probes/162-y1-docker-manylinux-x86_64.txt` "max GLIBC: GLIBC_2.14"; `probes/163-y1-docker-manylinux-aarch64.txt` "max GLIBC: GLIBC_2.17" |
| macOS floor | arm64 dylib minos 11.0; x86_64 dylib `LC_VERSION_MIN_MACOSX` 10.12 | `probes/62-macos-module-build-load.txt` "minos 11.0"; `probes/122-macos-x86_64.txt` "version 10.12" |

### 6.4 Rockspec and release edges

| Case | Rule | Checked example |
|---|---|---|
| version bump without renaming | docs-check fails (§5.6.3) | workspace `0.3.0`, file `airsstack-0.2.0-1.rockspec` → "has no `airsstack-0.3.0-1.rockspec` …" and "names version 0.2.0, which is not the workspace version 0.3.0 …" |
| renamed but `version =` not edited | docs-check fails; luarocks would too | `probes/202-final-rockspecs.txt`: "Inconsistency between rockspec filename (airsstack-0.3.0-1.rockspec) and its contents (airsstack-0.2.0-1.rockspec)" |
| re-running a failed publish | uploads replace in place: one rock per (version, arch), a re-upload of an existing version updates it | `probes/40-luarocks-site.txt` (uploaders.moon:138-146 overwrite; rocks.moon:28 unique (version_id, arch)) |
| re-running after a name refusal | `/upload` stores the rockspec again in our module outside the root, answers `manifests` without `root`, and the job fails again with the name message before any rock | §5.6.4 (uploaders.moon:157-160 adds to the root only on creation; api.moon:285-288 recomputes `manifests` on every upload); `probes/234-publish-jq-checks.txt` `taken-rerun.json` → "false" "[exit 1]" |
| a later release after a name refusal | the same as a re-run: the module is not new, so it is never added to the root | same |
| `airsstack` accepted, `airsstack-luajit` refused (phase 1) | the job stops before phase 2; `airsstack`'s version exists with its rockspec only, so `luarocks install airsstack <version>` builds from source until a re-run after the name is settled; the partial-state line lists `airsstack`'s rockspec and `airsstack-luajit`'s | §10.3; `probes/183-luarocks-revision-pick.txt` (a rockspec-only revision is chosen and built from source) |
| one rock refused after others uploaded | the job stops; earlier uploads stay; the partial-state line lists them; a re-run replaces them | §5.6.4; re-upload replaces (row above) |
| API key missing | the job fails before any request (§5.6.4) | `LUAROCKS_API_KEY=""` → first message; an undefined secret arrives empty (`probes/220-publish-downloads-build-artifacts.txt`:302 "LUAROCKS_API_KEY length: 0"); luarocks.org itself would answer an empty key segment with a 404 page (`probes/189-luarocks-org-names-api.txt` "[http 404 text/html]") |
| key invalid | HTTP refusal message with the server body | `probes/189-luarocks-org-names-api.txt`: `{"errors":["Invalid key"]}` "[http 401 application/json]" |
| account requires two-factor for uploads | a key alone cannot upload: 403 `two_factor_required` | `probes/189-luarocks-org-names-api.txt` (api.moon:78-97); hence the prerequisite of §10.4 |
| prerelease tag | dist skips custom publish jobs for prereleases | `probes/190-dist-rocks-global-job.txt`: "`if: ${{ !fromJson(needs.plan.outputs.val).announcement_is_prerelease \|\| fromJson(needs.plan.outputs.val).publish_prereleases }}`"; airsl versions have three numeric parts (`version.rs:43-50`) |

## 7. The native module and the type file

The type file and `.luarc` written by `lua setup` are unchanged: they are rendered from `stdlib()`,
which still includes `ext` (intent constraint 7; `crates/airsl-cli/src/lua.rs:44-46`). The editor
therefore offers `airsstack.ext` in a script that will run under stock Lua, where it is `nil`; the
book says `ext` exists only in airsl.

## 8. Rocks

### 8.1 The rockspecs

Checked in as `crates/airsl-lua/rockspecs/airsstack-<version>-1.rockspec` and
`airsstack-luajit-<version>-1.rockspec`, renamed and edited on every version bump. Text at `0.2.0`:
`probes/202-final-rockspecs/` (linted there, `[exit 0]` for both) plus the `copy_directories` line
of S13, linted again in `probes/235-rockspec-only-install-and-docs.txt` ("both-lint-clean",
`[exit 0]`; files in `probes/235-rockspec-only-install-and-docs/src/crates/airsl-lua/rockspecs/`):

```lua
rockspec_format = "3.0"
package = "airsstack"
version = "0.2.0-1"
source = {
   url = "git+https://github.com/airsstack/airsl",
   tag = "airsl-cli-v0.2.0",
}
description = {
   summary = "The airsl host modules as a native module for Lua 5.4",
   homepage = "https://airsstack.github.io/airsl/",
   license = "Apache-2.0",
}
dependencies = {
   "lua == 5.4",
}
build_dependencies = {
   "luarocks-build-rust-mlua == 0.2.7",
}
build = {
   type = "rust-mlua",
   cargo_extra_args = { "--package", "airsl-lua", "--locked" },
   modules = { airsstack = "airsstack" },
   copy_directories = { "crates/airsl-lua/doc" },
}
```

```lua
rockspec_format = "3.0"
package = "airsstack-luajit"
version = "0.2.0-1"
source = {
   url = "git+https://github.com/airsstack/airsl",
   tag = "airsl-cli-v0.2.0",
}
description = {
   summary = "The airsl host modules as a native module for LuaJIT",
   homepage = "https://airsstack.github.io/airsl/",
   license = "Apache-2.0",
}
dependencies = {
   "lua == 5.1",
   "luajit >= 2.1",
}
build_dependencies = {
   "luarocks-build-rust-mlua == 0.2.7",
}
build = {
   type = "rust-mlua",
   cargo_extra_args = { "--package", "airsl-lua", "--locked" },
   modules = { airsstack = "airsstack" },
   copy_directories = { "crates/airsl-lua/doc" },
}
```

Why each field: `rockspec_format = "3.0"` makes `luajit >= 2.1` resolve as "provided by VM"
(`probes/25-luarocks-config-keys.txt`: "(2.1.1770848219-1 provided by VM: success)"); the
`luajit >= 2.1` dependency refuses PUC 5.1 (§5.4) and Lua 5.4 (`probes/25-luarocks-config-keys.txt`:
"No results matching query were found for Lua 5.4."). `cargo_extra_args` builds the member from the
workspace root with the committed lockfile (`probes/146-rockspecs-make-server.txt`:
"args=build --release --package airsl-lua --locked --target-dir=target --features lua54").
`modules = { airsstack = "airsstack" }` maps the Lua module to the cargo lib name (D9).
`copy_directories = { "crates/airsl-lua/doc" }` keeps `CLAUDE.md` (and the other root docs) out of
the rocks: without the field luarocks tries `doc`, finds no such directory at the root, and then
copies every root file whose lower-cased name starts `readme`, `license` or `copying` or ends `.md`
(`probes/29-luarocks-3.12.2-src/build.lua:314-330` `install_default_docs`, patterns `{ "readme",
"license", "copying", ".*%.md" }` at :315; `:342-347` the `{ "doc" }` default; `:362-364` `if not
any_docs then install_default_docs`), which is how `doc/CLAUDE.md` reached the rocks of probe 146. A named
directory that exists is copied to the same relative path inside the rock and turns the defaults
off (`:350-355`); a named directory that is missing fails the build ("Directory '…' not found",
`:357-358`). `build.install` does not affect docs (`:335-340`). These are probe 146's rockspecs, which
built, loaded and routed, with `source`, `version`, `summary` and the `copy_directories` line
changed (`probes/202-final-rockspecs.txt` header; probe 235).

### 8.2 Packages and routing

luarocks.org lists a version in `manifest-X.Y` by the rockspec's `lua` dependency, so
`airsstack` appears only for Lua 5.4 and `airsstack-luajit` only for 5.1
(`probes/40-luarocks-site.txt`, versions.moon:8-15; `probes/146-rockspecs-make-server.txt`: the
local `manifest-5.4` lists only `airsstack`, `manifest-5.1` only `["airsstack-luajit"]`). Installing
the wrong package fails (`probes/24-local-server.txt`: "Error: No results matching query were found
for Lua 5.1."). A binary rock's file name cannot tell Lua 5.4 from LuaJIT (it is
`<package>-<version>-<rev>.<arch>.rock`, `probes/151-luarocks-pack-arch-name.txt`, pack.lua:109),
which is why there are two packages.

### 8.3 Rock contents

- Binary rock: the rockspec, `lib/airsstack.so` (or `.dll`), `rock_manifest`, and
  `crates/airsl-lua/doc/LICENSE` (S13; `probes/235-rockspec-only-install-and-docs.txt`, `unzip -l` of
  the rock packed after `luarocks make` and of the one packed after the rockspec-only install: those
  four entries plus their directories, "8 files"). The control, the same rockspec without
  `copy_directories`, packs `doc/LICENSE`, `doc/CHANGELOG.md`, `doc/README.md`, `doc/CLAUDE.md` (same
  probe, "9 files"), as probe 146 did.
- A binary rock installs with cargo off `PATH` and fetches no build dependency
  (`probes/23-binary-rocks.txt`: "With cargo hidden (command -v cargo exit=1) both binary rocks install (exit 0)"; "tree-bin54 holds only airsstack (no luarocks-build-rust-mlua, no network line in the output)").
- No source rock (S12). A platform without a binary rock installs from the rockspec alone:
  luarocks clones `source.url` at `source.tag`, builds `--package airsl-lua --locked` with cargo and
  installs (`probes/235-rockspec-only-install-and-docs.txt`, a server holding only the rockspec:
  "Cloning into 'bare'...", "Finished `release` profile", "airsstack 0.2.0-1 is now installed in
  <S>/p235/tree (license: Apache-2.0)", then `require` from that tree prints
  "<S>/p235/tree/lib/lua/5.4/airsstack.so	11	[2,2.0]	nil"). That probe's source was a local bare
  clone (`git+file://`) because the public repository has no `airsl-lua` yet; the released rockspecs
  name `git+https://github.com/airsstack/airsl`, fetched by the same git step.

### 8.4 Platforms

| Arch (luarocks name) | Runner | How | Proof |
|---|---|---|---|
| `linux-x86_64` | `ubuntu-latest` | `docker run quay.io/pypa/manylinux2014_x86_64` running `crates/airsl-lua/ci/manylinux.sh`: builds Lua 5.4.8, LuaJIT (pinned commit, §9.4) and luarocks 3.12.2 (against Lua 5.4) from checksummed sources, installs Rust 1.94, then for each package `luarocks [--lua-version 5.1 --lua-dir /opt/luajit] make`, `pack`, and a load check | `probes/162-y1-docker-manylinux-x86_64.txt`: "Packed: /tmp/pack-54/airsstack-0.0.0-1.linux-x86_64.rock", "(jit-via54) … Packed: …airsstack-luajit-0.0.0-1.linux-x86_64.rock", "max GLIBC: GLIBC_2.14"; script shape: `probes/160-ci-probe-src/ci/y1-manylinux.sh` |
| `linux-aarch64` | `ubuntu-24.04-arm` | the same with `manylinux2014_aarch64` | `probes/163-y1-docker-manylinux-aarch64.txt`: "…linux-aarch64.rock", "max GLIBC: GLIBC_2.17" |
| `macosx-aarch64` | `macos-latest` | `brew install lua@5.4 luajit luarocks`; `--lua-version 5.4 --lua-dir $(brew --prefix lua@5.4)` and `--lua-version 5.1 --lua-dir $(brew --prefix luajit)` | `probes/165-y3-brew-macos-arm64.txt`: "Packed: …airsstack-0.0.0-1.macosx-aarch64.rock", "…airsstack-luajit-0.0.0-1.macosx-aarch64.rock" |
| `macosx-x86_64` | `macos-26-intel` | the same as arm64 | `probes/166-y3-brew-macos-x86_64.txt`: "…macosx-x86_64.rock" for both |
| `win32-x86_64` | `windows-latest` | MSVC dev prompt; Lua 5.4.8 built with `cl` (`lua54.dll`), LuaJIT (pinned commit, §9.4) with `msvcbuild.bat`, both through `ci/msvc-lua.cmd` (`lua51.dll`); luarocks 3.12.2 `windows-64` zip; `--lua-version 5.4 --lua-dir C:\lua54` and `--lua-version 5.1 --lua-dir C:\luajit` | `probes/167-y4-windows-msvc-luarocks.txt`: "Packed: D:\a\_temp\pack54\airsstack-0.0.0-1.win32-x86_64.rock", "…airsstack-luajit-0.0.0-1.win32-x86_64.rock" |

Why LuaJIT is targeted through the Lua 5.4-hosted luarocks: a luarocks hosted on LuaJIT could not
load the backend it had just installed into `--tree` (`probes/162-y1-docker-manylinux-x86_64.txt`:
"Error: Failed initializing build back-end for build type 'rust-mlua'"), while
`--lua-version 5.1 --lua-dir /opt/luajit` on the 5.4 one built, loaded ("json = [2,2]") and packed.
Why not a job-level container: GitHub's node actions need a newer glibc than 2.17
(`probes/161-y1-container-aarch64.txt`: "`GLIBC_2.28' not found (required by /__e/node20/bin/node)").
Why x86_64 macOS on the Intel runner: probe 166 proved `make` and `pack` there; a cross-build from
arm64 was proven for cargo only (`probes/122-macos-x86_64.txt`), and `pack` names the rock after the
running luarocks' `arch` (`probes/151-luarocks-pack-arch-name.txt`, pack.lua:109).

## 9. Testing

### 9.1 Inline unit tests (default features, `#[cfg(test)] mod tests`, names are behaviour sentences)

`crates/airsl/src/modules/registry.rs`:
- `open_returns_one_subtable_per_module_in_set_order`
- `open_sets_no_global`
- `open_leaves_package_loaded_and_the_require_registry_untouched`
- `open_installs_with_trusted_authority` (`fs.read` of a temp file succeeds)
- `open_on_an_empty_set_returns_an_empty_table`
- `open_propagates_a_module_install_error`
- `install_hands_modules_the_policy_it_is_given` (a confined policy refuses `fs.read`)

`crates/airsl/src/modules/stdlib.rs`:
- `standalone_is_stdlib_without_ext_in_the_same_order`
- `stdlib_is_standalone_with_ext_last`

`crates/airsl/src/builder.rs`: the existing engine tests stay as they are and now exercise
`ModuleSet::install`. `crates/airsl/src/version.rs`: `the_version_has_three_numeric_components`
stays (M11).

`crates/airsl-cli/src/cli.rs` (via `Cli::try_parse_from`, like `lua_setup_takes_a_directory` at
`cli.rs:406-423`):
- `lua_setup_takes_native`
- `lua_setup_takes_native_with_lua_5_4`
- `lua_setup_takes_native_with_luajit`
- `lua_setup_without_native_has_no_lua`
- `lua_setup_refuses_lua_without_native`
- `lua_setup_refuses_an_unknown_lua`
- `lua_setup_refuses_lua_twice`
- `lua_setup_refuses_lua_without_a_value` (`--lua` last, and `--lua=`; E3b)

`crates/airsl-cli/src/lua/native.rs` (through a scripted fake `Luarocks`):
- `lua_5_4_chooses_airsstack`
- `lua_5_1_chooses_airsstack_luajit`
- `a_padded_or_crlf_version_is_trimmed`
- `an_empty_answer_is_refused_as_no_version`
- `a_three_part_version_is_refused_as_not_a_version`
- `two_lines_are_refused_and_shown_escaped`
- `non_ascii_whitespace_and_a_bom_are_not_trimmed`
- `invalid_utf8_is_shown_lossily`
- `lua_5_5_is_refused_naming_the_flag_and_the_book_page`
- `lua_5_04_is_refused_as_unsupported_not_read_as_5_4`
- `the_lua_flag_passes_lua_version_to_both_calls`
- `without_the_lua_flag_no_lua_version_is_passed`
- `the_lua_flag_refuses_a_disagreeing_answer`
- `the_install_arguments_are_local_package_and_the_airsl_version`
- `a_missing_luarocks_prints_both_install_lines`
- `a_missing_luarocks_with_the_lua_flag_prints_one_install_line`
- `a_missing_luarocks_on_windows_names_the_exe_and_the_bat_wrapper`
- `a_config_failure_names_the_command_and_its_status`
- `a_nonzero_config_exit_wins_over_a_valid_version_on_stdout`
- `a_config_signal_is_reported`
- `a_config_signal_wins_over_a_valid_version_on_stdout`
- `a_spawn_failure_names_luarocks_and_the_io_error` (E5/E6 text, both `Flavour`s)
- `an_install_failure_exits_one_whatever_luarocks_returned`
- `an_install_signal_is_reported`
- `the_hint_names_luarocks_path_for_the_chosen_lua`
- `the_windows_hint_names_the_cmd_and_powershell_forms`
- `the_rock_version_is_the_cli_version` (`airsl::VERSION == env!("CARGO_PKG_VERSION")`)

`crates/airsl-cli/src/lua/luarocks.rs`:
- `a_missing_program_is_reported_as_missing` (program pointed at a path that does not exist)
- `a_non_executable_file_is_a_spawn_failure` (unix)
- `a_killed_child_has_no_exit_code` (unix)
- `query_captures_stdout_and_the_exit_code` (every OS: `System` pointed at the test binary itself, `std::env::current_exe()`, with `--list`, which runs no test; stdout is captured non-empty and the code is `Some(0)`)

`crates/airsl-cli/src/lua/command.rs` (today's three tests from `lua.rs:84-136` move here, plus):
- `native_detection_failure_writes_nothing`
- `native_type_file_failure_installs_nothing`
- `native_prints_the_report_before_installing`
- `native_install_failure_keeps_the_report_and_exits_one`
- `luarocks_missing_at_install_keeps_the_report_and_prints_the_install_lines` (E4 at S4: the fake answers `config`, then `Missing` for `install`)
- `native_success_prints_the_report_then_the_hint`
- `a_failed_report_write_skips_the_install`
- `a_failed_hint_write_exits_one`
- `without_native_luarocks_is_never_run`

`crates/docs-check/src/rockspecs.rs`:
- `rockspecs_match_the_workspace_version` (the guard over the real tree)
- `a_missing_rockspec_is_reported`
- `a_package_line_that_disagrees_with_the_file_name_is_reported`
- `a_version_line_that_disagrees_with_the_file_name_is_reported`
- `a_tag_that_is_not_the_release_tag_is_reported`
- `a_missing_line_is_reported`
- `a_leftover_rockspec_of_another_version_is_reported`
- `a_revision_other_than_one_is_reported`
- `an_unknown_package_is_reported`
- `a_file_that_is_not_a_rockspec_is_reported`
- `a_member_that_does_not_inherit_the_workspace_version_is_reported`
- `a_leftover_rockspec_gets_one_message_and_no_line_checks`
- `a_workspace_version_that_is_missing_or_not_a_string_is_reported`
- `an_unreadable_rockspec_is_reported`

`crates/docs-check/src/examples.rs`: the test that pins the inner command (`:473`) expects no
`--all-features`.

### 9.2 The interpreter check

`crates/airsl-lua/lua/check.lua` takes the directory of a built library as `arg[1]` and the ABI it
was built for, `lua54` or `luajit`, as `arg[2]` (stock interpreters pass both so:
`probes/206-lua-arg-and-error-exit.txt` "sub/check.lua	target/native/x	nil"; `probes/230-real-module-set-stock-lua.txt`
ran `run.lua native/lua54 "$T"` with two arguments). It first checks the interpreter against `arg[2]`
(§5.6.2), so a 5.4 build is never loaded into Lua 5.5 or another ABI and reported as passing; then
prepends `<dir>/?.so` and `<dir>/?.dll` to `package.cpath` (the shape probes 06 and 100 loaded with),
then checks, in order, the rows of §5.6.2: `require` succeeds and returns a table; no global;
`package.loaded.airsstack` is that table; `ext` is nil; the eleven modules are tables; `fs.read` of
`arg[0]` succeeds (trusted authority; `arg[0]` is the script path as typed, probe 206); JSON of
`{2, 2.0}` is `[2,2.0]` under Lua 5.4 and `[2,2]` under LuaJIT, branching on `rawget(_G, "jit")`
(F7, F8: `jit` is a table only under LuaJIT); under LuaJIT `utf8` is nil. It prints the success line.

### 9.3 The gate (`Makefile.toml`)

`dod` keeps its five steps in their order: fmt-check, clippy, doc, test, test-doc. What each runs:

| Task | Runs | Proof the form works |
|---|---|---|
| fmt-check | `cargo fmt --all -- --check` (unchanged) | — |
| clippy | `cargo clippy --workspace --exclude airsl-lua --all-targets -- -D warnings`, then `cargo clippy -p airsl-lua --features lua54 --all-targets -- -D warnings`, then the same with `--features luajit` | `probes/05-feature-split-workspace.txt` (c) `exit=0`; `probes/143-luajit-stdlib-cfg.txt` form A, both ABIs `[exit 0]`; lints `airsl` too (M6) |
| doc | with `RUSTDOCFLAGS="-D warnings"`: `cargo doc --workspace --exclude airsl-lua --no-deps`, then `cargo doc -p airsl-lua --no-deps` with `--features lua54`, then `--features luajit` | `probes/05-feature-split-workspace.txt` (i): "Generated …/target/doc/native/index.html" |
| test | no command of its own: `dependencies = ["test-cargo", "native-load"]`, the mechanism `dod` uses for its five steps (`Makefile.toml:43-46`); `test-cargo` (new) runs `cargo test --workspace --exclude airsl-lua --all-targets` | §2.3; F15 does not apply (`--workspace` is explicit) |
| test-doc | `cargo test --workspace --exclude airsl-lua --doc` | — |
| clippy-fix | the clippy commands with `--fix` | — |
| dod-crate | any crate but airsl-lua: today's five commands without `--all-features`; `airsl-lua`: clippy and doc per ABI, no `cargo test` or doctests (D3), then native-load | — |
| native-load (new, part of test) | `script_runner = "@duckscript"`, as the repo's other multi-step tasks (`Makefile.toml:141`, `:333`). For `lua54`, then `luajit`: the interpreter is `AIRSL_LUA54` (default `lua5.4`) or `AIRSL_LUAJIT` (default `luajit`), looked up with `which`; empty → §5.6.1, task fails. `exec --fail-on-error cargo build -p airsl-lua --features <abi> --target-dir target/native-build`; `cp` the library (`target/native-build/debug/libairsstack.dylib` on macOS, `libairsstack.so` on Linux, `airsstack.dll` on Windows) to `target/native/<abi>/airsstack.so` (`airsstack.dll` on Windows); `exec --fail-on-error <interpreter> crates/airsl-lua/lua/check.lua target/native/<abi> <abi>`. The explicit `--target-dir` fixes the library's path whatever `CARGO_TARGET_DIR` or `build.target-dir` say, at the cost of a second target tree for the module builds | `probes/233-native-load-task-form.txt` (the task's form on macOS: `which` for a bare name and a path, `exec --fail-on-error`, `cp`; the `CARGO_TARGET_DIR=elsewhere` run left the library at "target/native-build/debug/libairsstack.dylib" and "ls: elsewhere: No such file or directory"); F12; `probes/140-lua-module-private-mod.txt` `nm -gU …/libairsstack.dylib`; `probes/20-rust-mlua-backend-source.txt` rust-mlua.lua:86-94 (`lib<name>.so`, `<name>.dll`). Not run on Linux or Windows (risk R12) |
| rocks (new, outside `dod` like deny and examples; needs luarocks and network) | `@duckscript`. For each package: the rockspec is the one file matching `crates/airsl-lua/rockspecs/<package>-*-1.rockspec` (docs-check keeps exactly one per package at the workspace version, §5.6.3) and `<version>` is read from its name; `luarocks --lua-version 5.4` (or `5.1`), plus `--lua-dir` from `AIRSL_LUA54_DIR` / `AIRSL_LUAJIT_DIR` when set, `make --tree <root>/target/rocks/<abi> <rockspec>`; then, with cwd `target/rocks/dist` because `pack` writes into the working directory, the same `--lua-version`/`--lua-dir` with `--tree <root>/target/rocks/<abi> pack <package> <version>-1`; then `check.lua <root>/target/rocks/<abi>/lib/lua/5.4 lua54` (or `…/5.1 luajit`). `<root>` is the workspace root, absolute | `probes/146-rockspecs-make-server.txt` (make from the workspace root, both `[exit 0]`; `pack` with `--tree` run in another directory, :147 and :151, "Packed: …"); `probes/165-y3-brew-macos-arm64.txt` (`--lua-dir`) |
| examples, deny, book, publish-dry-run, install, fmt | unchanged | — |

The Makefile comment that
`--all-features` is "carried for forward-safety" (`Makefile.toml:32-34`) is replaced by D1's reason.

### 9.4 CI (`ci.yml` and `build-rocks.yml`)

`ci.yml`'s `dod` job (ubuntu, macos, windows; `.github/workflows/ci.yml:35-38`) installs two interpreters before `cargo make dod`
and sets `AIRSL_LUA54`/`AIRSL_LUAJIT` where the names differ; the `deny`, `book` and `installer`
jobs are unchanged; `cargo make examples` stays. None of the runner images ships Lua
(`probes/47-ci-lua-tooling.txt`: `grep -n -i lua` over the Ubuntu 24.04, Ubuntu 26.04, macOS 26 arm64
and Windows 2025 image readmes, each "exit=1"); Lua is pinned to 5.4 because Homebrew's `lua` and
gh-actions-lua's default are 5.5 (same probe: "lua 5.5.1 deps= [] keg_only= False", `default: "5.5"`).

| OS | Lua 5.4 | LuaJIT | Proof |
|---|---|---|---|
| ubuntu-latest (26.04) | apt `lua5.4` (5.4.8, `/usr/bin/lua5.4`) | apt `luajit` (`LuaJIT 2.1.1761786044`, `/usr/bin/luajit`) | `probes/164-y2-ubuntu-apt.txt`: "lua5.4	5.4.8-1build1", "luajit	2.1.0+openresty20251030-1", 2×2 load matrix |
| macos-latest | brew `lua@5.4` (keg-only: `AIRSL_LUA54=$(brew --prefix lua@5.4)/bin/lua5.4`) | brew `luajit` | `probes/165-y3-brew-macos-arm64.txt`: "brew --prefix lua@5.4 = /opt/homebrew/opt/lua@5.4", `lua5.4 -> lua`, "Lua 5.4.9" |
| windows-latest | Lua 5.4.8 built with `cl` in the MSVC dev prompt (`lua54.dll`, `lua.exe`) | LuaJIT built with `msvcbuild.bat` (`lua51.dll`) | `probes/167-y4-windows-msvc-luarocks.txt`: "cl exit=0", "link dll exit=0", "msvcbuild exit=0"; `probes/100-msvc-module-load.txt`: an MSVC-built module loads in both ("result:	table	true	[2,2.0]", "[2,2]") |

The steps, the same in `ci.yml` and in the matching legs of `build-rocks.yml`:

- **Ubuntu**: `sudo apt-get install -y lua5.4 luajit` (`probes/164-y2-ubuntu-apt.txt`:138 "apt-get
  install exit=0"; the probe's step also installed headers and luarocks, which the `dod` job does not
  need). The default names `lua5.4`/`luajit` match, so no variable is set.
- **macOS**: `brew install lua@5.4 luajit`, then `AIRSL_LUA54=$(brew --prefix lua@5.4)/bin/lua5.4`
  into `$GITHUB_ENV` (`probes/165-y3-brew-macos-arm64.txt`:160 "brew install exit=0" and the prefix
  line above).
- **Windows**: `ilammy/msvc-dev-cmd` pinned to the commit the probe resolved
  (`probes/167-y4-windows-msvc-luarocks.txt`:40 "ilammy/msvc-dev-cmd@v1
  (SHA:0b201ec74fa43914dc39ae48a89fd1d8cb592756)") enters the MSVC environment; then
  `crates/airsl-lua/ci/msvc-lua.cmd` runs the probe's build steps (`probes/160-ci-probe-src/.github/workflows/probe-ci.yml`
  job Y4, "build lua 5.4.8 (MSVC, lua54.dll)" and "build luajit … (MSVC, msvcbuild.bat)"), and the
  job sets `AIRSL_LUA54`/`AIRSL_LUAJIT` to the absolute paths of the two built interpreters (R12).
- **Pinning**: every download is checked against the SHA-256 the probes recorded, and the step fails
  on a mismatch: `lua-5.4.8.tar.gz` `4f18ddae154e793e46eeab727c59ef1c0c0c2b744e7b94219710d76f530629ae`
  (`probes/162-y1-docker-manylinux-x86_64.txt`:150; the same under `certutil`,
  `probes/167-y4-windows-msvc-luarocks.txt`:310), `luarocks-3.12.2.tar.gz`
  `b0e0c85205841ddd7be485f53d6125766d18a81d226588d2366931e9a1484492` (probe 162:173), the luarocks
  3.12.2 `windows-64` zip `d3f4ddda6926618cadf560170a7c18a5ceead5997ba10832cd0e3b624c7de886`
  (probe 167:450). LuaJIT is checked out at commit `96f7b297326e9fd454b7186c6858b2773f94a0dc`, the
  `v2.1` tip every probe built (probe 162:153, 163:153, 167:381 "luajit commit 96f7b297…"), never the
  moving branch; `ci/manylinux.sh` pins the same. apt and Homebrew packages are not pinned: their
  versions follow the runner image, and `check.lua` refuses a wrong interpreter (§5.6.2).
- **Caching**: none. Downloading and building both interpreters with MSVC took 24 seconds in probe
  167 (`probes/167-y4-windows-msvc-luarocks.txt`:239 the Lua download step starts at 07:14:50.49,
  :382 "msvcbuild exit=0" at 07:15:14.43), so a cache is not worth its upkeep.

gh-actions-lua is not used: installed twice in one job it keeps the first Lua and silently skips
the second (`probes/171-y5-gh-actions-lua-ubuntu-cold.txt`, after `luaVersion: luajit`: still
"Lua 5.4.9", "reports	Lua 5.4	no jit"; the same on macOS and Windows,
`probes/172-y5-gh-actions-lua-macos-cold.txt`, `probes/173-y5-gh-actions-lua-windows-cold.txt`;
cached runs 168–170 alike).

`build-rocks.yml` runs on every pull request and every release (`probes/190-dist-rocks-global-job.txt`).
Each of the five legs of §8.4 builds both packages, packs both binary rocks, loads each from its
tree with `check.lua`, and uploads them as an artifact named `rocks-<arch>`. The name does not
match dist's `artifacts-*`, so the rocks are not attached to the GitHub release
(`probes/190-dist-rocks-global-job.txt`: "host's "Download GitHub Artifacts" step takes
`pattern: artifacts-*` …; another name is not"; seen in a real run,
`probes/220-publish-downloads-build-artifacts.txt`:4). The Windows leg additionally installs its
`airsstack` rock with `--local`, runs the hint's cmd form (`luarocks path > lrpath.bat`, `call
lrpath.bat`), and requires the module, so the Windows hint stays proven on every PR (the form of
`probes/221-windows-local-install-path.txt`:922-925, "true" :980).

### 9.5 `deny.toml`

As run in `probes/144-cargo-deny-features.txt` (d), byte for byte from `probes/144-cargo-deny-features/deny.d.diff`:

```diff
@@ -64,12 +64,15 @@
 multiple-versions = "deny"
 # No `version = "*"` dependencies. The root manifest pins every direct one.
 wildcards = "deny"
+allow-wildcard-paths = true
 highlight = "all"
 workspace-default-features = "allow"
 external-default-features = "allow"
 allow = []
 deny = []
-skip = []
+skip = [
+    { crate = "syn@2", reason = "mlua_derive 0.12 (mlua `module` feature) is still on syn 2" },
+]
 skip-tree = []
 
 # ---------------------------------------------------------------------------
```

Result: "advisories ok, bans ok, licenses ok, sources ok", exit 0. `airsl-lua` stays in the graph,
so its licenses and advisories are checked; it ships as rocks.

## 10. Release and publish

### 10.1 dist

As generated and checked in `probes/190-dist-rocks-global-job.txt`
(`probes/190-dist-rocks-global-job/dist-workspace.toml.diff`, byte for byte; the two `# probe M:`
comments become comments in the file's own style):

```diff
@@ -28,11 +28,15 @@ github-attestations = true
 # dist builds and publishes but never runs what it built. This hook runs after
 # every archive is built and `host` waits for it, so an archive that fails its
 # smoke test is never published. See .github/workflows/airsl-cli-smoke.yml.
-global-artifacts-jobs = ["./airsl-cli-smoke"]
+global-artifacts-jobs = ["./airsl-cli-smoke", "./build-rocks"]
 # Pull requests build and smoke-test every archive too, publishing nothing. The
 # gate in ci.yml never builds musl or this profile, so without this a change that
 # breaks a release target would first fail on the release tag itself.
 pr-run-mode = "upload"
+# probe M: build only the packages dist ships
+precise-builds = true
+# probe M: upload the rocks after the GitHub release exists
+publish-jobs = ["./publish-rocks"]
 
 # dist's defaults name fixed images (macos-14, ubuntu-22.04, windows-2022). GitHub retires
 # those, and the macos-14 retirement broke every release. The `-latest` labels move with GitHub
```

`dist generate --check` fails until the release workflow is regenerated, then passes, and
`dist plan` exits 0 (`probes/190-dist-rocks-global-job.txt`: "help: run 'dist init' to update the
file", then exit 0). `precise-builds = true` is required: without it dist builds `--workspace` and
the module member breaks every release build (`probes/44-dist-toy-member.txt` D/E: "`vendored`,
`external` and `module` features are mutually exclusive", "exit=255"; F: "using cargo profile dist
--package=airsl-cli", "exit=0"). dist ignores the cdylib (`probes/44-dist-toy-member.txt`: "B. toy member, publish=false, no dist metadata (output identical to A apart from the header)"), and
`[package.metadata.dist] dist = false` says so explicitly.

Job order the regenerated workflow has (`probes/190-dist-rocks-global-job.txt`):
`custom-build-rocks` needs `plan` and `build-local-artifacts`; `host` needs it and runs only if it
succeeded or was skipped, so a failed rock leg stops the GitHub release; `custom-publish-rocks`
needs `plan` and `host`, so it runs after `gh release create`; `announce` waits for it.

### 10.2 `build-rocks.yml`

§8.4 and §9.4. It uploads nothing outside the run.

### 10.3 `publish-rocks.yml`

Runs only for a release (needs `host`; skipped for prereleases, §6.4), on `ubuntu-latest`, the
runner of `probes/220-publish-downloads-build-artifacts.txt` (ubuntu-26.04), whose image ships `curl`
and `jq` (`probes/47-ci-lua-tooling/runner-images/Ubuntu2604-Readme.md`:78 "jq 1.8.1", :258 "curl").
In order:

1. Fail if `LUAROCKS_API_KEY` is the empty string (§5.6.4), before any download or request.
2. Download the `rocks-*` artifacts of the run with `actions/download-artifact` (`pattern: rocks-*`,
   `merge-multiple: true`): ten binary rocks. A publish job downloads a global job's artifacts in
   the same run (`probes/220-publish-downloads-build-artifacts.txt`:3 "(c)/(d) rocks-linux-x86_64 +
   rocks-windows-x86_64 …", :428 "outcomes: a=success b=success c=success d=success"). The name stays
   `rocks-<arch>`, not `artifacts-*`: `host` uploads every `artifacts-*` artifact to the GitHub release
   (same probe :4 "host's pattern artifacts-* also picked up artifacts-build-rocks, so its files would
   go to 'gh release create ... artifacts/*' -- rocks-* did not"), and attaching rocks is a non-goal.
3. Check out the release tag for its two rockspecs (`crates/airsl-lua/rockspecs/`). `actions/checkout`
   works under the publish job's token, which has no `contents` scope (same probe :277-278 "Metadata:
   read" / "Packages: write", then :553 "checkout outcome=success"; the repository is public). No
   source rock is packed or uploaded (S12).
4. Require all twelve files: per package the rockspec and the five
   `<package>-<version>-1.<arch>.rock` of §8.4.
5. Phase 1, the rockspecs (S15): for `airsstack`, then `airsstack-luajit`, `POST
   /api/1/<key>/upload` with `rockspec_file`; read `version.id`; check the root manifest (§5.6.4).
   If either check fails, the job stops here and no binary rock is uploaded for either package.
6. Phase 2, at once after phase 1: for each package, `POST /api/1/<key>/upload_rock/<version.id>`
   with `rock_file` for each of its five binary rocks (ten requests in all).
   Every request is `curl -sS -w '%{http_code}' -o <body file>`, the form of
   `probes/189-luarocks-org-names-api.txt` (`-w '%{http_code}'` against luarocks.org): curl's exit
   status is the transport, the printed code the HTTP status. The endpoints and JSON:
   `probes/189-luarocks-org-names-api.txt` (api.moon:264-321: `/upload` → `{ module, version,
   module_url, manifests, is_new }`, `version.id` an integer; `/upload_rock/:version_id` → `{ rock,
   module_url }`); the arch is taken from the file name and only its characters are checked
   (`probes/40-luarocks-site.txt`: "helpers/uploaders.moon (arch from file name; only charset-validated,
   no allow-list)", uploaders.moon:64 `fname\match "^#{escape_pattern(module_name)}%-(.-)%.([^.]+)%.rock$"`,
   :50 `str\match("^[a-zA-Z0-9%.%_%-]+$")`).

Re-running the whole job after a failure is safe for every failure but the name check: an existing
version is updated in place and each (version, arch) rock replaced (§6.4). A name refusal fails again
on every re-run, by design (§5.6.4). Both name checks run before any binary rock, so a refused name
never leaves the other package with binaries at that version. What phase 1 cannot avoid: each check
needs its own `/upload`, so when `airsstack-luajit` is refused, `airsstack`'s rockspec has already
created its version, with no binary rock; until the name is settled and the job re-run, `luarocks
install airsstack <version>` finds only that rockspec and builds from source
(`probes/183-luarocks-revision-pick.txt`: the rockspec-only `-2` was chosen over the `-1` binary and
built from source; S12's rockspec install, `probes/235-rockspec-only-install-and-docs.txt`). On a
normal run that window is the time from the first `/upload` to the last binary upload: one more
`/upload` and the ten binary requests, which follow at once (risk R3). The partial-state line names
what was uploaded (§5.6.4).

### 10.4 Prerequisites before the roadmap's release (not needed until then)

- A luarocks.org account without upload two-factor authentication, because a key alone cannot
  upload for an account that requires it (`probes/189-luarocks-org-names-api.txt`, api.moon:78-97: "403 { errors: {\"Two-factor authentication
  required\"}," … "two_factor_required: true }").
- Its API key in the repository secret `LUAROCKS_API_KEY`.
- The names `airsstack` and `airsstack-luajit` were free on 2026-11-26
  (`probes/189-luarocks-org-names-api.txt`: "airsstack:	false", "airsstack-luajit:	false"); the
  first upload claims them (risk R1).

## 11. Documentation

The book is written per `CLAUDE.md` § Documentation: plain sentences for a user, every command and
output run against the checkout before it lands, one fact in one chapter.

| File | Change |
|---|---|
| `book/src/ch01-05-stock-lua.md` (new, "Running Scripts in Stock Lua"; listed in `book/src/SUMMARY.md` after Editor Support) | what the native module is; `airsl lua setup --native [--lua 5.4\|luajit]` and plain `luarocks install --local airsstack` / `airsstack-luajit`; `require` returns the table and sets no global; trusted authority and no memory or instruction ceiling; `ext` exists only in airsl; LuaJIT differences (JSON `2` against `2.0`, `2^53`, no `utf8`); the `luarocks path` hint per OS; a section "Pointing LuaRocks at Lua 5.4 or LuaJIT" for Homebrew (`lua@5.4` keg-only, luarocks defaulting to 5.5, `--lua-dir`), Ubuntu apt (luarocks 3.8 defaulting to PUC 5.1, a LuaJIT-only directory) and Windows (the luarocks zip, `luarocks.exe`, `--lua-dir`); Neovim: needs `airsstack-luajit` and a `package.cpath` line shown from a run; musl and platforms without a binary rock install from the rockspec, which needs git and cargo, Rust 1.94 or newer (`Cargo.toml:8` `rust-version  = "1.94"`; the source carries `rust-toolchain.toml:9` `channel    = "1.94"`); two behaviours seen in probe 230 under the real module: `env.set` changes what `env.get` and `proc.run`'s children see (`crates/airsl/src/modules/proc.rs:149-151`, the child gets the overlay) but not Lua's own `os.getenv` (`probes/230-real-module-set-stock-lua.txt`: "env      ok   set-by-env \| string \| nil", both interpreters), and under LuaJIT with stdout redirected, `stdio.write`/`hook` output can land before `print` lines printed earlier (Lua 5.4 keeps program order; same probe, the LuaJIT run). E11's URL points here. Transcripts (S17, CLAUDE.md § Documentation "Every Lua snippet, CLI transcript and Rust example is run against the current checkout before it lands, and output is pasted"): with output blocks, only what runs before the release, against the checkout: `luarocks make` of the rockspec from a clone of the repository, `require("airsstack")` and module calls in `lua5.4` and `luajit` (the forms of `probes/146-rockspecs-make-server.txt` and `probes/230-real-module-set-stock-lua.txt`). The `luarocks install --local airsstack` / `airsstack-luajit` and `airsl lua setup --native` sections show the commands with no output block until the release (before it they can only print luarocks' "No results matching query", §5.4). |
| `book/src/ch01-04-editor-support.md` | links to the new chapter; notes that `ext` is typed but airsl-only |
| `book/src/appendix-01-cli.md` | synopsis `airsl lua setup  [--native [--lua 5.4\|luajit]] [<DIR>]` (`:8`); the command row (`:20`); the exit-code row (`:71`): `1` also covers luarocks failures, and an install failure leaves the type file written |
| `book/src/ch01-01-installation.md`, `book/src/appendix-04-installation.md` | a pointer to the native module chapter |
| `book/src/ch09-01-three-layers.md` (`:22-25`) | airsl itself stays Lua 5.4; the native module also builds for LuaJIT, with the JSON difference |
| `book/src/ch09-03-what-it-promises.md` (`:10`) | the limits hold inside airsl; stock Lua has none |
| `crates/airsl/examples/values-and-json/main.rs` (`:63-64`), `README.md` (`:49-51`) | the `1.7e9` claim is replaced: LuaJIT prints `1700000000` like Lua 5.4 (`probes/149-luajit-json-number.txt`: "\"observed_at\":1700000000" under both); the real difference is a float such as `2^53`, encoded `9007199254740992` on LuaJIT and `9007199254740992.0` on 5.4. The README's `## Output` block does not change. |
| `CHANGELOG.md` (`:7-9`), `crates/airsl/README.md` (`:101-102`), `crates/airsl-cli/README.md` (`:94-97`) | "versioned independently" becomes one shared version, and every release is a lockstep release of both crates tagged `airsl-v<version>` and `airsl-cli-v<version>` on one commit, never a library-only release (S16); the airsl-cli README's command table gains `--native`; the release's CHANGELOG entry has the `### Breaking` item of §2.5 |
| `README.md` (root) | the release step of S17 (no release checklist exists in the repository: `git ls-files \| xargs grep -li checklist` finds only an unrelated plan, `.claudestacks/sdlc/2026-08-29-windows-platform-support/plans/06-env-name-identity.md:140` "newtype checklist"; the maintainer list under `README.md` § Releases, `:81-93`, is the repository's release procedure): a new item, owned by the maintainer who pushes the `airsl-cli-v*` tag, says that after the first release whose `publish-rocks` job succeeds they run, against luarocks.org, `luarocks --lua-version 5.4 install --local airsstack <version>`, `luarocks --lua-version 5.1 install --local airsstack-luajit <version>` and `airsl lua setup --native` (macOS, Linux and, for the hint, Windows), paste the output into `book/src/ch01-05-stock-lua.md`'s output-less sections, and merge that as a `docs:` pull request |
| `README.md` (root), continued | `:54-57` "What is in this repository": the crate table gains `airsl-lua` (the native module, shipped as LuaRocks rocks, not on crates.io); `:59-61`: building from source still needs a C compiler, and the native module also builds for LuaJIT; `:66` and `:72-74` Development: `cargo make dod` also needs `lua5.4` and `luajit` on `PATH` (or `AIRSL_LUA54`/`AIRSL_LUAJIT`), and a line for `cargo make rocks`; `:78-79` "Each crate is tagged on its own, … because the two are versioned independently" becomes one shared version with both tags pushed on the same commit and no library-only release (S16, §2.5); `:83-85` "An `airsl-cli-v*` tag starts the binary release …": it also builds the rocks before the GitHub Release and uploads them to luarocks.org after it (§10), and the maintainer list names `build-rocks.yml`, `publish-rocks.yml` and the `LUAROCKS_API_KEY` secret |
| `Cargo.toml` comments (`:3`, `:15-20`) | members comment names airsl-lua as shipped through LuaRocks; the mlua comment says Lua, linking and `send` are now chosen by airsl's features |
| `crates/airsl/src/lib.rs` | "Cargo features" section (§3.1) |
| `CLAUDE.md` | "What this is" (`CLAUDE.md:7-10`, "Two published crates in one workspace … A third member"): a fourth member, `crates/airsl-lua`, unpublished on crates.io and shipped as rocks; build requirement (`:12-16`): `dod` also needs `lua5.4` and `luajit` (or `AIRSL_LUA54`/`AIRSL_LUAJIT`); Commands: native-load inside `test`, the `rocks` task, and that a `--workspace` command (an editor's check included) must `--exclude airsl-lua` while bare cargo commands work through `default-members` (F15); docs-check's list becomes five (`CLAUDE.md:37` "these four things") with the rockspec/version bullet; Architecture: the VM line's features (`CLAUDE.md:91` "`lua54,vendored,serialize,send`" becomes airsl's default feature set), `modules::stdlib()` bullet (`:122-123`, "A new module is registered there": in `standalone()`, unless it exists only inside airsl like `ext`), `Engine` bullet (`:134`, "`Engine` is `Send + Sync`": with the default `send` feature), `airsl-lua`, `ModuleSet::open`, the `lua/` folder of airsl-cli. The docs-check lone-name guard reads code spans of `CLAUDE.md`: `luaopen_airsstack` and `cargo_extra_args` appear in no `.rs` or `Cargo.toml` at HEAD (`grep -rn 'luaopen_airsstack\|cargo_extra_args' crates Cargo.toml` exits 1, while the same search for `luaopen_` finds `crates/airsl/src/require/disposition.rs`), and `cargo_extra_args` will live only in the rockspecs; `CLAUDE.md` writes such a name in prose, not in a code span, unless the implementation puts it in a `.rs` file (for example `entry.rs`'s `//!`) |
| `book/src/appendix-03-presets.md`, `book/src/ch03-05-presets.md` | unchanged: they describe airsl, which stays Lua 5.4 with `utf8` |

## 12. Risks

- **R1** — The names could be taken before the release; they were free on 2026-11-26 (§10.4). The
  publish job then fails with the name message instead of shipping under a name we do not own, on
  the first run and every re-run, before any binary rock (§5.6.4, S15). The first such run leaves one
  rockspec in a module of our account outside the root manifest. If `airsstack-luajit` is the one
  refused, `airsstack`'s rockspec has already created that version without binaries, and installs of
  it build from source until the name is settled and the job re-run (§10.3).
- **R2** — closed: `publish-rocks` downloads `build-rocks`' artifacts in the same run
  (`probes/220-publish-downloads-build-artifacts.txt`, §10.3 step 2).
- **R3** — Between a package's `/upload` and its binary uploads, `luarocks install` of that version
  picks the rockspec and builds from source (`probes/183-luarocks-revision-pick.txt`). With the two
  phases (S15) the window runs from the first `/upload` to the last binary upload: one more
  `/upload` and ten binary requests, which follow at once.
- **R4** — closed: `--local` plus `luarocks path` on Windows was run end to end and the hint names
  the two forms that worked (`probes/221-windows-local-install-path.txt`, §4). The Windows
  `build-rocks` leg keeps running the cmd form on every pull request (§9.4).
- **R5** — Source builds on apt's luarocks 3.8 need the backend installed into its own Lua 5.1 tree
  first (§6.2); binary rocks are unaffected (§8.3). The book's apt section says so.
- **R6** — Homebrew users without `--lua` meet E11 first, because Homebrew's luarocks targets 5.5
  (§6.2).
- **R7** — Non-additive features (D2): a dependent that disables `airsl`'s defaults now fails to
  build until it picks a Lua feature.
- **R8** — `dod` needs two interpreters on every developer machine (D5); a missing one fails the
  gate with the §5.6.1 message.
- **R9** — closed by S13: the binary rocks carry `crates/airsl-lua/doc/LICENSE` only, no `CLAUDE.md`
  (§8.3, `probes/235-rockspec-only-install-and-docs.txt`). The rocks no longer carry the README or
  CHANGELOG either: `copy_directories` turns all of luarocks' default docs off (§8.1).
- **R10** — No source rock (S12): a platform without a binary rock installs from the rockspec, which
  clones the public repository at the release tag, so it needs git and cargo on `PATH` and network
  access to GitHub; luarocks' own messages are relayed (§5.4).
- **R11** — A musl Linux luarocks would take the glibc rock (§6.2); musl is not supported.
- **R12** (accepted, S18) — The native-load task's duckscript form (§9.3) was run on macOS only
  (`probes/233-native-load-task-form.txt`); its first Linux and Windows runs are the `dod` job of
  the pull request that adds it. On Windows `ci.yml` sets `AIRSL_LUA54`/`AIRSL_LUAJIT` to the absolute
  paths of the MSVC-built interpreters, the path form probe 233 proved for `which`.
- **R13** (accepted, S18) — `copy_directories` with a nested, `/`-separated path was run on macOS only (probe 235);
  its first Linux and Windows runs are the `build-rocks.yml` legs of the pull request that adds it.

## 13. Claim ledger: every probe this spec relies on

| Probe | What the spec takes from it |
|---|---|
| `probes/01-mlua-module-feature-conflicts.txt` | F1 |
| `probes/02-airsl-lua54-module-mode.txt` | F2 |
| `probes/03-airsl-luajit-compile.txt`, `probes/04-airsl-luajit-hidden-errors.txt` | F3 |
| `probes/05-feature-split-workspace.txt` | workspace unification, `--exclude`, no-Lua error, doctests on cdylib, `use airsl::mlua` |
| `probes/06-macos-native-load.txt` | F5, F10, F13, macOS load texts |
| `probes/07-module-memory-limit.txt` | F6 |
| `probes/08-module-json-encode.txt` | F7 |
| `probes/09-process-command-errors.txt` | spawn error kinds and texts, signal → no code |
| `probes/10-module-unit-test-link.txt` | F4 |
| `probes/20-rust-mlua-backend-source.txt` | backend features, copy names (D9), no-cargo text |
| `probes/23-binary-rocks.txt` | binary rocks need no cargo or build dependency |
| `probes/24-local-server.txt` | wrong package refused; no-cargo text |
| `probes/25-luarocks-config-keys.txt` | `config lua_version` output; `luajit >= 2.1` with format 3.0 |
| `probes/26-version-selection.txt` | version without revision; not-found and offline texts |
| `probes/40-luarocks-site.txt` | manifest routing, upload API, re-upload replaces, arch from file name |
| `probes/41-luarocks-upload.txt` | `luarocks upload` sends one rock at most |
| `probes/44-dist-toy-member.txt` | `precise-builds` needed; cdylib ignored by dist |
| `probes/46-clippy-toy-member.txt` | workspace build fails with the member; `--exclude` passes |
| `probes/47-ci-lua-tooling.txt` | no Lua on runner images; 5.5 defaults |
| `probes/60-glibc-linux-aarch64.txt` | glibc floor comes from the build host |
| `probes/62-macos-module-build-load.txt`, `probes/122-macos-x86_64.txt` | macOS floors; x86_64 cross-build (cargo only) |
| `probes/63-windows-module-linking.txt` | Windows module links through raw-dylib |
| `probes/65-neovim-luajit-load.txt` | Neovim loads the LuaJIT build only |
| `probes/100-msvc-module-load.txt`, `probes/120-msvc-luarocks-make.txt` | MSVC module loads; Windows load-failure text; luarocks make on Windows |
| `probes/121-linux-x86_64-glibc.txt`, `probes/124-linux-luarocks-pack.txt` | x86_64 glibc 2.14 build; Linux pack name |
| `probes/140-lua-module-private-mod.txt` | F9, `ModuleSet::open` shape, macOS library name |
| `probes/141-workspace-default-features.txt` | path dependency required (D8); D2 side effect |
| `probes/142-clippy-lints-dependency.txt` | M6 |
| `probes/143-luajit-stdlib-cfg.txt` | M5, form A |
| `probes/144-cargo-deny-features.txt` | §9.5, D10 |
| `probes/145-release-profile-export.txt` | F11 |
| `probes/146-rockspecs-make-server.txt` | rockspecs build, load, route; backend args; rock contents before S13; `pack` with `--tree` |
| `probes/147-homebrew-luarocks-local.txt` | `--local` install and `luarocks path` round trip |
| `probes/148-two-abi-target-dir.txt` | F12 |
| `probes/149-luajit-json-number.txt` | F7, example correction |
| `probes/151-luarocks-pack-arch-name.txt` | rock names from `cfg.arch` |
| `probes/160-y1-container-x86_64.txt`, `probes/161-y1-container-aarch64.txt` | job-level container cannot run node actions |
| `probes/162-y1-docker-manylinux-x86_64.txt`, `probes/163-y1-docker-manylinux-aarch64.txt` | Linux legs, LuaJIT through the 5.4 luarocks, glibc floors |
| `probes/164-y2-ubuntu-apt.txt` | apt versions, PUC 5.1 refusal, backend preinstall, Linux wrong-ABI texts |
| `probes/165-y3-brew-macos-arm64.txt`, `probes/166-y3-brew-macos-x86_64.txt` | Homebrew legs and CI interpreters; 5.5 default |
| `probes/167-y4-windows-msvc-luarocks.txt` | Windows leg; `luarocks.exe` vs `.bat`; Windows `luarocks path` |
| `probes/168…173-y5-gh-actions-lua-*.txt` | gh-actions-lua skips a second Lua |
| `probes/180-clap-lua-flag.txt` | E1–E3b, help text |
| `probes/181-toy-rocks-and-no-headers.txt` | missing-headers text |
| `probes/182-luarocks-reinstall-downgrade.txt` | reinstall exit 0; downgrade removes |
| `probes/183-luarocks-revision-pick.txt` | highest revision wins; rockspec-only revision |
| `probes/184-luarocks-lua51-unconfigured.txt` | `config` exempt; "Could not find Lua 5.1" |
| `probes/185-luarocks-target-lua-missing.txt` | 5.5 default; `--lua-version` overrides; binary install needs no interpreter |
| `probes/186-luarocks-local-home-root.txt` | HOME unset, root |
| `probes/187-luarocks-init-project-vs-local.txt` | project detection; `--local` wins |
| `probes/188-require-twice.txt` | require caching, reload, two processes |
| `probes/189-luarocks-org-names-api.txt` | names free; API errors and JSON; two-factor |
| `probes/190-dist-rocks-global-job.txt` | §10.1 job order; artifacts naming |
| `probes/191-musl-load-skipped.txt` | musl load not run |
| `probes/200-luarocks-path-luajit-hint.txt` | the LuaJIT hint's `luarocks --lua-version 5.1 path` |
| `probes/201-lockstep-manifests.txt` | §2.4 manifests and resolved features; lockstep versions |
| `probes/202-final-rockspecs.txt` | §8.1 rockspecs lint; name/content mismatch refused |
| `probes/203-luajit-utf8-coroutine.txt` | F8 |
| `probes/204-std-trim-escape.txt` | §6.1 std behaviour |
| `probes/205-luarocks-streams.txt` | `config` value on stdout, errors on stderr |
| `probes/206-lua-arg-and-error-exit.txt` | `arg[0]`/`arg[1]`; uncaught error exits 1 |
| `probes/235-rockspec-only-install-and-docs.txt` | S12: install from the rockspec alone (clone, build, install, `require`); git-missing and cargo-missing texts; S13: `copy_directories` keeps `CLAUDE.md` out (make + pack + `unzip -l`, with a control); both rockspecs lint clean |
| `probes/220-publish-downloads-build-artifacts.txt` | R2 closed: publish job downloads `rocks-*` in the same run; `artifacts-*` go to the release; undefined secret is empty; checkout under the publish token |
| `probes/221-windows-local-install-path.txt` | R4 closed: Windows `--local` lands in `%APPDATA%\luarocks`; `luarocks path` lines; the cmd and PowerShell forms of the hint; `Invoke-Expression` fails |
| `probes/230-real-module-set-stock-lua.txt` | F14: the real eleven-module set in `lua5.4` and `luajit`; entry body; env overlay and LuaJIT output order |
| `probes/231-luarocks-site-root-membership.txt` | §5.6.4 name check: root membership on every `/upload`; no pre-upload owner signal |
| `probes/232-default-members.txt` | F15: `default-members` |
| `probes/233-native-load-task-form.txt` | §9.3 native-load form, §5.6.1 messages |
| `probes/234-publish-jq-checks.txt` | §5.6.4 jq tests |

## 14. Non-goals

- Sandboxing, grants or resource limits outside airsl (intent).
- `ext`, or any stand-in for it, outside airsl (intent).
- Publishing airsl itself on LuaRocks; a rocks server of our own (intent).
- musl rocks, and platforms airsl does not release for (intent).
- Installing the native module without LuaRocks (intent).
- Changing the type file (intent constraint 7).
- A debugger in airsl (intent).
- PUC Lua 5.1, 5.2 and 5.3 (intent); Lua 5.5 and any other version (S4).
- Passing `--lua-dir` or otherwise configuring the user's luarocks from airsl (S5).
- Attaching rocks to the GitHub release (§9.4).
- A README or crates.io release for `airsl-lua` (M8 Q8).
- Supporting an airsl engine built with `luajit` (§2.4).
- Neovim integration beyond a book note (S11).
