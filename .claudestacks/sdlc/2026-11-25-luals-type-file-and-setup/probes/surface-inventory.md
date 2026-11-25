# Lua surface of the twelve host modules (crates/airsl/src/modules)

Root: `airsstack` (default). Module keys are set on per-module tables; the root table holds one key per module.

Key counts: json 3, path 9, fs 21, env 3, proc 2, regex 8, hash 4, time 4, glob 2, stdio 5, hook 3, ext 2. Total 66 keys on module tables.
All 66 are functions. No non-function values are set directly on any module table.

Error convention: every `create_function` closure returns `Err(mlua::Error)` (raised as a Lua error) for refusals and I/O failures, via `?`. Only the absent-value cases return nil or false: `env.get` (unset), `proc.which` (not found), `regex.find` / `regex.captures` (no match), `fs.create_exclusive` (returns false when the name already exists).

## json (modules/json.rs install 51-86)

| module | key | kind | Lua args (Rust closure) | returns | errors | file:line |
|---|---|---|---|---|---|---|
| json | encode | function | `(mlua::Value)` any Lua value | String (compact JSON) | raises on unencodable value (convert::to_json) | json.rs:67 |
| json | encode_pretty | function | `(mlua::Value)` | String (indented JSON) | raises | json.rs:74 |
| json | decode | function | `(mlua::LuaString)` | Lua value (tables/numbers/strings/bool/nil, ints stay integer) | raises on invalid JSON | json.rs:82 |

## path (modules/path.rs install 59-136)

| module | key | kind | args | returns | errors | file:line |
|---|---|---|---|---|---|---|
| path | join | function | `mlua::Variadic<mlua::LuaString>` (0..n strings) | String | raises on non-UTF-8 | path.rs:79 |
| path | dirname | function | `(mlua::LuaString)` | String (POSIX dirname; bare name gives ".") | raises on non-UTF-8 | path.rs:84 |
| path | basename | function | `(mlua::LuaString)` | String | raises | path.rs:89 |
| path | stem | function | `(mlua::LuaString)` | String | raises | path.rs:94 |
| path | ext | function | `(mlua::LuaString)` | String (extension function `extension`) | raises | path.rs:99 |
| path | normalize | function | `(mlua::LuaString)` | String | raises | path.rs:104 |
| path | relative_to | function | `(mlua::LuaString, mlua::LuaString)` (path, base) | String | raises via `mlua::Error::from` | path.rs:111 |
| path | is_absolute | function | `(mlua::LuaString)` | bool (native Path::is_absolute) | raises on non-UTF-8 | path.rs:126 |
| path | absolute | function | `(mlua::LuaString)` | String | raises via `mlua::Error::from` | path.rs:133 |

## fs (modules/fs.rs install 78-410; all calls go through PathGuard)

| module | key | kind | args | returns | errors | file:line |
|---|---|---|---|---|---|---|
| fs | read | function | `(mlua::LuaString)` path | String (file contents, UTF-8 read_to_string) | raises (denial or I/O) | fs.rs:101 |
| fs | read_lines | function | `(mlua::LuaString)` | sequence of String (lines; trailing \n dropped, \r stripped) | raises | fs.rs:122 |
| fs | write | function | `(mlua::LuaString, mlua::LuaString)` (path, body) | nil | raises | fs.rs:135 |
| fs | append | function | `(mlua::LuaString, mlua::LuaString)` | nil | raises | fs.rs:151 |
| fs | atomic_write | function | `(mlua::LuaString, mlua::LuaString)` | nil | raises | fs.rs:160 |
| fs | create_exclusive | function | `(mlua::LuaString, Option<mlua::LuaString>)` (path, optional body) | bool (true = created, false = already exists) | raises on other I/O | fs.rs:196 |
| fs | exists | function | `(mlua::LuaString)` | bool | raises on denial (does not answer false for ungranted) | fs.rs:212 |
| fs | is_file | function | `(mlua::LuaString)` | bool | raises on denial | fs.rs:220 |
| fs | is_dir | function | `(mlua::LuaString)` | bool | raises on denial | fs.rs:228 |
| fs | stat | function | `(mlua::LuaString)` | table: `size` integer (u64 from meta.len()), `kind` string ("dir"/"symlink"/"file"), `readonly` bool, `modified` integer (seconds, from modified_seconds) | raises | fs.rs:258 (fields set at 237, 238, 253, 254) |
| fs | canonicalize | function | `(mlua::LuaString)` | String (resolved path, script vocabulary) | raises | fs.rs:267 |
| fs | same_content | function | `(mlua::LuaString, mlua::LuaString)` | bool | raises on I/O | fs.rs:281 |
| fs | list | function | `(mlua::LuaString)` | sequence of String (entry names, sorted) | raises | fs.rs:302 |
| fs | walk | function | `(mlua::LuaString)` | sequence of String (paths relative to root, sorted via helper `walk` fs.rs:428) | raises | fs.rs:311 |
| fs | mkdir | function | `(mlua::LuaString)` | nil (create_dir_all) | raises | fs.rs:322 |
| fs | remove | function | `(mlua::LuaString)` | nil (remove_file) | raises | fs.rs:333 |
| fs | remove_dir | function | `(mlua::LuaString)` | nil (remove_dir_all) | raises | fs.rs:344 |
| fs | copy | function | `(mlua::LuaString, mlua::LuaString)` (from, to) | nil | raises | fs.rs:355 |
| fs | rename | function | `(mlua::LuaString, mlua::LuaString)` (from, to) | nil | raises | fs.rs:369 |
| fs | tempdir | function | `()` | String (created `airsl-*` dir path, kept) | raises | fs.rs:388 |
| fs | tempfile | function | `()` | String (created `airsl-*` file path, kept) | raises | fs.rs:408 |

