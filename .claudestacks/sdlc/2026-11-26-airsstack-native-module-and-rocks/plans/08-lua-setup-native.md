---
status: approved
created: 2026-11-26
depends-on: [01, 04, 07]
built-on: af7c0dcc7368
---

# `airsl lua setup --native` Implementation Plan

**Goal:** `airsl lua setup --native [--lua 5.4|luajit]` installs, with LuaRocks, the native-module
package that matches the Lua luarocks targets, at the running airsl's version.

**Architecture:** `crates/airsl-cli/src/lua.rs` becomes the folder `lua/`, with one role per file:

| File | Role |
|---|---|
| `mod.rs` | docs, `mod command; mod luarocks; mod native;` and `pub(crate) use command::run;` |
| `command.rs` | today's dispatch, plus the ordering of the native stages and all printing; streams stay injected (M-IMPL-IO) |
| `native.rs` | decides what to install: the detection parse (§6.1), `Package`, `NativeRequest`, `Flavour`, the luarocks argv, the hint, and `NativeError` with the §5.2 texts |
| `luarocks.rs` | the only code in the CLI that spawns a process: `pub(crate) trait Luarocks` with `query`/`run`, `Captured`, `Outcome`, `System` and `LuarocksError` |

`main.rs` is unchanged, because `lua::run` still resolves.

`cli.rs` gains:
- `LuaAbi`, one type for both the flag value and the detection result (modularity Rule 2);
- `Setup`'s `native` and `lua` flags. clap's `native: bool` becomes `Option<NativeRequest>` at the
  dispatch boundary (strong-types).

Each generic is `L: Luarocks`, with no new `dyn` (static dispatch). `airsl-cli` gains `thiserror`
for the two error enums. The plan depends on plans 04 and 07 only because each of them also writes
`Cargo.lock` (plan 04 adds `airsl-lua`, plan 07 adds `toml` to docs-check); running after both
keeps every lockfile edit in one order: 03, 04, 07, then this plan.

The stages run in the §4 order and stop at the first failure:
1. S1: read-only detection;
2. S2: the type file and `.luarc` through the unchanged `airsl::luals::setup` with `stdlib()`;
3. S3: the report, written and flushed;
4. S4: `install --local`;
5. S5: the hint.

Tasks 2–5 add items before their first non-test caller (Task 6). On each item, field or variant
that rustc reports as dead in the non-test build, and only there, the task adds
`#[cfg_attr(not(test), expect(dead_code, reason = "…"))]`, with a reason naming Task 6 as the
first caller. This is the precedent at `crates/airsl/src/paths/rules.rs`, `has_separator`. Task 6
deletes every one. An `expect` that is no longer needed fails the gate as an unfulfilled
expectation, so none can be left behind.

**Tech Stack:** Rust 1.94, crate `airsl-cli`, clap 4.6.6 derive, `thiserror` 2, `std::process::Command`,
`tempfile` (dev). Cargo runs with `CARGO_BUILD_JOBS=2` (`CLAUDE.md`).

**Content authority:** spec §1.2 S3–S6, S14, §2.1 (`crates/airsl-cli/`), §2.2 (rows for
`airsl-cli/src/lua/` through `cli.rs` `LuaAbi`), §2.8 (rows for no God object, static dispatch, domain
types, errors, mockable syscalls, sans-IO, magic strings, inline tests), §3.3, §4, §5.1, §5.2
(E1–E18), §5.3, §6.1, §6.2, §7, §9.1 (`cli.rs`, `native.rs`, `luarocks.rs`, `command.rs` tests).

---

## Files that change

```
crates/airsl-cli/src/lua.rs          — delete  moved into lua/
crates/airsl-cli/src/lua/mod.rs      — create  docs (with the category-1 exemption line), mod declarations, pub(crate) use command::run
crates/airsl-cli/src/lua/command.rs  — create  lua setup dispatch, native stage order, printing, exit code; today's three tests plus nine
crates/airsl-cli/src/lua/luarocks.rs — create  trait Luarocks, Captured, Outcome, System, LuarocksError; four tests
crates/airsl-cli/src/lua/native.rs   — create  Package, NativeRequest, Flavour, detection, install argv, hint, NativeError; twenty-seven tests
crates/airsl-cli/src/cli.rs          — modify  + LuaAbi; Setup gains --native and --lua; eight tests; two existing tests destructure with `..`
crates/airsl-cli/Cargo.toml          — modify  + thiserror = { workspace = true }
Cargo.lock                           — modify  thiserror joins airsl-cli's dependency list (written by cargo)
```

