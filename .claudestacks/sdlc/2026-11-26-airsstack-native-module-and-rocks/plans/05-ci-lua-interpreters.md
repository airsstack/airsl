---
status: approved
created: 2026-11-26
depends-on: [04]
built-on: af7c0dcc7368
---

# CI Lua Interpreters Implementation Plan

**Goal:** The `dod` job of `ci.yml` has a Lua 5.4 and a LuaJIT interpreter on Ubuntu, macOS and
Windows before it runs `cargo make dod`.

**Architecture:** No runner image ships Lua (`probes/47-ci-lua-tooling.txt`), and the gate now
loads the native module into both interpreters (plan 04). Each OS gets its interpreters before
`cargo make dod`:
- **Ubuntu:** apt's `lua5.4` and `luajit`, which match the task's default names.
- **macOS:** Homebrew's `lua@5.4` and `luajit`. `lua@5.4` is keg-only, so the job exports
  `AIRSL_LUA54`.
- **Windows:** MSVC source builds made by a new script, `crates/airsl-lua/ci/msvc-lua.cmd`, after
  `ilammy/msvc-dev-cmd` enters the MSVC environment. The job then exports `AIRSL_LUA54` and
  `AIRSL_LUAJIT` as absolute paths.

`msvc-lua.cmd` exists so that the `dod` job and the Windows leg of `build-rocks.yml` (plan 09) build
the same interpreters. Every download is checked against the SHA-256 the probes recorded. LuaJIT
is pinned to commit `96f7b297326e9fd454b7186c6858b2773f94a0dc`. gh-actions-lua is not used (§9.4).
There is no caching (§9.4).

