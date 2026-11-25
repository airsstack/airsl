---
status: superseded
created: 2026-11-25
built-on: a43e1f666b4b
---

# Spec: LuaLS type file and `airsl lua setup`

Every `HostModule` describes its own Lua surface. airsl renders those descriptions as a LuaLS
type file for any root name and checks that each description matches what the module really
installs. `airsl lua setup` (and the same call in the library, for hosts) writes the type file and
a luarc file into a project. LuaLS then stops reporting the root table as an undefined global, and
every module autocompletes.

Every probe this spec cites is in `probes/`, and `probes/README.md` says how to re-run each one.
Versions: LuaLS 3.19.2, mlua 0.12.0, serde_json 1.0.151 (`Cargo.lock`), and rustc 1.94.1, the
toolchain `rust-toolchain.toml` pins (`channel = "1.94"`). Every Rust probe record states its
toolchain on its first line. `probes/rerun.sh` re-records them all on that toolchain, against
checkout `bb95c10`, which changes no code since `577e401`. LuaLS results come from
`probes/luals/run.sh` and are cited as `run.out <case>`, the case's block in
`probes/luals/run.out`.

This spec replaces `spec-superseded-2026-11-25.md`. Changes from it:

- D16 and §3.4: the type of every table a built-in returns.
- D17 and §10: the platform scope of the symlink and special-file tests.
- §3.1: description readers are `pub(crate)`.
- §3.3: how a signature message names a record field.
- §5: records are not checked against the installed table.
- §7: the stream shape of `lua.rs`, and a failed report write.
- §10: new tests, and the acceptance check reads every record field.
- §13: the Windows risk.
- Probes are re-recorded on the pinned toolchain.

## 1. Decisions settled with the author

| # | Decision | Answer |
|---|---|---|
| D1 | Which modules are typed | The 12 built-ins and any module a host installs, under any root name |
| D2 | Drift | `cargo make dod` fails when a module's installed keys and its description disagree |
| D3 | Existing luarc | Never lose a setting the user wrote. Add only what airsl needs. A re-run changes nothing when nothing changed |
| D4 | Source of descriptions | Structured Rust data returned by a method on `HostModule` |
| D5 | Trait shape | `describe` is a **required** method. This is a breaking change for implementors |
| D6 | Type file location | `<project>/.airsl/types/<root>.lua` |
| D7 | Where setup lives | In the library (`airsl::luals::setup`). The CLI is a thin caller |
| D8 | Version in the type file | None. The file changes only when the described surface changes |
| D9 | Architecture | §2, approved by the author on its own |
| D10 | Setup flow and luarc rules | §6. A rewrite sorts keys and reformats, and any refusal writes nothing |
| D11 | Description API, type spellings, messages, docs scope | §3–§5, §8, §11 |
| D12 | A luarc or type file that is a symlink | Write through the link. The link survives, and a dangling link is refused |
| D13 | A UTF-8 BOM at the start of a luarc | Parse after it, and write it back on a rewrite |
| D14 | A required parameter after an optional one | Allowed |
| D15 | One setting written both dotted and nested | Refused only when the same key path is set both ways and a rewrite is needed (§6.1 L9) |
| D16 | Types of the built-ins' table returns | Records `fs.Stat`, `regex.Compiled`, `proc.Output`, `ext.Granted`, `ext.GrantedFs`, `ext.GrantedProc`, `ext.GrantedLimits`. Maps for `env.all`, `regex.captures` and the name→`true` sets (§3.4) |
| D17 | Platforms for the symlink and special-file tests | Unix only (`#[cfg(unix)]`). The special file is a Unix socket made with std's `UnixListener::bind`, not a FIFO (§10) |

## 2. Architecture (approved by the author)

```
crates/airsl/src/
├── types/lua_name.rs        NEW  LuaName: a Lua identifier that is not a reserved word
├── description/             NEW  the vocabulary a HostModule describes its Lua surface with
│   ├── mod.rs                    export-only
│   ├── module.rs                 ModuleDescription, ModuleDescriptionBuilder; build() validates
│   ├── signature.rs              Function, Signature, Param, Presence
│   ├── record.rs                 Record, Field
│   └── lua_type.rs               LuaType and its LuaLS spelling
├── luals/                   NEW  turns described modules into a LuaLS project setup
│   ├── mod.rs                    export-only
│   ├── verify.rs                 install one module in a scratch state; compare with describe(); Drift
│   ├── render.rs                 TypeFile: ModuleSet + RootTable → LuaLS text (pure)
│   ├── luarc.rs                  LuarcPlan: existing luarc bytes (or none) → Unchanged | Write(bytes) | refusal (pure)
│   └── setup.rs                  setup(dir, &ModuleSet, &RootTable) → SetupReport (the only I/O)
├── modules/registry.rs           HostModule gains required describe(&self) -> Result<ModuleDescription>
├── modules/{env,ext,fs,glob,hash,hook,json,path,proc,regex,stdio,time}.rs   each implements describe()
├── types/root_table.rs           RESERVED_WORDS moves to lua_name.rs; RootTable imports it
├── error.rs                      + InvalidDescription, DescribeFailed, DescriptionDrift, LuarcRefused, TypeFileRefused
└── lib.rs                        + pub mod description; pub mod luals; pub use types::LuaName
crates/airsl-cli/src/
├── cli.rs                        + Command::Lua(LuaCommand), LuaCommand::Setup { dir }
├── lua.rs                   NEW  runs `lua setup`
└── main.rs                       + mod lua; dispatch
```

Public surface. `description/mod.rs` re-exports `ModuleDescription`, `ModuleDescriptionBuilder`,
`Function`, `Signature`, `Param`, `Presence`, `Record`, `Field` and `LuaType`. `luals/mod.rs`
re-exports `verify`, `Drift`, `TypeFile`, `setup`, `SetupReport` and `FileChange`. `luarc.rs` stays
`pub(crate)`: hosts reach it only through `setup`.

Why each new module exists:

- `types/lua_name.rs`: parameter, function, field and record names become Lua source. A name that
  is not an identifier, or is a reserved word, produces a type file that does not parse. The other
  name types already live in `types/`.
- `description/`: what a module *says* it installs, independent of LuaLS. It is split by role
  because a module description, a callable signature, a table shape and a type expression are read
  separately and each has its own rules.
- `luals/`: everything specific to the Lua language server. `verify` is separate from `render`
  because a host calls it alone in its own tests. `luarc` is separate from `setup` so that every
  merge rule is tested from bytes with no filesystem.