## env (modules/env.rs install 190-270)

| module | key | kind | args | returns | errors | file:line |
|---|---|---|---|---|---|---|
| env | get | function | `(mlua::LuaString)` name | String or nil (unset) | raises on ungranted name | env.rs:215 |
| env | all | function | `()` | table: name → String (only set names; all host names when unrestricted, else granted names only) | never raises from closure | env.rs:250 |
| env | set | function | `(mlua::LuaString, Option<mlua::LuaString>)` (name, optional value; None clears) | nil | raises on ungranted name | env.rs:269 |

## proc (modules/proc.rs install 98-190)

| module | key | kind | args | returns | errors | file:line |
|---|---|---|---|---|---|---|
| proc | run | function | `(mlua::Table)` argv: sequence of strings, element 1 = program, rest = args (read via sequence_values::<LuaString>) | table: `stdout` String, `stderr` String (bytes via lua.create_string), `status` integer (exit code, -1 if no code) | raises on empty argv, denied program, unresolvable (Windows), spawn I/O error | proc.rs:176 (fields 164, 165, 172) |
| proc | which | function | `(mlua::LuaString)` | String or nil (absolute path found on PATH) | raises on denied program | proc.rs:188 |

## regex (modules/regex.rs install 151-240; compiled_table 79-140)

| module | key | kind | args | returns | errors | file:line |
|---|---|---|---|---|---|---|
| regex | compile | function | `(mlua::LuaString)` pattern | table of 7 functions (see below) | raises on invalid pattern (Error::Denied) | regex.rs:170 |
| regex | is_match | function | `(mlua::LuaString, mlua::LuaString)` (pattern, text) | bool | raises on invalid pattern | regex.rs:177 |
| regex | find | function | `(mlua::LuaString, mlua::LuaString)` | String (match) or nil | raises on invalid pattern | regex.rs:186 |
| regex | find_all | function | `(mlua::LuaString, mlua::LuaString)` | sequence of String | raises | regex.rs:195 |
| regex | captures | function | `(mlua::LuaString, mlua::LuaString)` | table or nil; table keys: integer 0..n (group strings, unmatched groups absent) plus named-group strings | raises | regex.rs:206 |
| regex | replace | function | `(mlua::LuaString, mlua::LuaString, mlua::LuaString)` (pattern, text, with) | String (first match replaced) | raises | regex.rs:217 |
| regex | replace_all | function | `(mlua::LuaString, mlua::LuaString, mlua::LuaString)` | String | raises | regex.rs:228 |
| regex | split | function | `(mlua::LuaString, mlua::LuaString)` | sequence of String | raises | regex.rs:237 |

Compiled-table functions returned by `regex.compile` (set in compiled_table, regex.rs:79-140, not on the module table):
- is_match `(LuaString text) -> bool` regex.rs:84
- find `(LuaString text) -> String | nil` regex.rs:90
- find_all `(LuaString text) -> sequence of String` regex.rs:98
- captures `(LuaString text) -> table | nil` regex.rs:107 (same shape as captures above)
- replace `(LuaString text, LuaString with) -> String` regex.rs:118
- replace_all `(LuaString text, LuaString with) -> String` regex.rs:127
- split `(LuaString text) -> sequence of String` regex.rs:135