## Order of work

### Task 1 — Move `lua.rs` into a `lua/` folder without changing behaviour

**Files:**
- Delete `crates/airsl-cli/src/lua.rs`.
- Create `crates/airsl-cli/src/lua/mod.rs`:
  - `//!` in the house shape: what `airsl lua` is, why it is a folder (dispatch, the native flow and
    the process boundary are distinct roles), and `Responsibilities:` naming each file;
  - a line naming unit-test exemption category 1 (declarations only);
  - `mod command;` and `pub(crate) use command::run;`.
- Create `crates/airsl-cli/src/lua/command.rs` with today's `lua.rs` body: `run`, `setup`, `fail`,
  the module-level `expect(clippy::redundant_pub_crate)`, and the three tests
  `prints_the_report_and_exits_zero`, `prints_the_error_with_the_airsl_prefix_and_exits_one` and
  `a_failed_report_write_exits_one`. Its `//!` is today's, minus "nothing here loads or runs Lua",
  which Task 6 rewrites once it spawns luarocks.

**Change:** Behaviour is unchanged.

**Proof:**
- The three moved tests pass unchanged. Before and after the move,
  `cargo test -p airsl-cli lua::` lists the same three tests.
- No new test.

**Done when:** `cargo test -p airsl-cli lua::` passes with three tests and `cargo make dod` is green.
Commit `refactor(airsl-cli): move lua setup into a lua/ folder`.

### Task 2 — Add the process boundary to luarocks

**Files:**
- Create `crates/airsl-cli/src/lua/luarocks.rs`:
  - `//!`: why it is its own file (the only process spawn in the CLI, behind a trait so every failure
    path is unit-tested without luarocks; M-MOCKABLE-SYSCALLS), `Responsibilities:` and
    `Non-responsibilities:` (deciding what to run and what a failure means, which belongs to
    `native.rs`).
  - `pub(crate) trait Luarocks` with:
    - `query(&self, args: &[&str]) -> Result<Captured, LuarocksError>`: stdout captured, stderr
      inherited, stdin null;
    - `run(&self, args: &[&str]) -> Result<Outcome, LuarocksError>`: stdout and stderr inherited,
      stdin null.
  - `Captured { stdout: Vec<u8>, code: Option<i32> }` and `Outcome { code: Option<i32> }`. They are
    crate types, because a `std::process::ExitStatus` cannot be built portably in tests.
  - `System { program: OsString }`, whose `Default` program is `luarocks`. It spawns `Command::new`
    of the program and never sets a working directory, so luarocks runs in airsl's (§6.2).
  - `LuarocksError::Missing` for `io::ErrorKind::NotFound`, and `LuarocksError::Spawn { source }`
    for any other spawn error (`probes/09-process-command-errors.txt`).
- Modify `crates/airsl-cli/src/lua/mod.rs`: add `mod luarocks;`.

**Change:** A tested, injectable way to run luarocks. Nothing calls it outside tests yet, so the
`dead_code` rule in Architecture applies.

**Proof:** Each test is written first and seen failing:
- `a_missing_program_is_reported_as_missing`: `System` pointed at a path that does not exist returns
  `LuarocksError::Missing` from `query`.
- `a_non_executable_file_is_a_spawn_failure` (unix): a temp file without the execute bit returns
  `LuarocksError::Spawn` whose source is `PermissionDenied`.
- `a_killed_child_has_no_exit_code` (unix): a child that kills itself with a signal yields
  `code: None`.
- `query_captures_stdout_and_the_exit_code`, on every OS: `System` pointed at
  `std::env::current_exe()` with `--list`, which runs no test, gives non-empty stdout and
  `code == Some(0)`.

**Done when:** `cargo test -p airsl-cli lua::luarocks` passes and `cargo make dod` is green. Commit
`feat(airsl-cli): add a mockable boundary for running luarocks`.

### Task 3 — Add the `--lua` value type and the two flags