- `airsl-cli/src/lua.rs`: the `lua` command group. It is named `lua.rs`, not `lua_setup.rs`,
  because the project allows no `<concept>_<qualifier>` files.

Dependency direction (no cycles):

```
airsl-cli::lua ─▶ luals ─▶ modules::registry (HostModule, ModuleSet, InstallContext)
                   │   └─▶ sandbox (Policy::trusted, in verify)
                   ├─────▶ description ─▶ types (LuaName), error
                   └─────▶ types (RootTable)
modules::registry ─▶ description
```

Rust guideline rules applied (`claudestacks-guideline-rust` 0.1.5, all references read):

| Rule | Application |
|---|---|
| `mod-rs-export-only.md` | `description/mod.rs` and `luals/mod.rs` hold only `//!` docs, `mod` and `pub use` with `#[doc(inline)]` |
| `modularity.md` | One concept per file. Record and Field share a file, as do Function, Signature and Param. No `*Manager` or `*Util` |
| `static-dispatch.md` | No new `dyn`. `verify` takes `&dyn HostModule` only because `ModuleSet` already stores `Box<dyn HostModule>` (the extension seam, whose `// dyn:` comment is in `modules/registry.rs`) |
| `strong-types.md` | `LuaName` is parsed once. `Presence::{Required, Optional}` replaces a bool. `FileChange::{Created, Updated, Unchanged}`. `build()` is the parse step from a builder's raw text to a validated `ModuleDescription`. `Drift` holds structured lists |
| `unit-test-mandate.md` | Every new `.rs` has inline tests except the two `mod.rs` files (exemption 1) |
| `doc-comment-discipline.md` | Module docs follow the existing what / why / Responsibilities / Non-responsibilities shape. No planning vocabulary appears in source |
| `microsoft-guidelines.md` | M-MOCKABLE-SYSCALLS: I/O stays in `setup.rs`. M-ERRORS-CANONICAL-STRUCTS: every refusal is a variant with fields, and drift is a struct. M-PUBLIC-DEBUG: every new public type derives `Debug` |
| `workspace.md` | No new crate and no new dependency. `serde_json` and `tempfile` are already `airsl` dependencies (`crates/airsl/Cargo.toml:27`, `:36`) |

Errors go in `crates/airsl/src/error.rs`. That file holds the crate's single public `Error` enum,
and every existing airsl error is a variant of it. This is a **deliberate exception** to the
author's "errors live with their context" rule. Splitting the enum would break the public
`airsl::Error` / `airsl::Result` contract every host matches on, and that is out of scope here.

How the logic is tested:

- `render.rs` and `luarc.rs` are pure and tested from strings and bytes.
- `verify.rs` is tested with small stub modules.
- `setup.rs` is tested with tempdirs.
- Each built-in module file gets one test that runs `verify` on it. That test is the drift gate
  (§10).

## 3. The description model

### 3.1 API

```rust
pub trait HostModule: Send + Sync {
    fn name(&self) -> &ModuleName;
    fn install(&self, lua: &mlua::Lua, table: &mlua::Table, context: &InstallContext<'_>) -> Result<()>;
    /// Describes the keys `install` sets, for editors and for the drift check.
    ///
    /// # Errors
    /// Returns what `ModuleDescriptionBuilder::build` returns.
    fn describe(&self) -> Result<ModuleDescription>;   // NEW, required
}
```

```rust
ModuleDescription::builder(doc: &str) -> ModuleDescriptionBuilder
ModuleDescriptionBuilder::function(self, Function) -> Self
ModuleDescriptionBuilder::field(self, name: &str, ty: LuaType, doc: &str) -> Self   // a non-function value
ModuleDescriptionBuilder::record(self, Record) -> Self
ModuleDescriptionBuilder::build(self) -> Result<ModuleDescription>

Function::new(name: &str, doc: &str) -> Function
Function::param(self, name: &str, ty: LuaType, doc: &str) -> Self      // Presence::Required
Function::optional(self, name: &str, ty: LuaType, doc: &str) -> Self   // Presence::Optional
Function::variadic(self, ty: LuaType, doc: &str) -> Self               // trailing `...`
Function::returns(self, ty: LuaType) -> Self                           // repeatable: multiple returns

Signature::new() -> Signature            // a function *type*, e.g. a handler argument
Signature::param(self, name: &str, ty: LuaType) / optional(..) / variadic(ty) / returns(ty)

Record::new(name: &str, doc: &str) -> Record
Record::field(self, name: &str, ty: LuaType, doc: &str) -> Self

LuaType::{Nil, Boolean, Integer, Number, String, Any, Table}
LuaType::literal(text) / array(t) / map(k, v) / optional(t) / function(Signature) / record(name)
LuaType::or(self, other) -> LuaType       // builds a union; a union is never empty
```

The builder pieces hold raw text. `build()` is the single validation point (§3.3), and only a
validated `ModuleDescription` reaches `luals`. `LuaType::optional` is idempotent: the optional of
an optional is the same optional. A `Signature` carries no docs; it renders only as a `fun(...)` type.
The parts a description holds (names, docs, parameters, returns, fields, records, types) are read
by `luals` through `pub(crate)` accessors. Hosts build descriptions but never read them back, so
no reader is public.

Example: `hash.hash_file`. Its live surface is
`hash (4) hash_file:function hex:function sha1:function sha256:function`
(`probes/probe-surface.txt`).

```rust
fn describe(&self) -> Result<ModuleDescription> {
    ModuleDescription::builder("SHA-256 and SHA-1 digests.")
        .function(
            Function::new("hash_file", "Hashes a file's bytes.")
                .param("path", LuaType::String, "File to hash.")
                .optional("algorithm", LuaType::literal("sha256").or(LuaType::literal("sha1")), "Defaults to sha256.")
                .returns(LuaType::String),
        )
        // … sha256, sha1, hex
        .build()
}
```