**Tech Stack:** GitHub Actions (`ubuntu-latest`, `macos-latest`, `windows-latest`), apt, Homebrew,
MSVC (`cl`, `link`, LuaJIT's `msvcbuild.bat`), `certutil`, `ilammy/msvc-dev-cmd` pinned to
`0b201ec74fa43914dc39ae48a89fd1d8cb592756`.

**Content authority:** spec §1.2 S10, §2.1 (`ci.yml`, `ci/msvc-lua.cmd`), §2.2 (`ci/msvc-lua.cmd`
row), §2.10 M9, M14, §9.4 (`ci.yml` part, the table, the steps, pinning, caching), §12 R12.

---

## Files that change

```
crates/airsl-lua/ci/msvc-lua.cmd   — create  builds Lua 5.4.8 (lua54.dll, lua.exe) into C:\lua54 and pinned LuaJIT (lua51.dll, luajit.exe) into C:\luajit with MSVC
.github/workflows/ci.yml           — modify  dod job installs Lua 5.4 and LuaJIT per OS and sets AIRSL_LUA54/AIRSL_LUAJIT where names differ
```

## Order of work

### Task 1 — Add the shared MSVC build of Lua 5.4.8 and LuaJIT

**Files:**
- Create `crates/airsl-lua/ci/msvc-lua.cmd`, a cmd script run inside the MSVC developer environment.
  It follows the steps of job Y4 in `probes/160-ci-probe-src/.github/workflows/probe-ci.yml`
  ("build lua 5.4.8 (MSVC, lua54.dll) into C:\lua54" and "build luajit v2.1 (MSVC, msvcbuild.bat)
  into C:\luajit"), made fail-fast.
  - **Lua 5.4.8:**
    1. download `lua-5.4.8.tar.gz`;
    2. compare its `certutil -hashfile … SHA256` against
       `4f18ddae154e793e46eeab727c59ef1c0c0c2b744e7b94219710d76f530629ae`, and exit non-zero on a
       mismatch;
    3. build with `cl /MD /O2 /DLUA_BUILD_AS_DLL`, link `lua54.dll` with `lua54.lib` and `lua.exe`;
    4. install into `C:\lua54\bin`, `\lib` and `\include`.
  - **LuaJIT:**
    1. clone LuaJIT and check out exactly commit `96f7b297326e9fd454b7186c6858b2773f94a0dc`, never
       the moving `v2.1` branch;
    2. run `msvcbuild.bat`;
    3. install `luajit.exe`, `lua51.dll`, `lua51.lib` and the headers into `C:\luajit\bin`,
       `\lib` and `\include`.
  - Every step checks `%ERRORLEVEL%` and exits non-zero on failure, instead of the probe's
    `continue-on-error`.
  - It ends by running `C:\lua54\bin\lua.exe -v` and `C:\luajit\bin\luajit.exe -v`.
  - A leading comment says what the script builds, why it is shared (the `dod` job and the rock
    leg build identical interpreters), and that it must run in an MSVC developer prompt.

**Change:** One script builds the Windows interpreters for every caller. The luarocks zip is not
part of it; plan 09's Windows leg downloads that zip.

**Proof:** There is no local run: the script needs Windows with MSVC. Its first run is the pull
request's Windows `dod` job (R12, accepted). Before committing, check it by reading it against
probe-ci.yml job Y4 and the hashes and commit of §9.4. Its run on CI must print the two version
lines `Lua 5.4.8` and `LuaJIT 2.1.…`.

**Done when:** the script holds every §9.4 pin (the Lua tarball SHA-256 and the LuaJIT commit), and
`cargo make dod` is green locally. Commit `ci(airsl-lua): build Lua 5.4 and LuaJIT with MSVC in one
shared script`.

### Task 2 — Install the interpreters in the `dod` job

**Files:**
- Modify `.github/workflows/ci.yml`, job `dod`. Add the steps before `cargo make dod`
  (`ci.yml:80-81`), each limited to its OS with `if: runner.os == …`:
  - **Ubuntu:** `sudo apt-get install -y lua5.4 luajit` (`probes/164-y2-ubuntu-apt.txt`:138). Set no
    variable, because the default names match.
  - **macOS:** `brew install lua@5.4 luajit`, then append
    `AIRSL_LUA54=$(brew --prefix lua@5.4)/bin/lua5.4` to `$GITHUB_ENV`
    (`probes/165-y3-brew-macos-arm64.txt`:160 and its `brew --prefix lua@5.4` line).
  - **Windows:**
    1. `uses: ilammy/msvc-dev-cmd@0b201ec74fa43914dc39ae48a89fd1d8cb592756` with `arch: x64`;
    2. a `shell: cmd` step running `crates\airsl-lua\ci\msvc-lua.cmd`;
    3. a step appending `AIRSL_LUA54=C:\lua54\bin\lua.exe` and `AIRSL_LUAJIT=C:\luajit\bin\luajit.exe`
       to `$GITHUB_ENV`.

  Each new step carries a comment in the file's style:
  - why: the gate loads the native module into real interpreters, and no runner image ships Lua;
  - Lua is pinned to 5.4, because Homebrew's `lua` and gh-actions-lua default to 5.5;
  - gh-actions-lua is not used, because installed twice in one job it keeps the first Lua
    (`probes/171-y5-gh-actions-lua-ubuntu-cold.txt`).

  Leave the `deny`, `book` and `installer` jobs and the `cargo make examples` step unchanged.

**Change:** `cargo make dod` finds both interpreters on all three CI operating systems, so
native-load runs there instead of failing with the §5.6.1 message.

**Proof:** CI's first run of the edited job is the real proof (R12). Locally:
- `cargo make dod` is green.
- The file is still valid YAML:
  `python3 -c 'import sys, yaml; yaml.safe_load(open(sys.argv[1]))' .github/workflows/ci.yml`
  exits 0 (PyYAML, already installed with the local `python3`). This checks YAML syntax only;
  GitHub checks the workflow schema on the pull request.
- `git diff` shows only added steps inside the `dod` job.

**Done when:** `cargo make dod` is green locally, the YAML parse above exits 0, and the diff touches
only the `dod` job's steps.
Commit `ci: install Lua 5.4 and LuaJIT for the gate on every OS`.

---

## Risks

- This plan is the riskiest for CI. Nothing here can run locally:
  - the Windows MSVC build;
  - `AIRSL_LUA54`/`AIRSL_LUAJIT` as absolute paths through duckscript's `which`;
  - the Linux and Windows native-load runs (R12).

  Each step's form comes from a probe that ran on that runner (`probes/164-y2-ubuntu-apt.txt`,
  `probes/165-y3-brew-macos-arm64.txt`, `probes/167-y4-windows-msvc-luarocks.txt`). The pull
  request's `dod` job is the first combined run.
- apt and Homebrew versions follow the runner image and are not pinned (§9.4). `check.lua` refuses a
  wrong interpreter (§5.6.2), so drift fails loudly.
- Not done here: luarocks on CI, which only `build-rocks.yml` needs (plan 09).

## Proof

- `cargo make dod` is green locally.
- On the pull request, the `dod` job is green on ubuntu-latest, macos-latest and windows-latest, and
  its `test` step prints both `check.lua: ok` lines on each.