## hash (modules/hash.rs install 63-128)

| module | key | kind | args | returns | errors | file:line |
|---|---|---|---|---|---|---|
| hash | sha256 | function | `(mlua::LuaString)` body | String (lowercase hex) | never raises from closure | hash.rs:80 |
| hash | sha1 | function | `(mlua::LuaString)` body | String (hex) | never raises from closure | hash.rs:91 |
| hash | hash_file | function | `(mlua::LuaString, Option<mlua::LuaString>)` (path, algorithm; default "sha256", or "sha1") | String (hex) | raises on denial, I/O, unknown algorithm (Error::Denied) | hash.rs:121 |
| hash | hex | function | `(mlua::LuaString)` | String (hex of raw bytes) | never raises from closure | hash.rs:126 |

## time (modules/time.rs install 74-153)

| module | key | kind | args | returns | errors | file:line |
|---|---|---|---|---|---|---|
| time | now | function | `()` | integer (Unix seconds) | never raises | time.rs:90 |
| time | monotonic | function | `()` | float (seconds since process-first call, f64) | never raises | time.rs:109 |
| time | format | function | `(i64, Option<mlua::LuaString>)` (seconds, optional strftime pattern; None gives RFC 3339) | String (UTC) | raises on invalid instant or bad pattern | time.rs:128 |
| time | parse | function | `(mlua::LuaString, Option<mlua::LuaString>)` (text, optional pattern; None = RFC 3339) | integer (Unix seconds) | raises on parse failure | time.rs:151 |

## glob (modules/glob.rs install 89-141)

| module | key | kind | args | returns | errors | file:line |
|---|---|---|---|---|---|---|
| glob | match | function | `(mlua::LuaString, mlua::LuaString)` (pattern, path) | bool | raises on invalid glob | glob.rs:106 |
| glob | walk | function | `(mlua::LuaString, mlua::LuaString)` (root, pattern) | sequence of String (matches relative to root, sorted by file name) | raises on denial, invalid glob, walk I/O | glob.rs:139 |

## stdio (modules/stdio.rs install 66-155)

| module | key | kind | args | returns | errors | file:line |
|---|---|---|---|---|---|---|
| stdio | read | function | `()` | String (all stdin) | raises on I/O | stdio.rs:80 |
| stdio | lines | function | `()` | sequence of String (stdin lines, \r stripped) | raises on I/O | stdio.rs:96 |
| stdio | write | function | `(mlua::LuaString)` | nil (raw bytes to stdout, no newline added) | raises on I/O | stdio.rs:117 |
| stdio | error | function | `(mlua::LuaString)` | nil (raw bytes to stderr) | raises on I/O | stdio.rs:136 |
| stdio | isatty | function | `(Option<mlua::LuaString>)` stream: "stdin" / "stderr" / anything else (incl. none) = stdout | bool | raises on non-UTF-8 name | stdio.rs:153 |

## hook (modules/hook.rs install 73-142)

| module | key | kind | args | returns | errors | file:line |
|---|---|---|---|---|---|---|
| hook | payload | function | `()` | Lua value from stdin JSON; empty table if stdin blank | raises on invalid JSON | hook.rs:98 |
| hook | emit | function | `(mlua::Value)` any value | nil (JSON to stdout, no newline) | raises on unencodable value or I/O | hook.rs:106 |
| hook | context | function | `(mlua::LuaString, mlua::LuaString)` (hookEventName, additionalContext) | nil (writes envelope to stdout) | raises on I/O | hook.rs:119 |

Envelope written by `hook.context` (built in context_envelope, hook.rs:129-141, not a module key):
`{ hookSpecificOutput = { hookEventName = <String>, additionalContext = <String> } }`, serialized to JSON.

## ext (modules/ext.rs install 72-124)

| module | key | kind | args | returns | errors | file:line |
|---|---|---|---|---|---|---|
| ext | on | function | `(mlua::LuaString, mlua::Function)` (event name, handler) | nil (stored in registry table) | raises `Error::UnknownEvent` if event not declared; raises on invalid EventName | ext.rs:105 (closure 90-104) |
| ext | granted | function | `()` | deep copy of the grant snapshot table (see below) | raises only on Lua allocation failure | ext.rs:117 |