`ext.on`'s handler is described as `LuaType::function(Signature::new().param("payload",
LuaType::Any).returns(LuaType::Any))`, which spells `fun(payload: any): any`. A `fun(...)` with no
returns makes LuaLS warn on any callback that returns a value (`run.out spell`:
`use.lua:4:57 … at most 0 return value(s) … (redundant-return-value)`). So a callback that may
return a value is described with a return type.

### 3.2 LuaType spelling

Sources: `run.out grammar`, `run.out compose` and `run.out spell`. In each case, the good-usage
file gets no diagnostic.

| LuaType | Spelling | Probed in |
|---|---|---|
| `Nil` `Boolean` `Integer` `Number` `String` `Any` `Table` | `nil` `boolean` `integer` `number` `string` `any` `table` | grammar, compose |
| `literal("sha1")` | `"sha1"` | grammar (`hash_file` algorithm) |
| `a.or(b)` | `a\|b`, where each member that is a function is written `(fun…)` | compose (`string\|integer`), spell (`(fun(x: string): boolean)\|string`) |
| `array(t)` | `t[]`, or `(t)[]` when `t` is a union, an optional or a function | compose `(string\|integer)[]`, spell `(string?)[]` and `(fun(x: string): boolean)[]` |
| `map(k, v)` | `table<k, v>` | grammar `table<string, string>`, compose `table<integer\|string, string>?` |
| `optional(t)` | `t?`, or `(t)?` when `t` is a union or a function | compose `(string\|integer)?` and `(fun(x: string): boolean)?`, spell `string[]?` |
| `function(sig)` | `fun(a: T, b?: U, ...: V): R1, R2`, with no `: …` when there are no returns | compose (variadic, multiple returns), spell `fun(a: string, b?: integer)` |
| `record("Stat")` | `<root>.<module>.Stat` | grammar `airsstack.fs.Stat`, `airsstack.regex.Compiled` |

Why a function member of a union is parenthesised: LuaLS reads the bare
`fun(x: string): boolean|string` as a function returning `boolean|string`. `run.out spell`:
`use.lua:3:25 [Warning] Cannot assign `string` to parameter `fun(x: string):boolean|string`.`
The parenthesised form accepts both a string and a function with no diagnostic.

Limits of the check, from the same runs: an omitted argument to a required `any` parameter is not
reported. Calling a method on an unguarded `string?` return is also not reported at
`--checklevel=Information`. `grammar-bad.lua` has 7 misuse lines, and LuaLS reports 5
(`run.out grammar`).

The built-ins need no other shape. They expose 66 keys, all functions (`probes/probe-surface.txt`).
The only variadic is in `path.rs`, and no module defines userdata (`probes/surface-greps.txt`).
`probes/surface-inventory.md` gives each key's arguments and returns with `file:line`, read from
the module sources. §3.4 fixes the type of every table a built-in returns.

### 3.3 Validation in `build()`

`build()` walks the description in this fixed order and returns the **first** problem it finds:

1. The module doc.
2. Each member in the order it was added. A function's order is: its name, its doc, then each
   parameter's name, doc and type, then its variadic tail, then its return types. A field's order
   is: its name, its doc, then its type. Types are walked depth-first, left to right.
3. Each record in the order added: its name, its doc, then each field's name, doc and type.
4. Cross-checks: member versus record name clashes, then record references.

Names go through `LuaName::new`, which returns the existing `Error::InvalidName` with
`kind: "Lua name"`. Its display is `invalid {kind} `{value}`: {reason}`
(`crates/airsl/src/error.rs:196-197`).

| Input | Message |
|---|---|
| `""` | ``invalid Lua name ``: must not be empty`` |
| `"9x"` | ``invalid Lua name `9x`: must start with an ASCII letter or underscore`` |
| `"a-b"` | ``invalid Lua name `a-b`: must contain only ASCII letters, digits and underscores`` |
| `"end"` | ``invalid Lua name `end`: must not be a Lua reserved word`` |
| `"End"`, `"_x"`, `"hookEventName"` | accepted. The reserved words are the 22 lowercase words in `crates/airsl/src/types/root_table.rs:24`, which move to `lua_name.rs` |

Every other problem is `Error::InvalidDescription { reason }`, displayed as
`invalid module description: {reason}`. `{owner}` is one of:

- `the module`
- ``function `f` ``
- ``parameter `p` of `f` ``
- ``the variadic parameter of `f` ``
- ``field `x` ``
- ``record `R` ``
- ``field `x` of record `R` ``

The three parameter rows below name their function as `{fn}`:

- For a module function, `{fn}` is the function's name in backticks, e.g. `` `run` ``. This
  includes a `Signature` nested anywhere in that function's parameter or return types.
- For a `Signature` nested in a module field's type, it is the field's name in backticks.
- For a `Signature` nested in a record field's type, it is ``field `x` of record `R` ``. For
  example: ``parameter `text` of field `find` of record `Compiled` is described twice``.

| Problem | `reason` |
|---|---|
| two members (functions or fields) share a name | ``{name}` is described twice`` |
| a record shares a name with a member | ``{name}` is described both as a member and as a record`` |
| two parameters of one function or signature share a name | ``parameter `{p}` of {fn} is described twice`` |
| `variadic` called twice on one function or signature | ``{fn} has two variadic parameters`` |
| `param` or `optional` called after `variadic` | ``parameter `{p}` of {fn} comes after the variadic parameter`` |
| two records share a name | ``record `{R}` is described twice`` |
| two fields of one record share a name | ``field `{x}` of record `{R}` is described twice`` |
| `LuaType::record(n)` names no record of this module | ``record `{n}` is used but not described`` |
| any doc contains `\r` | ``the doc of {owner} must not contain a carriage return`` |
| a doc line starts with `@`, after leading spaces | ``a line in the doc of {owner} starts with `@` `` |
| a parameter, variadic or field doc contains `\n` | ``the doc of {owner} must be one line`` |
| a literal contains `"`, `\` or `\n` | ``string literal type `{text}` must not contain a double quote, a backslash or a line break`` |

Two cases are accepted:

- A required parameter after an optional one (D14). LuaLS accepts `---@param a? string` before
  `---@param b integer` and type-checks `b` (`run.out order`: only
  `use.lua:3:20 … Cannot assign `string` to parameter `integer`.`).
- An unused record. It renders as a class nothing references.

The `@` rule exists because a doc line that starts with `@` becomes a LuaLS annotation.
`run.out spell` shows `---@deprecated` in a doc block producing `use.lua:7:1 [Warning] Deprecated.`.

### 3.4 Tables the built-ins return (D16)

Every table a built-in returns has exactly one of these types. Two functions return a value
whose type depends on its input, so they return `any`. A record is declared with
`ModuleDescriptionBuilder::record` in the module that returns it, so its class is
`<root>.<module>.<Name>`.