**Files:**
- Modify `crates/airsl-cli/src/cli.rs`:
  - add `LuaAbi`, a `ValueEnum` deriving `Debug, Clone, Copy, PartialEq, Eq`, with variants:
    - `#[value(name = "5.4")] Lua54`, doc "Lua 5.4 (PUC-Rio).";
    - `#[value(name = "luajit")] LuaJit`, doc "LuaJIT 2.1 (the Lua 5.1 ABI).".
  - give `LuaCommand::Setup`:
    - `#[arg(long)] native: bool`, doc "Also install the native module with LuaRocks.";
    - `#[arg(long, value_enum, requires = "native")] lua: Option<LuaAbi>`, doc "The Lua to install
      the native module for, instead of the one luarocks targets.".

    Keep the existing doc paragraphs of `Setup`, whose first line is what `-h` shows (§3.3).
  - list `LuaAbi` in the module `//!` `Responsibilities:`.
  - in the existing `lua_setup_defaults_to_the_working_directory` and `lua_setup_takes_a_directory`,
    destructure `Setup { dir, .. }`.
- Modify `crates/airsl-cli/src/lua/command.rs`:
  - match `LuaCommand::Setup { dir, .. }`, because Task 6 reads the new fields. Any `dead_code`
    rustc reports for them follows the Architecture rule;
  - in the tests' `setup_command` helper, build `native: false, lua: None`.

**Change:**
- `airsl lua setup -h` renders the §3.3 help block.
- clap refuses E1, E2, E3 and E3b with exit 2 and the §5.2 texts (`probes/180-clap-lua-flag.txt`,
  clap 4.6.6).

**Proof:** Each test is written first and seen failing. All use `Cli::try_parse_from`, like
`lua_setup_takes_a_directory`:
- `lua_setup_takes_native`: `lua setup --native` parses to `native == true`, `lua == None`.
- `lua_setup_takes_native_with_lua_5_4`: `--native --lua 5.4` parses to `Some(LuaAbi::Lua54)`.
- `lua_setup_takes_native_with_luajit`: `--native --lua luajit` parses to `Some(LuaAbi::LuaJit)`.
- `lua_setup_without_native_has_no_lua`: plain `lua setup` parses to `native == false`, `lua == None`.
- `lua_setup_refuses_lua_without_native` (E2): `--lua 5.4` alone is an error whose rendered text
  starts with `error: the following required arguments were not provided:` and names `--native`.
- `lua_setup_refuses_an_unknown_lua` (E1): `--native --lua 5.1` renders
  `invalid value '5.1' for '--lua <LUA>'`, `[possible values: 5.4, luajit]` and
  `tip: a similar value exists: '5.4'`.
- `lua_setup_refuses_lua_twice` (E3): `--native --lua 5.4 --lua luajit` renders
  `the argument '--lua <LUA>' cannot be used multiple times`.
- `lua_setup_refuses_lua_without_a_value` (E3b): both `--native --lua`, with `--lua` last, and
  `--native --lua=` render `a value is required for '--lua <LUA>' but none was supplied`.

**Done when:** `cargo test -p airsl-cli cli::` passes, `cargo run -p airsl-cli -- lua setup -h`
prints the §3.3 block, and `cargo make dod` is green. Commit `feat(airsl-cli): parse --native and
--lua for lua setup`.

### Task 4 — Decide the package from luarocks' answer

