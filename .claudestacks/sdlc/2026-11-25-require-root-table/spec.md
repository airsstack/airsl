---
status: approved
created: 2026-11-25
built-on: aa2cd27de821
---

# Spec: `require("<root>")` returns the root table

After this change `require("<root>")` returns the engine's own root table under every surface and
grant, for a script on disk or built from source. `<root>` is the engine's root name (`airsstack`
by default). The root name always wins over a user file with the same name, the global keeps
working, and every other `require` call behaves as it does today. The LuaLS type file gains
`return <root>` so a local bound with `require` is typed like the global. This is milestone
`lua-ecosystem/02`. Probe records live in `probes/` beside this file, and `probes/README.md`
indexes them.

## 1. Decisions

Settled with the author during design.

| # | Question | Decision |
|---|---|---|
| D1 | How `trusted` (the `Full` surface, Lua's own `require`) finds the root | Put the root table into `package.loaded[<root>]` when the engine is built. Lua's own `require` checks `package.loaded` first. Lua's `require` is not wrapped |
| D2 | Which table `require("<root>")` returns once a script has replaced the global | The engine's own table: the one it installed its modules into, kept in the registry (and in `package.loaded` on `Full`) |
| D3 | A `trusted` script clears or replaces `package.loaded[<root>]` | Accepted, as in stock Lua. After clearing, later calls (including later evaluations on a reused engine) search `package.path`. After replacing, they return the replacement. The entry is written once, at build. The intent's Desired outcome records this exception |
| D4 | What a `require` with no directory (a script from source under a granting policy) says for any other name | A new error, `Error::RequireNoDirectory { module }` (§5) |
| D5 | How the type file types `require("<root>")` | One final block, `return <root>`. The `---@meta` header stays unnamed |
| D6 | A root named after an entry `package.loaded` already holds (`utf8`) | The root wins: under `Full`, registering the root overwrites `package.loaded.utf8`, matching the global it already replaces |
| D7 | Which names are special | Only the bare root name, compared byte for byte (case-sensitive). `require("<root>.fs")` is an ordinary lookup |
| D8 | Module layout | `require_loader.rs` becomes the folder `require/` with role files (§7). Approved by the author on its own |

Proof behind D1 and D5:

- D1: a `package.loaded` hit returns exactly one value, and the entry wins over a file of the same
  name on `package.path` (`probes/mlua-0.12.0-require.txt:3`, `values=1 root=true user=nil`). The
  rejected alternative was wrapping Lua's `require` in a Rust function. That changes every other
  `require` error under `trusted`: Lua's own error starts `module 'nope' not found:`
  (`mlua-0.12.0-require.txt:7`), while the wrapped one starts `runtime error: module 'nope' not
  found:` and carries a second `stack traceback:` (`mlua-0.12.0-require.txt:12-27`).
- D3: after `package.loaded.airsstack = nil`, Lua's `require("airsstack")` loads the user file
  (`mlua-0.12.0-require.txt:6`, `root=false user=true`). After `package.loaded.airsstack = mine`,
  it returns `mine` (`mlua-0.12.0-require.txt:61`, `mine=true`).
- D5: with `return airsstack` appended, LuaLS 3.19.2 reports `Undefined field nope` on
  `a.json.nope` where `a = require("airsstack")` (`probes/luals-require.out:24,26`). With no edit,
  it reports nothing on that line (`luals-require.out:12-13`, only line 3, the global, is flagged).
  `---@meta airsstack` plus the return gives the same diagnostics as the return alone
  (`luals-require.out:38-41` against `:24-27`), so the name is left out. LuaLS documents a named
  `@meta` as "it will only be able to be required by the given name"
  (`probes/luals-wiki-annotations.mdx:788`).
- D6: `RootTable::new("utf8")` is accepted today, the global `utf8` is the root table, and
  `package.loaded.utf8` is still Lua's `utf8` library (`probes/airsl-0.2.0-utf8-root.txt:3-4`).
  `utf8` is absent from `SHADOWED_GLOBALS` (`crates/airsl/src/types/root_table.rs:23-53`).

## 2. Behaviour today

Each row was reproduced through airsl's public API on checkout `4620389`, for the roots
`airsstack` and `agentstacks` (`probes/airsl-0.2.0-require-today.txt`).

| Case | Today | Record line |
|---|---|---|
| `trusted`, no user file | `module 'airsstack' not found:` plus the `package.path` search list | `:3` |
| `trusted`, user `airsstack.lua` on `package.path` | the user file's table | `:14` |
| `confined` on disk, no user file | ``module `airsstack` not found under `<dir>` `` | `:15` |
| `confined` on disk, user `airsstack.lua` | the user file's table | `:21` |
| `pure` on disk | ``require denied: `require` is not granted — no access is granted`` | `:22` |
| `pure` + `require` member, on disk, user file | the user file's table | `:28` |
| `trusted` from source | `module 'airsstack' not found:` | `:29` |
| `confined` from source | `attempt to call a nil value` | `:39` |
| `pure` from source | the same `require denied:` text | `:40` |
| `pure` + `require` member, from source | `attempt to call a nil value` | `:46` |

The `agentstacks` rows (`:47-90`) match row for row.

The code behind this: `RequireDisposition::decide` (`crates/airsl/src/require_loader.rs:79-93`)
picks `Native` on `Full`. Below `Full`, it picks `Refused` without the `require` member, `Confined`
with it and a directory, and `Absent` with it and no directory. `Engine::set_require`
(`crates/airsl/src/engine.rs:283-297`) applies the result before every evaluation
(`engine.rs:245`). The confined loader converts its argument to a Lua string and validates it as a
`RequireTarget` before resolving (`require_loader.rs:135-142`). The refusal ignores its arguments
(`require_loader.rs:164`), and `Absent` clears the global (`require_loader.rs:183`).

## 3. Behaviour after

`R` is the engine's own root table, the one `install_modules` built (§4.1). "As today" means the
text and outcome recorded in §2, or in the probe line named.

| Call | `Full` (`trusted`) | `Confined` (granted, on disk) | `Refused` (not granted) | `RootOnly` (granted, from source) |
|---|---|---|---|---|
| `require("<root>")` | `R` from `package.loaded`, one value | `R`, one value. No file is read | `R`, one value | `R`, one value |
| `require("<root>", "x")` | `R`, one value (`mlua-0.12.0-require.txt:60`) | `R` (a `LuaString` parameter takes the first argument and ignores the rest, `mlua-0.12.0-require.txt:51`) | `R` (only the first argument is read) | `R` (as `Confined`) |
| `package.loaded[<root>]` replaced or cleared by the script | the replacement, or Lua's own lookup (D3) | n/a (no `package`) | n/a | n/a |
| user `<root>.lua` or `<root>/init.lua` beside the script | not loaded by `require("<root>")` (D3 aside) | not loaded | n/a | n/a |
| any other valid name, e.g. `"lib"`, `"Airsstack"`, `"<root>.fs"` | Lua's own lookup, as today | the confined lookup, as today | `RequireDenied`, as today | `RequireNoDirectory` |
| an invalid target, e.g. `"../x"`, `" airsstack"` | Lua's own lookup, as today | `invalid require target …`, as today | `RequireDenied`, as today | `invalid require target …`, the confined loader's text |
| `require()` | Lua's `bad argument #1 to 'require' (string expected, got no value)` (`mlua-0.12.0-require.txt:52`) | the conversion error `bad argument #1: error converting Lua nil to string (expected string or number)` (`mlua-0.12.0-require.txt:30`), as today | `RequireDenied`, as today | the same conversion error as `Confined` |
| `require(nil)`, `require({})` | Lua's `bad argument #1 to 'require' (string expected, got nil)` / `got table)` (`mlua-0.12.0-require.txt:53,59`) | the conversion error (`… Lua nil to string …` / `… Lua table to string …`, `mlua-0.12.0-require.txt:36,44`), as today | `RequireDenied`, as today | the same conversion error as `Confined` |
| `require(1)` | Lua looks up `"1"` (`mlua-0.12.0-require.txt:54`) | looks up `1`, as today (`1` converts to `"1"`, `mlua-0.12.0-require.txt:42`) | `RequireDenied`, as today | `RequireNoDirectory { module: "1" }` |
| custom root `agentstacks`, `require("airsstack")` | ordinary lookup | ordinary lookup | `RequireDenied` | `RequireNoDirectory` |

Rules the table follows:

- **The root is answered first.** A `require` airsl installs in Rust (`Confined`, `Refused`,
  `RootOnly`) compares the first argument with the root name before anything else happens: no
  validation, no file system access, no grant check. Only a Lua string can match. A number never
  matches, because a root name starts with a lowercase letter (`types/module_name.rs:49-50`).
- **The conversion stays where it is.** `Confined` and `RootOnly` convert the first argument to a
  Lua string first, as the confined loader does today, so `nil` and tables raise the same
  conversion error. A number becomes its string form (`mlua-0.12.0-require.txt:30,36,42,44`). The
  root comparison runs on the converted bytes. `Refused` reads the first argument without
  converting it: a string equal to the root returns `R`, and everything else is refused with
  today's `RequireDenied` text.
- **The order in `RootOnly`** is: convert, compare with the root, validate as a `RequireTarget`
  (raising today's `InvalidName`), then raise `RequireNoDirectory`. It is the confined loader's
  order, with "resolve" replaced by the refusal.
- **One value.** Every disposition returns exactly one value for the root (§1, D1, for `Full`). A
  Rust-installed `require` returns one value today (`require_loader.rs:137-141`).
- **The global is untouched.** The `<root>` global is still set as it is today
  (`builder.rs:258`). `require("<root>")` and the global are the same table until a script
  replaces the global. After that, `require` still returns `R` (D2). A table kept in the registry
  is the same table after the global is set to `nil` (`mlua-0.12.0-require.txt:29`,
  `same=true marker=root`).
- **Engine reuse.** The registry entry and `package.loaded[<root>]` are written once, at build, and
  persist across evaluations like every other piece of state a script did not clear.
- **Dispatch.** `Engine::dispatch` (`engine.rs:361`) does not reinstall `require`
  (`engine.rs:352-353`). A handler calls the `require` installed by the evaluation that registered
  it, and that function answers the root too.
- **Extensions.** `ExtensionHost` builds an ordinary engine with the host's root
  (`extension/loaded.rs:111-114`) and runs the entry with `Script::from_file(&entry)?.with_root(&dir)`
  (`loaded.rs:122-123`). An extension therefore gets `Confined` or `Refused` under its negotiated
  policy and answers `require("<host root>")` through the engine changes alone.
- **The CLI.** `airsl-cli` never builds a script from source (`git grep -n from_source
  crates/airsl-cli/src` prints nothing; the same search for `from_file` in `crates/airsl/src` prints
  30 lines). `RootOnly` is therefore reached only by library hosts. CLI behaviour changes only for
  the root name.
- **The root name is always a valid target.** `RootTable` accepts `[a-z][a-z0-9_]{0,31}` minus
  reserved words and shadowed globals (`types/module_name.rs:44-55`,
  `types/root_table.rs:80-103`). `RequireTarget` accepts ASCII letters, digits, `_`, `-` and `.`
  (`types/require_target.rs:44-62`). The comparison therefore never needs a validated target.

## 4. Mechanism

### 4.1 Registering the root (build time)

`builder.rs`'s `install_modules` (`builder.rs:240-260`) gains one call,
`require::root::register(lua, root_table, &root, policy.language())`. It goes immediately before
the line that sets the global (`builder.rs:258`), because that line moves `root` into the globals
table.

`register(lua: &mlua::Lua, name: &RootTable, table: &mlua::Table, surface: LanguageSurface) ->
Result<()>`:

- stores `table` as the named registry value `airsl.require.root`, on every surface;
- on `LanguageSurface::Full` only, sets `package.loaded[name] = table`;
- matches `surface` exhaustively, with no wildcard, as `decide` does (`require_loader.rs:77-78`);
- maps a failure to `Error::EngineSetup { stage: "registering the root table", source }`.

`Full` opens `package` (`StdLib::ALL_SAFE`, `sandbox/language_surface.rs:90`). `Restricted` and
`Minimal` do not (`language_surface.rs:96-103`), so `package.loaded` is never touched on them.

### 4.2 Answering the root (call time)

`require::root::answer(lua: &mlua::Lua, name: &RootTable, requested: &[u8]) ->
mlua::Result<Option<mlua::Table>>` returns the registry table when `requested` equals
`name.as_str()` byte for byte, and `None` otherwise.

### 4.3 Dispositions

`RequireDisposition` keeps four variants and `decide` keeps its signature. `Absent` is renamed
`RootOnly`.

| Variant | Chosen when (unchanged) | Engine installs |
|---|---|---|
| `Native` | `Full` | nothing (the `package.loaded` entry from §4.1 does the work) |
| `Confined(dir)` | below `Full`, granted, directory | `RequireLoader::new(dir, root).install` |
| `Refused` | below `Full`, not granted | `RequireLoader::install_refusal(lua, root, detail)` |
| `RootOnly` | below `Full`, granted, no directory | `RequireLoader::install_root_only(lua, root)` (new) |

`root` is a clone of the engine's `RootTable` (`engine.rs:53`), captured by each closure.
`RequireLoader::remove` (`require_loader.rs:183`) has no caller left and is deleted.
`install_root_only` maps a failure to `Error::EngineSetup { stage: "root-only require", source }`.
The existing stages, `"confined require"` and `"refusing require"`, are unchanged.

## 5. Errors

**New.** `Error::RequireNoDirectory`, placed in `error.rs` beside `RequireNotFound`
(`error.rs:383-390`). `Error` is already `#[non_exhaustive]`, so adding a variant is not breaking.

```rust
/// A script with no directory to load from called `require` for a module other than the root table.
#[error("module `{module}` cannot be loaded: the script has no directory to load modules from")]
RequireNoDirectory {
    /// The requested module name.
    module: String,
},
```

For `module: "lib"` it displays exactly
``module `lib` cannot be loaded: the script has no directory to load modules from``. A script sees
the text the way it sees the other `require` errors today: the message, then a `stack traceback:`
(the `Confined` row's shape, `airsl-0.2.0-require-today.txt:15-20`).

**Unchanged.** `RequireDenied` (`error.rs:309`, ``require denied: {detail}``), `RequireNotFound`,
`RequireEscape`, `RequireCycle`, `InvalidName` for require targets, the mlua conversion error, and
every error from Lua's own `require` under `Full`. The doc of `RequireDenied` (`error.rs:304-308`)
says the call is "refused before any module name is looked at". It becomes: refused for every name
except the root table's.

**Cannot happen.** A `require("<root>")` never errors in any disposition, unless the registry
lookup itself fails. `answer` returns that failure as the mlua error.

## 6. Type file

`TypeFile::render` (`crates/airsl/src/luals/render.rs:84-123`) pushes one final block,
`return <root>`. Blocks are joined with a blank line and the text ends with `\n`
(`render.rs:117-118`), so the file ends `…end\n\nreturn airsstack\n`. That is the byte tail
`probes/airsl-check-typefile-with-return.txt` records, and `airsl check` compiles that file
(`:4-5`, `1 file(s) compiled, 0 failed`, exit 0). The module doc's layout list (`render.rs:12`)
gains "then `return <root>`".

- The header stays `---@meta` (D5).
- Under a host root the block is `return <host root>`. LuaLS then types `require("agentstacks")`
  and leaves `require("airsstack")` untyped (`luals-require.out:80-82`).
- A user file named after the root does not stop LuaLS typing the `require` from the type file
  (`luals-require.out:52-55`).
- `airsl lua setup` writes whatever `render` returns (`luals/setup.rs:172`), so it needs no change.

## 7. Architecture (approved by the author on its own)

### 7.1 Module tree

```
crates/airsl/src/
├── require/                 replaces require_loader.rs
│   ├── mod.rs               export-only: //! block, mod declarations, pub(crate) use
│   ├── disposition.rs       RequireDisposition + decide (moved; Absent → RootOnly) and their tests
│   ├── root.rs              NEW: register (§4.1), answer (§4.2), the registry key, their tests
│   └── loader.rs            RequireLoader: install / install_refusal / install_root_only, each asking
│                            root::answer first; resolve, load, run, compile (moved unchanged) and tests
├── builder.rs               install_modules calls require::root::register
├── engine.rs                set_require passes the engine's RootTable; RootOnly arm
├── error.rs                 Error::RequireNoDirectory; RequireDenied's doc
├── lib.rs                   `mod require_loader;` → `mod require;`
├── extension/manifest.rs    comment at :298 names require::loader::resolve
└── luals/render.rs          the final `return <root>` block
```

Why each module exists:

- `require/` is one concept, the `require` a script gets. It has three distinct roles, so it is a
  folder of role-named files rather than one file.
- `disposition.rs` decides which `require` a script gets. It is pure and has no Lua state.
- `root.rs` is the only place that knows where the root table is kept for `require` and how a name
  is matched against it. The builder writes through it and every installer reads through it.
- `loader.rs` holds the functions installed as `require`, plus the confined resolution, caching and
  cycle detection they share today.

### 7.2 Dependency direction

```
builder ──► require::root ──► types::RootTable, sandbox::LanguageSurface, error
engine  ──► require::{disposition, loader}, modules::access (existing: builds the refusal's detail)
                 loader ──► require::root, types::{RequireTarget, RootTable}, paths, error
                 disposition ──► sandbox::{GrantSet, HostAccess, LanguageSurface}
```

Nothing under `require/` imports `builder` or `engine`, so the graph has no cycle. Outside its
own file, `grep -rn require_loader crates/airsl/src` lists `lib.rs:45` (`mod require_loader;`),
`engine.rs:28` (the `use`) and `extension/manifest.rs:298`, a comment naming
`require_loader::resolve` that becomes `require::loader::resolve`. `builder.rs → require::root` is
the one new edge.

### 7.3 Guideline rules applied

| Rule (`claudestacks-guideline-rust`) | How it is applied |
|---|---|
| `modularity.md` Rule 1 | Answering the root is a third concept, after the disposition and the loader mechanics, so the file is split in the same change |
| `modularity.md` Rule 2 | `RootTable` stays the one root-name type. No new name type is introduced |
| `mod-rs-export-only.md` | `require/mod.rs` has only the `//!` block, `mod` lines and `pub(crate) use` |
| Global module-layout rule | Role-named files under the concept's folder. There is no `require/require.rs` (clippy `module_inception`) and no `require_error.rs`. The new error sits in `error.rs` beside its siblings |
| `strong-types.md` | `register`/`answer` take `&RootTable`, not `&str`. Dispositions stay an enum. `surface` is the existing `LanguageSurface` enum, not a `bool` |
| `static-dispatch.md` | No trait objects. Closures are concrete |
| `unit-test-mandate.md` | Each of `disposition.rs`, `root.rs` and `loader.rs` keeps an inline `#[cfg(test)] mod tests`. `mod.rs` is exempt (export-only) |
| `doc-comment-discipline.md` | Every new or moved file opens with the project's module-doc shape (what, why, `Responsibilities:`, `Non-responsibilities:`). No plan or milestone vocabulary in source |
| `strict-quality.md` | `cargo make dod` is the gate. Test modules keep `#![expect(clippy::unwrap_used, reason = …)]` |
| `microsoft-guidelines.md` | `M-ERRORS-CANONICAL-STRUCTS`: the new error is a struct variant of the canonical `Error`. `M-CONCISE-NAMES`: role file names, no `*Manager`/`*Helper`. `M-AVOID-STATICS`: the root lives in the Lua registry of its own state, not in a Rust static |
| `workspace.md` | Not applicable: no manifest, member or dependency changes |

### 7.4 How logic is tested

Pure logic (disposition, name matching) is unit-tested in its own file. Lua-facing behaviour is
tested against a real `mlua::Lua` in `root.rs` and `loader.rs`, and end to end through `Engine` in
`engine.rs`, the existing pattern (`engine.rs:700-890`). LuaLS is not in the gate: its behaviour is
the probe record. The gate pins the rendered bytes.

## 8. Tests

Written first by `execute`.

**`require/root.rs`**
- `register_keeps_the_engine_s_table_in_the_registry`
- `register_seeds_package_loaded_on_the_full_surface_only`: on `Full`, `package.loaded[name]` is the
  table. On `Restricted` and `Minimal`, `register` succeeds and there is no `package`.
- `register_overwrites_a_package_loaded_entry_of_the_same_name`: root `utf8` (D6).
- `answer_matches_the_exact_root_name_only`: `airsstack` matches. `Airsstack`, `airsstack.fs`,
  ` airsstack`, `airsstack ` and `airsstac` do not.
- `answer_returns_the_registry_table_after_the_global_is_replaced`

**`require/loader.rs`**
- `the_refusing_require_ignores_its_argument` is renamed
  `the_refusing_require_refuses_every_name_but_the_root`. It keeps `require()`, `require({})` and
  `require('x')` refused with today's text, adds `require(1)` refused, and `require('airsstack')`
  returns the root.
- `a_confined_require_answers_the_root_before_looking_for_a_file`: `airsstack.lua` and
  `airsstack/init.lua` exist and each sets a global flag when run. The result is the root, and
  neither flag is set.
- `the_root_only_require_answers_the_root_and_refuses_other_names_for_want_of_a_directory`: `lib`
  and `1` give `RequireNoDirectory` with the exact §5 text.
- `the_root_only_require_rejects_an_invalid_target_as_the_confined_one_does`: `../x`.
- `the_root_only_require_rejects_a_non_string_as_the_confined_one_does`: `require()`,
  `require(nil)` and `require({})` give the same message under `RootOnly` as under `Confined`.
- `a_root_require_returns_exactly_one_value`: `select('#', require('airsstack')) == 1` under
  `Confined`, `Refused` and `RootOnly`.
- `a_root_require_ignores_arguments_after_the_first`: `require('airsstack', 'x')` is the root under
  `Confined`, `Refused` and `RootOnly`.
- The existing loader tests move unchanged.

**`require/disposition.rs`**
- `a_script_with_no_root_has_nothing_to_resolve_against_and_gets_no_require` is renamed
  `a_granted_script_with_no_directory_gets_a_require_that_answers_only_the_root` and asserts
  `RootOnly`.
- The other `decide` tests move unchanged.

**`engine.rs`**
- `a_script_from_source_has_no_require_at_all` is replaced by
  `a_script_from_source_requires_the_root_and_nothing_else`.
- `require_returns_the_root_table_under_every_preset`: `trusted`, `confined`, `pure`, and `pure` +
  `require`, each from a file and from source. `rawequal(require(root), <root>)`.
- `a_custom_root_is_required_by_its_own_name_and_the_default_name_is_ordinary`: root
  `agentstacks`. Under `confined`, `require('airsstack')` gives `RequireNotFound`.
- `a_user_file_named_after_the_root_is_never_loaded_by_require`: `trusted` (with `package.path` on
  the folder), `confined`, and `pure` + `require`.
- `require_returns_the_engine_s_table_after_the_global_is_replaced`: under `trusted` and
  `confined`.
- `a_trusted_script_that_clears_package_loaded_gets_lua_s_own_lookup` (D3).
- `a_trusted_script_that_replaces_package_loaded_gets_its_replacement` (D3).
- `a_trusted_require_of_the_root_ignores_a_second_argument`: one value, the root.
- `a_dispatched_handler_requires_the_root`: a handler registered with `ext.on` returns
  `require(root)`, and `Engine::dispatch` yields the root.
- `a_pure_policy_gives_a_refusing_require_to_a_script_on_disk` stays and still passes: it
  requires `lib`.

**`extension/loaded.rs`**: `an_extension_requires_its_host_s_root_by_name`. An extension under a
host root `myapp` gets the root from `require("myapp")`.

**`error.rs`**: `require_no_directory_names_the_module` (exact §5 text).

**`luals/render.rs`**
- New: `the_type_file_ends_by_returning_the_root`. The default root ends with
  `"\n\nreturn airsstack\n"`. Root `myapp` ends with `"\n\nreturn myapp\n"`.
- Changed: `omits_empty_docs` (`render.rs:362-374`) asserts a tail ending
  `function airsstack.m.f(x) end\n`. Its expected tail gains `\nreturn airsstack\n` after that
  line, so it still pins the whole end of the file. Its other assertion (`!text.contains('#')`)
  is unchanged.

**`luals/setup.rs`**: its tests compare with `TypeFile::render` (`setup.rs:507`) and need no edit.

## 9. Documentation and examples

Every Lua snippet, transcript and output below is run against the checkout and pasted, as
`CLAUDE.md` requires for the book.

| File | Change |
|---|---|
| `book/src/ch02-00-first-project.md` | After the `require` section (`:120-170`), a short section: `local airsstack = require("airsstack")` works under every preset and returns the same modules as the global. A file named `airsstack.lua` beside the script is never loaded by that name. One example. The summary list (`:241`) gains one line |
| `book/src/ch03-05-presets.md` | `:54` and the table row `:71`: `pure`'s `require` refuses every call except `require("airsstack")` until the member is granted |
| `book/src/appendix-03-presets.md` | Row `:17`, same change |
| `book/src/appendix-06-surfaces.md` | `:77-79`, same change |
| `book/src/ch06-02-scripts-and-values.md` | `:50-56`: a script from source has a `require` that answers only `require("airsstack")`. Any other name fails with the no-directory message, quoted. `.with_root(dir)` gives it a folder |
| `book/src/ch01-04-editor-support.md` | A `lua-language-server --check` transcript showing a misspelled field reported on a local bound with `local airsstack = require("airsstack")`, as it is on the global (`luals-require.out:24-26`). One sentence of prose says completion follows from the same type. No completion transcript: `--check` cannot produce one |
| `book/src/ch07-00-your-own-module.md` | `:59`: with the root renamed, scripts also write `require("myapp")` |
| `book/src/ch08-03-hosting.md` | Row `:74`: the root name is also what `require` returns |
| `crates/airsl/src/script.rs` | `:32`, the public rustdoc of `Script::from_source`: "no root, so `require` is unavailable" becomes: its `require` answers only the root table's name. Use `Script::from_file` or `with_root` to load siblings |
| `crates/airsl/src/engine.rs` | `:274-278`, the doc of `set_require`: "gets no `require` at all" becomes a `require` that answers only the root table. A refusing `require` still answers the root |
| `crates/airsl/examples/multi-file-project/main.rs` | Module doc `:8-9` (`pure`'s `require` refuses every call except the root name) and `:13-14` (the four-way answer: refusing, confined, Lua's own, root-only). The comment at `:52-56`. `FROM_MEMORY` (`:24-34`) and its comment: the row reports that `require("airsstack")` is the global and quotes `require("lib")`'s no-directory error. The comment at `:77-78` is rewritten |
| `…/multi-file-project/which_require.lua` | `:22-30`: `flavour`'s doc lists the four globals as refusing, confined, Lua's own and root-only. Its `require == nil` branch is removed, because no disposition leaves the global `nil`. The line at `:70-77` gains `root=yes|no`, yes when `require("airsstack")` is the global table |
| `…/multi-file-project/README.md` | The output block is pasted from a run. The prose at `:5-13`, `:78-101` (four values; `RootOnly` replaces "the global is cleared"; the `memory` row; the `pure` row still refuses `../secrets` before looking at it, and answers only the root) and the `src/require_loader.rs` paths at `:82`, `:120` and `:154` become `src/require/disposition.rs`, `src/require/loader.rs` and `src/require/loader.rs` |
| `…/policy-presets/README.md` | `:64` path becomes `src/require/disposition.rs`. `:66-71`: `pure`'s `require` refuses every call except `require("airsstack")`, and a source script gets a `require` that answers only the root |
| `…/engine-reuse/README.md` | `:78-79`: a confined script from source gets a `require` that answers only the root. `:81` path becomes `src/require/loader.rs` |
| `crates/airsl/src/extension/manifest.rs` | `:298` comment: `require_loader::resolve` becomes `require::loader::resolve` |
| `CLAUDE.md` | `:93` `require_loader` becomes `require::loader`. Key seams gains one line: `require("<root>")` is answered by `require::root` before any other resolution, and through `package.loaded` on `Full` |

The inventory came from `git grep -n require_loader -- ':!.claudestacks'` plus a read of every
passage that states what a script from source or a refusing `require` does. After the change,
`git grep -n require_loader -- ':!.claudestacks'` prints nothing.

No CHANGELOG entry and no version bump: the release waits for the whole roadmap.

## 10. Risks

- **Scripts that relied on today's behaviour.** A project with its own `airsstack.lua` (or a host
  root's file) loses `require` access to it by that name (§2 rows `:14,21,28` against §3). Under
  `pure`, `require("<root>")` stops being refused. From source, `type(require)` goes from `"nil"`
  to `"function"` (§2 rows `:39,46`). The book records all three. The release notes cover them
  when the roadmap ships.
- **LuaLS facts are pinned to 3.19.2** (`luals-require.out:1`). The gate does not run LuaLS.
- **The split moves 843 lines** (`wc -l crates/airsl/src/require_loader.rs`). The moved tests are
  the regression net, and they move unchanged.

## 11. Non-goals

- Removing or deprecating the global root table.
- `require("<root>.<module>")` returning a module's subtable (D7).
- Wrapping or changing Lua's own `require` under `Full` (D1).
- Rejecting more root names, such as `utf8` (D6).
- The native module, LuaRocks and `airsl lua setup --native` (milestones 03–05).
- Typing a project's own Lua files or modules loaded through `require`.
- A release, CHANGELOG entry or version bump.