| Returned by | Type | Fields (record) |
|---|---|---|
| `fs.stat` | record `Stat` | `size: integer`, `kind: "dir"\|"symlink"\|"file"`, `readonly: boolean`, `modified: integer` |
| `regex.compile` | record `Compiled` | `is_match: fun(text: string): boolean`, `find: fun(text: string): string?`, `find_all: fun(text: string): string[]`, `captures: fun(text: string): table<integer\|string, string>?`, `replace: fun(text: string, with: string): string`, `replace_all: fun(text: string, with: string): string`, `split: fun(text: string): string[]` |
| `proc.run` | record `Output` | `stdout: string`, `stderr: string`, `status: integer` |
| `ext.granted` | record `Granted` | `fs: GrantedFs`, `env: table<string, boolean>`, `proc: GrantedProc`, `limits: GrantedLimits`, `unrestricted: boolean` |
| (field of `Granted`) | record `GrantedFs` | `read: string[]`, `write: string[]` |
| (field of `Granted`) | record `GrantedProc` | `run: table<string, boolean>` |
| (field of `Granted`) | record `GrantedLimits` | `memory: integer?`, `instructions: integer?` |
| `regex.captures` | `table<integer\|string, string>?` | — a map, because its keys are group numbers and group names |
| `env.all` | `table<string, string>` | — |
| `fs.read_lines`, `fs.list`, `fs.walk`, `regex.find_all`, `regex.split`, `glob.walk`, `stdio.lines` | `string[]` | — |
| `json.decode`, `hook.payload` | `any` | — any decoded JSON value: a table, number, string, boolean or JSON null |

Field names written as `GrantedFs` stand for `LuaType::record("GrantedFs")`, which renders as
`<root>.ext.GrantedFs`.

Evidence:

- The live shapes come from airsl itself (`probes/probe-shapes.txt`, `probes/shapes.lua` run
  with `airsl run --policy confined` plus grants and both limits):
  - `proc.run`: `[string status] = integer 0`, plus `stderr` and `stdout` strings.
  - `regex.captures`: keys `[integer 0]`, `[integer 1]`, `[integer 2]` and `[string n]`, all
    strings. An unmatched optional group is absent: `(c)?` adds no key.
  - `ext.granted`: `env` maps `HOME` to `boolean true`. `fs.read` and `fs.write` are sequences of
    strings. `limits` holds `instructions` and `memory` as integers. `proc.run` maps `echo` to
    `boolean true`. `unrestricted` is a boolean.
  - Under `--policy trusted`, `limits` is an empty table (same file, second run), so both of its
    fields are optional.
  - `json.decode('1')` is `integer 1`, `json.decode('"s"')` is `string s`, `json.decode('null')`
    is `userdata userdata: 0x0`, and `hook.payload()` with stdin `"x"` is `string x`. Neither
    always returns a table, so both are `any`. On blank stdin, `hook.payload` returns an empty
    table (`hook.rs:89-93`).
- `fs.stat`'s `kind` values and the `Compiled` closures come from the sources:
  `surface-inventory.md` (`fs.rs:258`, fields at 237–254; `regex.rs:84`–`135`).
- LuaLS accepts these spellings and checks through them. `run.out records` renders, in the §4
  layout, every row above except `fs.stat` and the `string[]` and `env.all` returns: `Output`,
  all seven `Compiled` fields, `regex.captures`, `json.decode`, `hook.payload` and the four
  `Granted` records. `Stat` is covered by `run.out grammar` (`grammar-typefile.lua:31-35`), and
  `string[]` and `table<string, string>` returns by `run.out grammar` too. `records-good.lua`
  calls every `Compiled` field and reads every field of the other records, including `caps[0]`,
  `caps.n`, `g.fs.read[1]`, `g.env.HOME`, `g.proc.run.echo` and `g.limits.memory + 1`. It also
  uses an `any` result as a table, a number and a string. It gets no diagnostic. `bad.lua` gets all 6 of its expected warnings:
  `Undefined field `nope`` on `Output`, `GrantedFs`, `GrantedLimits`, `GrantedProc` and
  `Compiled`, and `Undefined field `upper`` on `status`.
- The fixture is valid Lua: `probes/luals/check-typefiles.out`, third run →
  `1 file(s) compiled, 0 failed`.

## 4. Rendering (`luals::render`)

```rust
TypeFile::render(modules: &ModuleSet, root: &RootTable) -> Result<TypeFile>
TypeFile::file_name(&self) -> String      // "<root>.lua"
TypeFile::as_str(&self) -> &str
```

`render` calls `describe()` on each module, in `ModuleSet` insertion order. A failure becomes
`Error::DescribeFailed { module, source }` (§8). The output is a pure function of the
descriptions and the root name. It contains no version, date or path, so it is byte-stable (D8).

The exact layout:

- Blocks are separated by one empty line, and the file ends with one `\n`.
- A multi-line doc renders one `---<line>` per `\n`-separated line. An empty doc renders nothing.
- A parameter, variadic or field doc is appended after one space, only when it is not empty.

```
---@meta
-- Generated by airsl. Do not edit; regenerate it instead.

---Host modules installed under `<root>`.
---@class <root>
<root> = {}

---<module doc>
---@class <root>.<m>
---@field <field> <type> <doc>              (one per described field, in order)
<root>.<m> = {}

---<record doc>                             (one block per record, in order)
---@class <root>.<m>.<Record>
---@field <field> <type> <doc>

---<function doc>                           (one block per function, in order)
---@param <name> <type> <doc>               (`<name>?` when Presence::Optional)
---@param ... <type> <doc>                  (variadic)
---@return <type>                           (one per return)
function <root>.<m>.<f>(<p1>, <p2>, ...) end
```

Some module names are Lua reserved words. `ModuleName::new` has no reserved-word check
(`crates/airsl/src/types/module_name.rs:34-57`); only `RootTable` refuses them
(`root_table.rs:105`). Such a module renders as `<root>["<m>"] = {}`, and its functions as
`<root>["<m>"].<f> = function(<params>) end`.

Evidence for this layout:

- `run.out layout` uses this exact layout, including a module field and a reserved-word module.
  `airsstack.info.version:upper()` and `airsstack["end"].hello("me"):upper()` get no diagnostic,
  while `airsstack.info.nope` and `airsstack["end"].nope` each get `Undefined field `nope``.
- Both layout fixtures are valid Lua: `probes/luals/check-typefiles.out` records
  `cargo run -q -p airsl-cli -- check …` on the checkout → `1 file(s) compiled, 0 failed`, twice.
- The default state of a project works: `.airsl/` is not gitignored, and the luarc lists the
  library entry. `run.out default_state` shows only the deliberate `Undefined field `nope``.

## 5. Drift check (`luals::verify`)

```rust
pub fn verify(module: &dyn HostModule, root: &RootTable) -> Result<()>
pub struct Drift { /* undescribed, missing, not_a_function, not_a_field; accessors */ }
```