**Files:**
- Create `crates/airsl-cli/src/lua/native.rs`:
  - `//!`: why it is its own file (it decides what to install and owns the error texts; it spawns
    nothing), `Responsibilities:` and `Non-responsibilities:` (spawning, which belongs to
    `luarocks.rs`, and printing and ordering, which belong to `command.rs`).
  - Types:
    - `Package` (`Airsstack`, `AirsstackLuajit`), built from `LuaAbi`, with `as_str` giving
      `airsstack` / `airsstack-luajit`;
    - `NativeRequest { lua: Option<LuaAbi> }`;
    - `Flavour` (`Unix`, `Windows`), passed in by the caller so both texts are tested on every OS.
  - Documented consts for the luarocks argv parts, each naming the luarocks behaviour it relies on
    (M-DOCUMENTED-MAGIC): `config`, `lua_version`, `--lua-version`, and the `5.4`/`5.1` values.
  - One argv builder used both to spawn and to render `{command}`, so the command a message names is
    always the one that ran (§5.1).
  - `setup_native<L: Luarocks>(luarocks: &L, request: &NativeRequest, flavour: Flavour) ->
    Result<Package, NativeError>` runs `config lua_version`, with `--lua-version 5.4|5.1` first when
    `--lua` is given, and applies §6.1 in its exact order:
    1. a spawn error (E4–E6);
    2. no exit code (E7), whatever stdout holds;
    3. a non-zero code (E8), whatever stdout holds;
    4. the stdout rule: trim ASCII whitespace only (`<[u8]>::trim_ascii`);
       - empty is E9;
       - not UTF-8, or not ASCII `digits.digits`, is E10, shown lossily through `escape_debug`;
       - `5.4` gives `airsstack` and `5.1` gives `airsstack-luajit`;
       - any other `digits.digits` is E11 without `--lua` and E12 with it;
       - with `--lua`, the value must equal the requested `5.4`/`5.1`.
  - `NativeError`, a `thiserror` enum with one variant per §5.2 row it owns: E4, E5/E6, E7, E8, E9,
    E10, E11, E12. Its `Display` is the §5.2 text without the `airsl: ` prefix, which `fail` adds.
    Continuation lines are indented seven spaces (§5.1). Install command lines are printed bare.
    Names and commands inside a sentence keep their backticks. The E4 install lines use
    `airsl::VERSION`. Under `Flavour::Windows`, E4 names `luarocks.exe` and adds the `.bat` wrapper
    line, and E5/E6 name `luarocks.exe`.
- Modify `crates/airsl-cli/src/lua/mod.rs`: add `mod native;`.
- Modify `crates/airsl-cli/Cargo.toml`: add `thiserror = { workspace = true }` under `[dependencies]`.
- Modify `Cargo.lock`: commit what cargo writes for the new dependency edge.

**Change:** One tested function turns luarocks' `config lua_version` answer into a package or one of
the §5.2 refusals.

**Proof:** Each test is written first and seen failing. All go through a scripted fake `Luarocks`
in the test module, which returns queued results and records each argv. Exact texts are §5.2 at
version `airsl::VERSION`:
- `lua_5_4_chooses_airsstack`: stdout `5.4\n`, code 0 → `Package::Airsstack`.
- `lua_5_1_chooses_airsstack_luajit`: `5.1\n` → `Package::AirsstackLuajit`.
- `a_padded_or_crlf_version_is_trimmed`: ` 5.4 \r\n` and `5.4` → `airsstack`.
- `an_empty_answer_is_refused_as_no_version`: empty, `\n` and `  \t ` each give E9.
- `a_three_part_version_is_refused_as_not_a_version`: `5.4.8\n` gives E10 showing `5.4.8`.
- `two_lines_are_refused_and_shown_escaped`: `5.4\n5.1\n` gives E10 showing `5.4\n5.1`, escaped.
- `non_ascii_whitespace_and_a_bom_are_not_trimmed`: `5.4` plus NBSP gives E10 showing `5.4\u{a0}`.
  BOM plus `5.4` gives E10 showing `\u{feff}5.4`. Fullwidth `５.４` gives E10.
- `invalid_utf8_is_shown_lossily`: `\xff5.4\n` gives E10 showing `�5.4`.
- `lua_5_5_is_refused_naming_the_flag_and_the_book_page`: `5.5\n` gives the three-line E11 text,
  with `--lua 5.4` and `https://airsstack.github.io/airsl/ch01-05-stock-lua.html`. `10.0\n` also
  gives E11.
- `lua_5_04_is_refused_as_unsupported_not_read_as_5_4`: `5.04\n` gives E11 for `5.04`.
- `without_the_lua_flag_no_lua_version_is_passed`: the recorded argv is exactly
  `config lua_version`.
- `the_lua_flag_refuses_a_disagreeing_answer`: `--lua luajit` with stdout `5.4` gives E12 naming
  `luarocks --lua-version 5.1 config lua_version` and `--lua luajit`. `--lua 5.4` with stdout `5.1`
  gives the 5.4 form.
- `a_config_failure_names_the_command_and_its_status`: code 1 gives E8 with
  `` `luarocks config lua_version` failed with exit status 1 ``.