`ext.granted()` returns table built by granted_table (ext.rs:154-205):
- `fs` table: `read` = sequence of String (sorted roots), `write` = sequence of String (sorted roots) (ext.rs:176-178)
- `env` table: name → `true` for each granted env name (ext.rs:182-184)
- `proc` table: `run` table: program name → `true` (ext.rs:189-192)
- `limits` table: `memory` integer and/or `instructions` integer, present only when limit is set (ext.rs:196-201)
- `unrestricted` bool (ext.rs:203)

## Userdata

None. No `UserData` impl and no `add_method` / `add_function` on userdata anywhere in these modules. The `regex.compile` return is a plain Lua table of closures (regex.rs:79-140), not userdata.

## Option tables and table arguments

- proc.run (proc.rs:98-176, argv helper proc.rs:77-91): takes a single `mlua::Table` used as a sequence of strings (argv). No named option fields. Program is element 1.
- ext.on (ext.rs:90): takes an `mlua::Function`.
- Only other table arguments: json.encode/encode_pretty/hook.emit/hook.context take `mlua::Value`/`LuaString`, not option tables.
- No option table with named fields is accepted anywhere. Optional positional arguments are `Option<mlua::LuaString>` (fs.create_exclusive body, env.set value, hash.hash_file algorithm, time.format / time.parse pattern, stdio.isatty stream).

## Doc comments describing Lua-level signatures

- hook.rs:1-34 module doc: JSON envelope `{"hookSpecificOutput": {"hookEventName": "PreToolUse", "additionalContext": "…"}}`; events PreToolUse, UserPromptSubmit, SessionStart.
- hook.rs:84-85 inline comment: payload reads and decodes in one call.
- hook.rs:90-92 inline comment: empty stdin gives an empty table, not an error.
- hook.rs:108-110 inline comment: envelope nesting; `hookEventName` is an argument.
- ext.rs:1-16 module doc: purpose of `on` and `granted`.
- ext.rs:152 doc on granted_table: snapshot built once at install, copied per call.
- env.rs:222-224 inline comment: `env.all` sorted and deduplicated when unrestricted.
- glob.rs:125-127 inline comment: match against path relative to root.
- time.rs:92-102 inline comment: `monotonic` uses Instant, not SystemTime.
- regex.rs:68-69 inline comment: captures keyed by both number and name.
- stdio.rs:98-99 inline comment: `write` adds no trailing newline.
- path.rs:113-122 inline comment: `is_absolute` and `absolute` use platform grammar, not the `/` vocabulary.

## Install-time dependencies beyond the caller's (Q4)

- hook.rs: no registry or Lua state required at install. Install only creates closures. At call time `payload` uses `stdio::read_stdin` (stdio.rs:49), a Rust fn, not a Lua value.
- ext.rs: at install writes two named registry values:
  - `airsl.ext.handlers` (HANDLERS_KEY, ext.rs:28) = fresh empty table, set at ext.rs:84
  - `airsl.ext.granted` (GRANTED_KEY, ext.rs:124) = grant snapshot table, set at ext.rs:109
  - `on` reads `airsl.ext.handlers` at call time (ext.rs:100). `granted` reads `airsl.ext.granted` at call time (ext.rs:113).
  - `on` refuses events not in `Ext::events` (ext.rs:98-104); `Ext::new()` declares none (ext.rs:40-42).
  - Needs `InstallContext` grants and policy limits (ext.rs:107-108).

## Root table and attachment (Q5)

- `install_modules` (builder.rs:184-203):
  - builder.rs:196 `let root = lua.create_table()`
  - builder.rs:198-200 for each module in ModuleSet: `let table = lua.create_table()`, `module.install(lua, &table, &context)` (builder.rs:199), `root.set(module.name().as_str(), table)` (builder.rs:200)
  - builder.rs:202 `lua.globals().set(root_table.as_str(), root)`; the global name is `RootTable`, default `airsstack` (types/root_table.rs:18).
- Called from builder.rs:118 `install_modules(&lua, &modules, &self.policy, &self.root)?`.
- Module set comes from `stdlib()` (modules/stdlib.rs:22-40): json, path, fs, env, proc, regex, hash, time, glob, stdio, hook, ext.
- Each module's `name()` is its key under the root table (e.g. `airsstack.json`).
- Nothing else sets module keys on the root table in these modules.

## divergence

None found for the surface itself. Inline comment at proc.rs:184-190 says `which` is "the same `which` `airsstack.proc.which` uses"; code matches (proc.rs:188 `which` closure).