1. Call `module.describe()`. A failure becomes `Error::DescribeFailed`.
2. Create a fresh `mlua::Lua::new()` and a fresh table. Call
   `module.install(&lua, &table, &InstallContext::new(&Policy::trusted(), root))`. A failure is
   returned unchanged. No limits are armed and the table is not set as a global; `verify` only
   needs what `install` puts in the table. The built-in surface is identical under all three
   presets (`probes/probe-surface.txt`: the `== trusted`, `== confined` and `== pure` blocks match).
3. Compare:
   - **Undescribed**: every key from a raw walk of the table (`Table::pairs`) that is not a
     described name. `Table::pairs` ignores `__pairs` (`probes/probe-more.txt`:
     `Table::pairs keys: ["real"]`, while Lua's `pairs()` yields `fake`).
   - "Described names" are the module's members: its functions and fields. Records are types and
     are never installed, so a record name is neither expected on the table nor excused from it.
     `fs`, `regex`, `proc` and `ext` describe records (§3.4) and still pass.
   - **Missing**: every described name whose `Table::get` is `nil`. `Table::get` honours
     `__index` (`probe-more.txt`: `Table::get(hidden) = "integer", raw_get(hidden) = "nil"`), so a
     module that serves members through a metatable is not reported missing.
   - **Not a function**: a described function whose value is not a function.
   - **Not a field**: a described field whose value is a function.

Any non-empty list returns `Error::DescriptionDrift { module, drift }`, displayed as
`host module `{module}` does not match its description: {drift}`. `Drift`'s `Display` joins
parts with `; ` in the order below and leaves out empty groups. Names are sorted by their
displayed text (byte order) and backticked:

| Group | Text |
|---|---|
| undescribed | `installs {names} without describing them` (one name: `without describing it`), names joined with `, ` |
| missing | `describes {names} but does not install them` (one name: `install it`), names joined with `, ` |
| not a function | one part per name: ``describes `{n}` as a function but installs a value of type `{t}` `` |
| not a field | one part per name: ``describes `{n}` as a field but installs a function`` |

Displayed key text:

- A string key shows as itself.
- An integer or float key shows through `mlua::Value::to_string`.
- Any other key shows as `<{type}>`, for example `<table>`. Its `to_string` is an address
  (`probe-more.txt`: `table: 0x…`), which would make the message change between runs.

`{t}` is `mlua::Value::type_name()` (`probes/probe-mlua-typename.txt`: `string integer number
boolean table function nil`).

## 6. Setup (`luals::setup`)

```rust
pub fn setup(project: &Path, modules: &ModuleSet, root: &RootTable) -> Result<SetupReport>
SetupReport::type_file(&self) -> (&str, FileChange)   // (".airsl/types/<root>.lua", …)
SetupReport::luarc(&self) -> (&str, FileChange)       // (".luarc.json" | ".luarc.jsonc", …)
impl Display for SetupReport                          // two lines, below
pub enum FileChange { Created, Updated, Unchanged }   // Display: created / updated / unchanged
```

**Paths in errors.** Every path in an error is `project.join(<relative path>)` shown with
`Path::display()`. The project path is shown as the caller gave it. A failure while writing a
temporary file names the target it was meant to replace, never the temporary file.

**Resolving a target** (the luarc file or the type file), done before anything is written:

1. If `std::fs::symlink_metadata(path)` fails with `NotFound`, the target is absent.
2. If the target is a symlink, resolve it with `std::fs::canonicalize`. If that fails, refuse with
   `it is a dangling symlink`.
3. If the resolved target exists and is not a regular file (directory, FIFO, socket, device),
   refuse with `it is not a regular file`. It is never opened, so a FIFO cannot block.

Probe evidence (`probes/probe-fs-special.txt`, std, rustc 1.94.1):

| Behaviour | Result |
|---|---|
| a dangling link | `exists(): false`, but `symlink_metadata ok: true`. Resolution therefore uses `symlink_metadata`, so a dangling link is never mistaken for absence |
| `canonicalize` on a dangling link | `NotFound` |
| a FIFO | `is_file: false, is_dir: false` |
| a Unix socket made by `UnixListener::bind` (`probes/probe-socket.txt`) | `is_file: false, is_dir: false, is_socket: true`. `File::open` on it fails: `Operation not supported on socket (os error 102)`. A symlink to it canonicalizes to a target with `is_file: false` |
| writing a temp file beside the resolved target, then renaming it over the target | `link still symlink: true`, and the new content reads through the link |

**Writing a target.** Write a temporary file in the resolved target's directory, then rename it
over the resolved target. A symlink is therefore written through and survives (D12).

**Flow (D10).** Steps 1–4 write nothing, and the first error ends the call.

| Step | Action | Failure |
|---|---|---|
| 1 | `std::fs::read_dir(project)` | `Io { operation: "open project directory", path: <project as given> }` |
| 2 | `verify` every module in `modules`, in order | `DescribeFailed`, the install error, or `DescriptionDrift` |
| 3 | `TypeFile::render(modules, root)` | none (step 2 already called every `describe`) |
| 4 | Resolve the type file target and the luarc target. Read the luarc bytes if present. Plan the edit (§6.1) | `TypeFileRefused` / `LuarcRefused` / `Io { operation: "read" }` |
| 5 | `create_dir_all(project/.airsl/types)`. Write the type file only if its bytes differ | `Io { operation: "create directory", path: <project>/.airsl/types }` / `Io { operation: "write" }` |
| 6 | Write the luarc only if the plan is `Write` | `Io { operation: "write" }` |

Step 1 refuses a missing project and a project that is a file. `create_dir_all` therefore never
creates the project (`probes/probe-io.txt`: `read_dir missing: … NotFound`, `read_dir file: …
NotADirectory`). If step 6 fails after step 5 succeeded, the type file is already written. A
re-run then completes the luarc and reports the type file as `unchanged`.

**Luarc selection.** An entry "exists" when `symlink_metadata` succeeds.

- If `.luarc.json` exists, use it.
- Otherwise, if `.luarc.jsonc` exists, use that.
- Otherwise, create `.luarc.json`.

LuaLS 3.19.2 loads `.luarc.json` first, and `.luarc.jsonc` only when `.luarc.json` is absent:
`local rc = cfgLoader.loadRCConfig(folder.uri, '.luarc.json') or cfgLoader.loadRCConfig(folder.uri,
'.luarc.jsonc')` (`probes/luals-provider-53-54.lua`). Observed with `.airsl/` gitignored:

- `.luarc.jsonc` alone is honoured (`run.out jsonc_only`).
- A `.luarc.json` without the entry shadows a `.luarc.jsonc` that has it: `run.out json_wins` reports
  `Undefined global`, the same as `run.out control`, which has no luarc at all.

**Why the entry matters.** Writing it is what makes a gitignored `.airsl/` work. With `.airsl/`
ignored and no luarc, LuaLS reports `Undefined global `airsstack`` (`run.out control`). With
`{"workspace.library":[".airsl/types"]}` it reports only `Undefined field `nope``
(`run.out dotted`).

**Report.** One line each, the type file first, with paths relative to the project and `/`
separators:

```
created .airsl/types/airsstack.lua
created .luarc.json
```

### 6.1 Luarc plan (`luals::luarc`, pure)

```rust
pub(crate) enum LuarcPlan { Unchanged, Write(Vec<u8>) }
pub(crate) fn plan(existing: Option<&[u8]>) -> Result<LuarcPlan, String /* refusal reason */>
```

The entry is the string `.airsl/types`. "Listed" means an array element that is exactly that
string. The rules are applied in this order, and the first that decides wins:

| # | Input | Plan | Proof |
|---|---|---|---|
| L1 | `None` (no file) | `Write` the bytes `{\n  "workspace.library": [\n    ".airsl/types"\n  ]\n}\n` | `probes/probe-more.txt`, `L1 bytes:` |
| L2 | starts with `EF BB BF` | strip it, continue, and put it back in front of any `Write` (D13) | LuaLS accepts a BOM (`run.out bom`). serde_json does not: `bom: err -> expected value at line 1 column 1`; `bom stripped: ok` (`probe-more.txt`) |
| L3 | empty or ASCII-whitespace only | treated as `{}`, continue from L5 | serde_json rejects it otherwise: `empty file: err -> EOF while parsing a value at line 1 column 0` (`probes/probe-serde.txt`) |
| L4 | `serde_json::from_slice` fails: comments, trailing comma, invalid JSON, not UTF-8 | refuse: `it is not plain JSON ({serde error})` | `comment: err -> key must be a string at line 2 column 3`, `trailing comma: err -> trailing comma at line 1 column 9` (`probe-serde.txt`), `not utf-8: err -> invalid unicode code point at line 1 column 8` (`probe-more.txt`). LuaLS does read comments (`run.out comments`), so this case occurs in practice |
| L5 | the top level is not an object | refuse: `its top level is not a JSON object` | `not object: ok -> [` parses (`probe-serde.txt`) |
| L6 | dotted `workspace.library` exists and is not an array | refuse: `` `workspace.library` is not an array`` | — |
| L7 | `workspace` exists and is not an object, or nested `workspace` → `library` exists and is not an array | refuse: `` `workspace` is not an object`` / `` `workspace` → `library` is not an array`` | — |
| L8 | every spelling of `workspace.library` that is present lists the entry, and at least one spelling is present | `Unchanged`: no write, bytes untouched | — |
| L9 | a rewrite is needed, and some key path is set in two spellings | refuse: ``it sets `{path}` in two spellings`` (the first such path in byte order) | LuaLS uses the textually last spelling: `run.out nested_first_has` and `dotted_first_has` give `Undefined global`. A rewrite sorts keys (L11) and could flip which spelling wins. Disjoint key paths are safe (`run.out disjoint`) |
| L10 | otherwise: a dotted `workspace.library` array (append) / a nested `workspace.library` array (append) / a nested `workspace` without `library` (add `"library": [".airsl/types"]`) / neither (add `"workspace.library": [".airsl/types"]`) | `Write` | dotted and nested are both honoured (`run.out dotted`, `run.out nested`) |
| L11 | the bytes of any `Write` | `serde_json::to_string_pretty` + `\n`: 2-space indent, keys sorted, no comments | `order: ok -> {` … `"a": { "b": 2, "y": 1 }, "runtime.version": …, "z": 1` (`probe-serde.txt`). `cargo tree -e features -i serde_json` shows only `default` and `std`, so there is no `preserve_order` (`probes/cargo-tree-serde-json.txt`) |
| L12 | duplicate keys | the last value is kept | serde_json gives `duplicate key: ok -> { "a": 2 }` (`probe-serde.txt`). LuaLS also keeps the last: `run.out lastwins_has` finds the types, `firstwins_has` does not. The effective setting is preserved |

**Key paths for L9.** Each top-level key is split on `.` into a path. If its value is an object,
the walk continues into it, and each nested key is split on `.` and appended. The walk stops at a
value that is not an object, or at an empty object. A key path is "set in two spellings" when
two different top-level keys reach the same path.

**When L10 appends** to a library list that holds the entry in no spelling, it appends to the one
spelling that is present. L9 has already refused the case where both spellings are present. Every
other key and value is kept as parsed. Order and formatting are not kept (D10).

**Refusals** become `Error::LuarcRefused { path, reason }`, displayed as
``will not edit `{path}`: {reason}; add ".airsl/types" to `workspace.library` by hand``.
The two file-kind reasons from §6 use the same variant when the target is the luarc. For the type
file they use `Error::TypeFileRefused { path, reason }`, displayed as
``will not write `{path}`: {reason}``.

## 7. CLI: `airsl lua setup [DIR]`

```
airsl lua setup [DIR]     DIR defaults to "."
```

The CLI adds `Command::Lua(LuaCommand)` with `LuaCommand::Setup { dir: PathBuf }`. `lua.rs` calls
`airsl::luals::setup(&dir, &airsl::modules::stdlib()?, &RootTable::default())`.

`lua.rs`'s entry point takes `stdout` and `stderr` as `&mut impl Write` and returns the exit code,
the shape `ext_fire::run` already has (`crates/airsl-cli/src/ext_fire.rs:91-98`), so its tests
assert on both streams. `main.rs` passes the real streams.

- On success it prints the report to stdout and exits 0.
- If writing the report to stdout fails, it prints `airsl: {io error}` to stderr and exits 1. Both
  files are already written by then. `ext_fire` handles a failed stdout write the same way, with
  its own prefix `airsl ext fire: ` (`ext_fire.rs:25`, `:84-87`).
- On any error it prints `airsl: {error}` to stderr and exits 1. That is the prefix `check`, `run`
  and `test` already use (`crates/airsl-cli/src/check.rs:33`, `run.rs:46`).
- There is no `--root` flag. A host with its own root calls `luals::setup` itself.

## 8. Error catalogue

New `Error` variants:

| Variant | Display |
|---|---|
| `InvalidDescription { reason: String }` | `invalid module description: {reason}` |
| `DescribeFailed { module: String, source: Box<Error> }` | ``cannot describe host module `{module}`: {source}`` |
| `DescriptionDrift { module: String, drift: Drift }` | ``host module `{module}` does not match its description: {drift}`` |
| `LuarcRefused { path: String, reason: String }` | ``will not edit `{path}`: {reason}; add ".airsl/types" to `workspace.library` by hand`` |
| `TypeFileRefused { path: String, reason: String }` | ``will not write `{path}`: {reason}`` |

Two existing variants are reused:

- `InvalidName`, with kind `"Lua name"` (§3.3).
- `Io`, with operations `"open project directory"`, `"read"`, `"create directory"` and `"write"`.
  Its doc sentence changes from "An operating-system call made on a script's behalf failed." to
  "An operating-system call failed.", because setup now makes these calls too. Its display,
  ``{operation} failed on `{path}`: {source}`` (`crates/airsl/src/error.rs:250-251`), is unchanged.

Example of the composed `Io` message, for `airsl lua setup proj` when `.airsl` is a file:
``create directory failed on `proj/.airsl/types`: Not a directory (os error 20)``. The OS text is
from `probes/probe-io.txt` (`create_dir_all under file: … "Not a directory (os error 20)"`). The
surrounding format comes from the `Io` display above; this example was composed, not run.

## 9. Edge cases

| Case | Behaviour | Owner |
|---|---|---|
| Project dir missing, or a file | `Io` "open project directory". Nothing is created | `setup.rs` |
| `.airsl` is a file | `Io` "create directory" on `<project>/.airsl/types` | `setup.rs` |
| Type file or luarc is a directory, FIFO, socket or device | `TypeFileRefused` / `LuarcRefused`: `it is not a regular file`. Never opened | `setup.rs` |
| Type file or luarc is a symlink to a regular file | Written through. The link survives (D12) | `setup.rs` |
| Type file or luarc is a dangling symlink | Refused: `it is a dangling symlink` | `setup.rs` |
| Type file exists and is identical | `Unchanged`, not written | `setup.rs` |
| Type file exists and was edited by the user | Overwritten (`Updated`). The header says not to edit it | `setup.rs` |
| Other files in `.airsl/types/` | Untouched | `setup.rs` |
| Both `.luarc.json` and `.luarc.jsonc` exist | Only `.luarc.json` is read and edited | `setup.rs` |
| Luarc has a BOM | Parsed after the BOM, and the BOM is kept on a rewrite (L2) | `luarc.rs` |
| Luarc is not UTF-8 | L4 refusal | `luarc.rs` |
| Entry spelled `./.airsl/types` | Not "listed". `.airsl/types` is appended | `luarc.rs` |
| Library array holds non-strings | Kept. The entry is appended | `luarc.rs` |
| Library set in both spellings, listed in both | `Unchanged` (L8) | `luarc.rs` |
| Library set in both spellings, listed in one or neither | Refused (L9) | `luarc.rs` |
| Disjoint settings in both spellings, e.g. dotted `workspace.library` plus `workspace: {checkThirdParty}` | Accepted, and rewritten if needed | `luarc.rs` |
| Empty module set | The type file holds only the header and root class. Setup succeeds | `render.rs` |
| A host root name, e.g. `agentstacks` | The file is `agentstacks.lua`, with classes `agentstacks.*` | `render.rs` |
| A module name that is a reserved word | Bracket form (§4) | `render.rs` |
| A module that installs no keys and describes none | Passes `verify`. Renders an empty class | `verify.rs` |
| A module that serves members through `__index` | Not reported missing. Keys visible only through `__pairs` are not walked | `verify.rs` |
| A module that installs a table- or function-keyed entry | Reported as undescribed, shown as `<table>` or `<function>` | `verify.rs` |
| `describe()` returns an error | `DescribeFailed { module, source }` from `render`, `verify` and `setup` | `luals` |
| Re-run with nothing changed | Both lines say `unchanged`, and neither file's bytes are touched | `setup.rs` |

## 10. Testing

Named tests, each in an inline `#[cfg(test)] mod tests`:

- `types/lua_name.rs`:
  - `accepts_identifiers_with_mixed_case_and_underscores`
  - `refuses_an_empty_name`
  - `refuses_a_leading_digit`
  - `refuses_a_hyphen`
  - `refuses_every_reserved_word`
  - `accepts_a_capitalised_reserved_word`
- `types/root_table.rs`: the existing tests keep passing after `RESERVED_WORDS` moves.
- `description/module.rs`:
  - one test per `InvalidDescription` row in §3.3
  - `build_reports_the_first_problem_in_declaration_order`
  - `a_required_parameter_may_follow_an_optional_one`
  - `an_unused_record_is_accepted`
  - `a_valid_description_builds`
  - `a_signature_problem_in_a_record_field_names_the_field_and_record`: a record field typed
    `fun(text: string, text: string)` gives
    ``invalid module description: parameter `text` of field `find` of record `Compiled` is described twice``
- `description/signature.rs`:
  - `param_optional_and_variadic_record_their_presence`
  - `returns_accumulate_in_order`
  - `a_signature_keeps_parameters_in_declaration_order`
- `description/record.rs`:
  - `fields_keep_declaration_order`
  - `a_record_carries_its_name_and_doc`
- `description/lua_type.rs`:
  - one test per row of §3.2
  - `optional_of_an_optional_is_one_optional`
  - `a_function_in_a_union_is_parenthesised`
  - `an_array_of_an_optional_is_parenthesised`
  - `an_optional_array_needs_no_parentheses`
- `luals/render.rs`:
  - `renders_the_header_and_root_class`
  - `renders_a_reserved_word_module_with_brackets`
  - `renders_module_fields_on_the_module_class`
  - `renders_optional_and_variadic_params`
  - `omits_empty_docs`
  - `renders_multiple_returns`
  - `renders_records_before_functions`
  - `the_stdlib_type_file_compiles_as_lua`: loads the rendered stdlib file through `Engine::check`
    (`crates/airsl/src/engine.rs:321`)
  - `rendering_twice_is_byte_identical`
  - `a_host_root_names_the_file_and_classes`
- `luals/verify.rs`:
  - `a_matching_module_passes`
  - `an_undescribed_key_is_drift`
  - `a_missing_key_is_drift`
  - `a_member_served_through_index_is_not_missing`
  - `a_field_installed_as_a_function_is_drift`
  - `a_function_installed_as_a_value_is_drift`
  - `a_table_key_is_shown_by_its_type`
  - `drift_display_orders_and_joins_parts`
  - `a_describe_error_names_the_module`
  - `a_described_record_is_not_reported_missing`
- `luals/luarc.rs`:
  - one test per row L1–L12
  - `parse_refusals_win_over_shape_refusals`
  - `a_listed_entry_in_both_spellings_is_unchanged`
  - `disjoint_spellings_are_rewritten`
  - `key_paths_split_nested_dotted_keys`
- `luals/setup.rs`, using tempdirs:
  - `first_run_creates_both_files`
  - `second_run_reports_unchanged_and_writes_nothing`: the mtimes are equal
  - `a_changed_module_updates_only_the_type_file`
  - `a_luarc_refusal_writes_nothing`
  - `a_drifting_module_writes_nothing`
  - `a_missing_project_is_refused`
  - `a_file_as_project_is_refused`
  - `prefers_luarc_json_over_jsonc`
  - `edits_luarc_jsonc_when_it_is_the_only_one`
  - `a_symlinked_luarc_is_written_through` (Unix only)
  - `a_dangling_symlink_luarc_is_refused` (Unix only)
  - `a_socket_luarc_is_refused_without_opening_it` (Unix only): binds a `UnixListener` at
    `<tempdir>/.luarc.json` and expects `LuarcRefused` with `it is not a regular file`. The
    assertion holds on every Unix. On macOS, opening the socket fails (`probe-socket.txt`:
    `os error 102`), so code that opened the target before checking its type would return `Io`
    there instead, and the test would fail. That open-before-check detection is proven on macOS
    only. It was not probed on Linux, where CI also runs the test
  - `a_directory_at_the_type_file_path_is_refused`
  - `io_errors_name_the_joined_path`

  **Platforms (D17).** The three tests marked Unix only are `#[cfg(unix)]`. They make links
  with `std::os::unix::fs::symlink` and the socket with `std::os::unix::net::UnixListener`, both
  std, with no external tool and no new dependency. All other tests run on every CI platform.
  Windows runs the same resolution code, but no probe or test covers symlinks or special files
  there.
- Each of the 12 built-in module files: `its_description_matches_what_it_installs`, which calls
  `verify`. This is the drift gate (D2), and `cargo make dod` runs it.
- `airsl-cli/src/cli.rs`:
  - `lua_setup_defaults_to_the_working_directory`
  - `lua_setup_takes_a_directory`
- `airsl-cli/src/lua.rs`:
  - `prints_the_report_and_exits_zero`
  - `prints_the_error_with_the_airsl_prefix_and_exits_one`
  - `a_failed_report_write_exits_one`: stdout is a writer whose every write fails with
    `io::Error::other("boom")`. Asserts that stderr is `airsl: boom\n` and the exit code is 1
- These stub `HostModule`s, plus the `registry.rs` doctest, each gain a `describe` that matches
  what they install. The list comes from `git grep "impl HostModule for"`:
  - `engine.rs`: Boomerang, EvalBoomerang, Poisoner
  - `extension/host.rs`: Probe
  - `modules/registry.rs`: Stub
  - `crates/airsl/examples/custom-module/main.rs`

**Acceptance check.** `execute` runs this on this machine. It is not part of the gate, because CI
has no LuaLS. Run `cargo run -p airsl-cli -- lua setup <fixture>`, then
`~/.local/share/nvim/mason/bin/lua-language-server --check . --checklevel=Information` inside the
fixture. Do this twice: once with `.airsl/` not ignored (the default state), and once with
`.airsl/` in `.gitignore` inside a `git init` repository. Each run must show:

- no diagnostic on a script that calls at least one function of every module with correct
  arguments, and reads every field of every record in §3.4 the way `records-good.lua` does;
- exactly one `undefined-field` on a script that reads `airsstack.json.nope`.

This repeats the `run.out default_state` and `run.out dotted` cases on generated output.

## 11. Documentation

- New book chapter `book/src/ch01-04-editor-support.md`, "Editor Support", listed under Getting
  Started in `SUMMARY.md`. It covers:
  - running `airsl lua setup`;
  - what it writes;
  - the note about gitignoring `.airsl/`;
  - re-running setup after an upgrade.

  Every transcript is run against the checkout before it lands.
- `book/src/appendix-01-cli.md`: the `lua setup` entry.
- `book/src/ch07-00-your-own-module.md`:
  - the sentence "The trait has two methods" becomes three methods, naming `describe`;
  - every `HostModule` example gains `describe`;
  - a short section shows `luals::verify` in a host's test and `luals::setup` for a host's own root.
- Rustdoc for every new public item. `LuaName::new`, `ModuleDescription::builder`,
  `TypeFile::render`, `verify` and `setup` get doctests.
- Crate READMEs: only where they show a `HostModule` implementation. None do today:
  `git grep -n "HostModule for" -- '*README*'` finds nothing, while the control
  `git grep -n "HostModule" -- '*README*'` finds `crates/airsl/examples/README.md:66`. Update that
  README line if its wording becomes wrong, and compile any README example that changes.

## 12. Non-goals

- `require("<root>")` and typing it (milestone `lua-ecosystem/02`).
- The native module, LuaRocks and `--native` (milestones 03–05).
- Any LuaLS setting other than `workspace.library`. That rules out `runtime.version`,
  diagnostics tuning, and hiding `io`/`os` per policy.
- Typing a project's own Lua files, or what a script loads with `require`.
- Editor-specific configuration beyond the luarc file.
- Keeping key order, formatting or comments in a luarc that needs an edit. A file with comments
  is refused instead (L4).
- Bumping the crate version or cutting a release. The required trait method is breaking, so the
  release that ships it takes the next minor version. That is release work, outside this chain.
- Checking parameter or return *types* against the implementation. Drift is checked on key names
  and on function versus value only (§5).

## 13. Risks

- **Descriptions can be wrong about types.** `verify` proves names and kinds only, so a wrong
  parameter type misleads the editor without failing the gate. Mitigations: `describe` sits beside
  `install` in the same file, and the acceptance check calls every module.
- **A breaking trait change.** Every downstream `HostModule` stops compiling until it adds
  `describe`. The book and the example show the change.
- **LuaLS behaviour is version-specific.** The grammar and luarc rules are proven on 3.19.2 only.
  `probes/luals/run.sh` re-runs them on another version.
- **Symlinks and special files are untested on Windows** (D17). CI runs on `windows-latest`
  (`.github/workflows/ci.yml:38`), but the three Unix-only tests do not run there.