- `a_nonzero_config_exit_wins_over_a_valid_version_on_stdout`: code 1 with stdout `5.4\n` gives E8.
- `a_config_signal_is_reported`: code `None` gives E7.
- `a_config_signal_wins_over_a_valid_version_on_stdout`: code `None` with stdout `5.4\n` gives E7.
- `a_spawn_failure_names_luarocks_and_the_io_error`: `Spawn` with a permission-denied error gives
  E5, and with another I/O error gives E6, each under both `Flavour`s (`luarocks` / `luarocks.exe`).
- `a_missing_luarocks_prints_both_install_lines`: `Missing` without `--lua` gives E4 with both
  `luarocks --lua-version 5.4 install --local airsstack <v>` and
  `luarocks --lua-version 5.1 install --local airsstack-luajit <v>` as bare lines.
- `a_missing_luarocks_with_the_lua_flag_prints_one_install_line`: `Missing` with `--lua 5.4` gives
  the one-line `install LuaRocks, then run: …` form.
- `a_missing_luarocks_on_windows_names_the_exe_and_the_bat_wrapper`: under `Flavour::Windows`, the
  first line names `luarocks.exe`, followed by the wrapper line.

**Done when:** `cargo test -p airsl-cli lua::native` passes and `cargo make dod` is green. Commit
`feat(airsl-cli): choose the native package from luarocks' Lua version`.

### Task 5 — Build the install command and the path hint

**Files:**
- Modify `crates/airsl-cli/src/lua/native.rs`:
  - add the documented consts `install`, `--local` and `path`;
  - add `install_native<L: Luarocks>(luarocks: &L, request: &NativeRequest, package: Package,
    flavour: Flavour) -> Result<(), NativeError>`. It runs `install --local <package>
    <airsl::VERSION>`, with `--lua-version 5.4|5.1` first when `--lua` is given (§4 table, S4 row).
    It maps:
    - a spawn error to E4–E6, the same variants as detection;
    - code `None` to E17;
    - any non-zero code to E16, naming luarocks' status. Airsl's own exit stays 1.
  - add the hint (S5, §4 table):
    - `Flavour::Unix`: one line, ``require("airsstack") works once you run: eval "$(luarocks path)"``,
      with `luarocks --lua-version 5.4|5.1 path` when `--lua` is given;
    - `Flavour::Windows`: the six lines, with the cmd form `luarocks path > lrpath.bat` / `call
      lrpath.bat` and the PowerShell `$env:LUA_PATH = luarocks path --lr-path` /
      `$env:LUA_CPATH = luarocks path --lr-cpath` form (S14);
    - `--lua` puts `--lua-version` into all four commands;
    - column 0, with commands indented two spaces under their shell (§5.1).
  - add `NativeError` variants for E16 and E17.

**Change:** Detection, install and hint now cover every §4 command form and the §5.2 rows E16 and
E17.

**Proof:** Each test is written first and seen failing, through the scripted fake:
- `the_install_arguments_are_local_package_and_the_airsl_version`: the recorded argv is
  `install --local airsstack <airsl::VERSION>`.
- `the_lua_flag_passes_lua_version_to_both_calls`: with `--lua luajit`, both recorded argvs start
  with `--lua-version 5.1`, and install names `airsstack-luajit`.
- `an_install_failure_exits_one_whatever_luarocks_returned`: install code 2 gives E16 with
  `` `luarocks install --local airsstack <v>` failed with exit status 2 ``. The error carries no
  exit code of its own.
- `an_install_signal_is_reported`: install code `None` gives E17.
- `the_hint_names_luarocks_path_for_the_chosen_lua`: the unix hint is exactly the §4 line for no
  `--lua`, for `--lua 5.4` and for `--lua luajit`.
- `the_windows_hint_names_the_cmd_and_powershell_forms`: the Windows hint is exactly the six §4
  lines, with `--lua-version` in all four commands when given.
- `the_rock_version_is_the_cli_version`: `airsl::VERSION == env!("CARGO_PKG_VERSION")`, which holds
  under plan 01's lockstep.

**Done when:** `cargo test -p airsl-cli lua::native` passes and `cargo make dod` is green. Commit
`feat(airsl-cli): install the native package and print the luarocks path hint`.

### Task 6 — Run the native stages from `lua setup`

**Files:**
- Modify `crates/airsl-cli/src/lua/command.rs`:
  - `run(command, stdout, stderr) -> i32` keeps its signature and calls the private
    `run_with<L: Luarocks>(command, &System::default(), Flavour, stdout, stderr)`, with the
    `Flavour` picked by `cfg!(windows)`.
  - `Setup { dir, native, lua }` turns `native` into `Option<NativeRequest>`.
  - With a request, the stages run in this order, each failure going through `fail`
    (`airsl: {message}`, exit 1):
    1. S1 `setup_native`;
    2. S2 today's `setup(&dir)`, unchanged, rendering from `stdlib()` with `ext` still typed (§7);
    3. S3 `writeln!` of the report, then `flush` of stdout, where a failure is E14;
    4. S4 `install_native`;
    5. S5 the hint on stdout, where a failure is E18.
  - Without a request, the flow is today's and luarocks is never run.
  - The `//!` no longer says nothing runs. It now names the luarocks spawn and the stage order.
- Delete every `cfg_attr(not(test), expect(dead_code, …))` that Tasks 2–5 added. Each item now has
  a non-test caller.

**Change:**
- The command behaves as §4 states.
- A detection failure writes nothing. A type-file failure installs nothing. A failed report write
  does not install. luarocks' own output reaches the terminal directly above any airsl line.
- `DIR` missing gives detection, then E13. A second run prints `unchanged …`, luarocks' notice and
  the hint, and exits 0 (§6.2).

**Proof:** Each test is written first and seen failing, through the scripted fake and in-memory
streams:
- `native_detection_failure_writes_nothing`: `config` code 1 gives E8 on stderr, exit 1, and no
  `.airsl/` or `.luarc.json` in the temp project.
- `native_type_file_failure_installs_nothing`: a missing `DIR` gives the E13 text, exit 1, and the
  fake recorded `config` only.
- `native_prints_the_report_before_installing`: when the fake's `run` is called, stdout already
  holds the two report lines and has been flushed. The fake records this through a shared,
  flush-counting writer.
- `native_install_failure_keeps_the_report_and_exits_one`: install code 1 leaves the report on
  stdout, E16 on stderr, exit 1, and both files written.
- `luarocks_missing_at_install_keeps_the_report_and_prints_the_install_lines`: `config` answers
  `5.4`, then `install` returns `Missing`. The report is on stdout, E4 is on stderr, and exit is 1.
- `native_success_prints_the_report_then_the_hint`: stdout is the two report lines followed by the
  hint for the given `Flavour`, stderr is empty, and exit is 0.
- `a_failed_report_write_skips_the_install`: a failing stdout gives `airsl: boom`, exit 1, and the
  fake recorded no `install`.
- `a_failed_hint_write_exits_one`: stdout accepts the report and fails on the hint, which gives
  `airsl: {io error}` and exit 1, after `install` ran.
- `without_native_luarocks_is_never_run`: plain `lua setup` records no luarocks call and keeps
  today's output.
- The three tests moved in Task 1 still pass.

**Done when:** `cargo test -p airsl-cli` passes, `grep -rn "dead_code" crates/airsl-cli/src` finds
nothing, and `cargo make dod` is green. Commit `feat(airsl-cli): install the native module
from lua setup --native`.

---

## Risks

- Task 6 is the riskiest step, because it reorders a command users already run. The three moved
  tests and `without_native_luarocks_is_never_run` guard today's path.
- No test spawns a real luarocks. `System` is proven against the test binary itself, and the
  luarocks behaviours it relies on are the spec's probes (§4 "Why these commands hold"). The first
  real `--native` run against luarocks.org can only print luarocks' "No results matching query"
  until the release (§5.4).
- E15 does not exist (§5.2). An install that cannot be spawned is E4, E5 or E6 at S4.
- Not done here: the book's chapter and appendix (plan 11) and the README command table (plan 12).

## Proof

- `cargo test -p airsl-cli` passes: 8 `cli.rs` tests, 4 `luarocks.rs` tests, 27 `native.rs` tests
  and 12 `command.rs` tests, all named in §9.1.
- `cargo run -p airsl-cli -- lua setup -h` prints the §3.3 help.
- `cargo make dod` is green.
- `grep -rn "Box<dyn\|Arc<dyn" crates/airsl-cli/src/lua` finds nothing.
