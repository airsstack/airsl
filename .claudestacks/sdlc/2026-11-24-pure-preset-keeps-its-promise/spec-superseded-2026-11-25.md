---
status: superseded
created: 2026-11-25
built-on: dc525f353d7c
---

# Spec: The `pure` preset keeps the promise its name makes

`Policy::pure()` promises a computation with no I/O, no clock and the same output for the same
input. On 0.1.6 a `pure` script can still:

- write the host's stdout and read its stdin;
- read the clock, three ways;
- learn the process cwd and probe the host filesystem through refusal messages;
- read the host's time-zone database;
- draw a different `math.random` value every run.

A `pure` script also cannot load its own sibling files, because `require` exists only on the
`Restricted` surface.

This spec closes every route airsl controls:

- `GrantSet` gains seven members, called host access: `stdin`, `stdout`, `stderr`, `clock`, `cwd`,
  `require`, `zones`. `GrantSet::declared()` holds all seven. The new `GrantSet::sealed()` holds
  none, and `pure` is built on it.
- Host modules check each member before acting.
- The `Minimal` surface withholds `print` and makes `math.random` repeatable.
- The filesystem guard refuses before it touches the filesystem when the answer cannot be yes.

A host can give a `pure`-based policy any member back explicitly, `require` included, and
`airsl doctor` shows what it was given. The variation airsl does not control is named in the book:
`pairs` order, printed addresses, and the order `table.sort` gives elements its comparator ties.
`trusted` and `confined` scripts behave as on 0.1.6, apart from the refusal changes listed in §14.
Ships as `airsl` 0.2.0 and `airsl-cli` 0.2.0. Tracked as airsstack/airsl#11, #12, #13 and #14.

## 1. Evidence

Every behaviour claim below cites one of these files in `probes/`. Each was run on 2026-11-25 in
one of two ways:

- against this checkout at `f281887` (`airsl 0.1.6`, built with `cargo build -p airsl-cli`);
- as a throwaway crate against the exact dependency version in `Cargo.lock`.

Files without a version in their name come from the intent stage (0.1.5, checkout `62d2572`). They
were re-run on 0.1.6, and those runs are the `-0.1.6.out` files.

| Id | Files | What it is |
|---|---|---|
| E1 | `require.lua`, `require-sib.lua`, `require.out` | `type(require)` and `require "require-sib"` under `pure` and `confined` |
| E2 | `globals.lua`, `globals-0.1.6.out` (`globals.out` = 0.1.5, identical) | Every global and library function each preset loads |
| E3 | `pure-reach.lua`, `pure-reach-0.1.6.out` | Three runs under `pure`: `print`, `stdio`, `time.now`, `path.absolute`, `env.get`, `math.random`, `pairs`, `tostring` |
| E4 | `warn.lua`, `warn-0.1.6.out` | `warn("@on")`, then `warn(...)`, under `pure`: exit 0, stdout and stderr empty |
| E5 | `leaks.lua`, `tz.lua`, `proc.lua`, `leaks.out` | Refusals from `hash`, `fs`, `glob` and `proc` under `pure`; `time.parse` with `%Q` and with `%z`; a `TZDIR` swap; `stdio.isatty`; the keys of `ext.granted()`. A block left over from an earlier `proc.lua` was removed, and every block now matches its script |
| E6 | `mlua-0.12.0-minimal.{Cargo.toml,rs,txt}` | `mlua = "=0.12.0"` (workspace features) on a `STRING\|TABLE\|MATH\|UTF8` state; each mode run three times in separate processes |
| E7 | `mlua-0.12.0-randomseed-wrapper.{rs,txt}` (E6's `Cargo.toml`) | A Rust function wrapping Lua's own `math.randomseed` and refusing only the no-argument form; two runs |
| E8 | `lua-5.4.8-lmathlib-seed.txt` | `lmathlib.c:609-662` of the vendored Lua (`lua-src` 550.1.1): `setseed`, `randseed`, `math_randomseed`, `setrandfunc` |
| E9 | `jiff-0.2.35-tz-lookup.{Cargo.toml,rs,txt}` | `jiff = "=0.2.35"`: the parse paths `time.parse` uses, run with the system tzdb and with `TZDIR` pointing at a swapped zone file |
| E10 | `jiff-0.2.35-zoneinfo-tzdir.txt` | `jiff-0.2.35/src/tz/db/zoneinfo/enabled.rs:42-58`: `TZDIR` is read first, then the default directories |
| E11 | `clap-4.6.6-allow-access.{Cargo.toml,rs,txt}` | `clap = "=4.6.6"`: a repeatable `--allow-access NAME` whose values come from a list |
| E12 | `host-access-enum.{rs,txt}` | The §3.1 enum declaration under `rustc 1.94.1 --edition 2024 -D warnings`: exit 0 |
| E13 | `mlua-0.12.0-sort-pivot.{rs,txt}` (E6's `Cargo.toml`) | `table.sort` with tied keys on a `Minimal` state: 100 sorts of identical input, two runs |
| E14 | `lua-5.4.8-ltablib-pivot.txt` | `ltablib.c:230-272`, `:360-370`, `:398-406`: `l_randomizePivot` draws on `clock()` and `time()`. It is used for intervals of `RANLIMIT` (100) or more, and after an unbalanced partition |
| E15 | `mlua-0.12.0-state.{rs,txt}` | On one state: the text of calling a withheld `print`, and two evaluations of `math.random` after one seeding |
| E16 | `jiff-0.2.35-both.{rs,txt}` | A pattern with both `%z` and `%Q`: the zone is named, and `to_zoned` reads it (and fails against the swapped zone) |
| E17 | `hook.lua`, `hook.out` | `hook.payload` reads stdin and `hook.emit` writes stdout under `pure` on 0.1.6 |
| E18 | `std-is-absolute.{rs,txt}` | `Path::is_absolute` for `""`, `x`, `/a`, `\a`, `C:a` and `C:\a` on Darwin arm64, plus the `std` doc text for Windows (`library/std/src/path.rs:2489-2510`, rustc 1.94.1) |
| E19 | `const-access-set.{rs,txt}` | A private `u8` bit set over `HostAccess`, filled inside a `const fn` and used in `const` items: exit 0 |

Lockfile pins: `Cargo.lock:148-149` clap 4.6.6, `:372-373` jiff 0.2.35, `:452-453` lua-src 550.1.1,
`:477-478` mlua 0.12.0. Each probe crate's own `Cargo.lock` resolves the same version. The last
lines of E6, E7, E9 and E11 are the `grep` of that lockfile.

## 2. What `pure` reaches today, and after this spec

| Route under `pure` | 0.1.6 (evidence) | After this spec |
|---|---|---|
| `print` | writes host stdout (E3 `print: function`) | `nil`, withheld by `Minimal` (§4.1) |
| `stdio.read`/`lines`/`write`/`error`/`isatty` | work with no grant (E3 `stdio.write: true`) | refused unless `stdin`/`stdout`/`stderr` is granted (§5.1) |
| `hook.payload`, `hook.emit` | read stdin, write stdout (E17 `{"got":1}`) | refused unless `stdin` / `stdout` is granted (§5.1) |
| `time.now`, `time.monotonic` | read the clock (E3 `now: true 1795581841`) | refused unless `clock` is granted (§5.2) |
| `math.randomseed()` | re-seeds from `time(NULL)` and **returns the clock** (E8 `:628-645`; E6 `1795579300 4308691144`, with `date +%s` printing `1795579312` twelve seconds later) | refused on `Minimal` (§4.2) |
| `math.random` | differs per run (E3 `540745`/`325554`/`523488`; E6 `fresh-random`) | the same sequence on every fresh engine (§4.2) |
| `time.parse` with a named zone | reads the host tzdb and honours `TZDIR` (E5 `1719849600`, then `1719802800` with the swapped file; E9; E10; E16) | refused unless `zones` is granted, before any lookup (§5.3) |
| `path.absolute` of a relative path | returns the process cwd (E3 `absolute: true /Users/…/probes/x`) | refused unless `cwd` is granted (§5.4) |
| `fs`/`hash.hash_file`/`glob.walk` refusing a relative path | the message carries the cwd (E5 `` `/private/tmp/…/cli-probe/p.lua` is outside the granted read roots ``) | refused before resolution, with the path as written (§5.5) |
| the same, with an absolute path | the message shows the host layout: `/etc/hosts` comes back as `/private/etc/hosts`, while `/nonexistent/x` comes back as written (E5) | refused before any filesystem access, with the path as written (§5.5) |
| `require` | `nil` (E1 `nil`, `attempt to call a nil value`) | a refusing function; with `require` granted, the confined loader (§6) |
| `proc.run`, `env.get` | refused, no leak (E5 `` `ls` is not granted — no executables are ``; E3 `` `HOME` is not granted — no environment variables are ``) | unchanged |
| `warn` | writes nothing (E4; E6 `warn` `stderr=[]`) | unchanged |
| `collectgarbage("count")` | repeatable (E6 `74.4482` ×3) | unchanged |
| `table.sort` on elements its comparator ties | the order differs per run: 100 sorts of the same input gave 100 orders, while all-distinct keys gave 1 (E13). Pivots come from `clock()`/`time()` (E14) | unchanged; named in the book (§12) |
| `pairs` order over string keys, `tostring({})`, `string.format("%p", …)` | differ per run (E3; E6 `format-p`) | unchanged; named in the book (§12) |

## 3. Host access: the policy layer

### 3.1 `HostAccess`

New file `crates/airsl/src/sandbox/host_access.rs`, re-exported as `airsl::HostAccess` and
`airsl::sandbox::HostAccess`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum HostAccess { Stdin, Stdout, Stderr, Clock, Cwd, Require, Zones }
```

The declaration compiles as shown (E12).

- `as_str` returns `stdin`, `stdout`, `stderr`, `clock`, `cwd`, `require` or `zones`. `Display`
  writes `as_str`.
- `pub const ALL: [HostAccess; 7]` lists the members in declaration order. Every listing uses that
  order (`Display`, `doctor`, `ext.granted()`, refusals), and `Ord` follows it.
- `FromStr` accepts exactly the seven lowercase names. Anything else, including `""` and `STDOUT`,
  returns `Error::InvalidName { kind: "host access", value, reason: "expected one of stdin, stdout,
  stderr, clock, cwd, require, zones" }`. That error's text is
  `` invalid host access `bogus`: expected one of stdin, stdout, stderr, clock, cwd, require, zones ``
  (shape: `error.rs:196`, `#[error("invalid {kind} `{value}`: {reason}")]`).

| Member | Authorises |
|---|---|
| `stdin` | `stdio.read`, `stdio.lines`, `stdio.isatty("stdin")`, `hook.payload` |
| `stdout` | `stdio.write`; `stdio.isatty()`, `isatty("stdout")` and any other stream name; `hook.emit`, `hook.context` |
| `stderr` | `stdio.error`, `stdio.isatty("stderr")` |
| `clock` | `time.now`, `time.monotonic` |
| `cwd` | resolving a relative path: `path.absolute`, and every `PathGuard` call (`fs.*`, `hash.hash_file`, `glob.walk`) |
| `require` | the `require` global loading from the script's folder (§6) |
| `zones` | `time.parse` with a pattern that names an IANA time zone |

Host access bounds **host modules and the confined `require`** only. Lua's own globals belong to
the surface:

- On `Restricted`, `os.time`, `os.clock`, `os.date` and `print` stay present whatever host access
  says (E2 confined listing).
- On `Full`, so do Lua's own `io` and `require` (§6).

The book says so (§12).

### 3.2 `GrantSet`

`GrantSet` (`grant_set.rs:52-59`) gains a private field, `access`: a bit set over `HostAccess` (one
bit per member, in a `u8`). It is not a `BTreeSet`, so that every constructor that is `const` today
stays `const`. A bit set can be filled inside a `const fn`, used in `const` items, and answers
correctly (E19).

| Item | Behaviour |
|---|---|
| `GrantSet::declared()` (`grant_set.rs:64`, `const`) | holds all seven members; fs, env and proc empty. Rustdoc: the baseline a confined script gets |
| `GrantSet::sealed()` (new, `const`) | holds nothing on any axis. Rustdoc: the base to build a `pure`-based grant set on, because building on `declared()` reopens every member |
| `GrantSet::unrestricted()` (`:79`, `const`) | reach unchanged; `allows` is true for every member |
| `Default` | stays equal to `declared()`. The derive on `:52` becomes an `impl Default` that calls `declared()`. So `declared_is_the_default_reach` (`:191-194`) keeps passing, and `GrantSet::default().with_fs(..)` keeps all seven members |
| `with_access(self, HostAccess) -> Self` (new, `const`) | sets the member's bit; a member already present stays as it is |
| `allows(&self, HostAccess) -> bool` (new, `const`) | `is_unrestricted()`, or the member's bit is set |
| `access(&self) -> impl Iterator<Item = HostAccess>` (new) | the members in `ALL` order; for `unrestricted()`, all seven |
| `is_empty` (`:141`) | also requires that no access bit is set. `sealed()` is empty; `declared()` is not |
| `Display` (`:149`) | `unrestricted` is unchanged. Otherwise: `none` when empty, else the parts joined by `; `, with `access m1,m2,…` first and then the existing `read`/`write`/`env`/`exec` parts. `declared()` renders as `access stdin,stdout,stderr,clock,cwd,require,zones` |

No `without_*` method is added. A host that wants fewer members starts from `sealed()` and adds
the ones it wants.

### 3.3 Presets

| Preset | `grants` (`policy.rs`) | `const` |
|---|---|---|
| `trusted()` (`:61-64`) | `GrantSet::unrestricted()`, unchanged | stays |
| `confined()` (`:73-76`) | `GrantSet::declared()`, the same call, which now holds all seven members | stays |
| `pure()` (`:87-90`) | `GrantSet::sealed()` (was `declared()`) | stays |

The `Policy::pure()` rustdoc states what it guarantees (§12), and that a host widening it should
build on `GrantSet::sealed()`.

## 4. The `Minimal` surface

### 4.1 `print`

`Minimal` withholds `print` after the state is built, alongside the chunk loaders
(`builder.rs:108-111` → `withhold_unsafe_globals`, `:130-149`). Removing the global leaves nothing
callable (E6 `print-removed`: `nil false` ×3).

A script that calls it gets Lua's own error, prefixed with the chunk and line:
`printnil.lua:1: attempt to call a nil value (global 'print')` (E15). In E15 that text is preceded
by `runtime error: `, which is mlua's `Display` of the error on the Rust side. `Restricted` and
`Full` keep `print` (E2). `warn` stays on every surface: it writes nothing on an mlua-created state
(E4, E6).

The withheld name lives in `language_surface.rs` as a `pub(crate)` constant that the builder reads,
beside `CHUNK_LOADERS` and `UNSAFE_OS_FUNCTIONS` (`:18-36`).

The `Minimal` doc (`language_surface.rs:58-64`) says that dropping `os` "removes the last source of
nondeterminism a script could reach". That is corrected to name what remains:

- `math.random`'s seeding, which this spec fixes;
- the VM's own variation (§12).

### 4.2 `math.random` and `math.randomseed`

Lua 5.4.8 seeds the generator at `luaopen_math` from `time(NULL)` and the state's address (E8
`:628-632`, `:657-662`). `math.randomseed()` with no argument does the same, and returns both seed
values (E8 `:635-645`).

On `Minimal`, the builder does two things after the libraries load and before
`ResourceLimits::apply` (`builder.rs:112`):

1. **It seeds with 0,** by calling Lua's own `math.randomseed(0)`. After that,
   `math.random(1, 1000000)` returns `275394` in every separate process (E6 `seeded-random` ×3; E7
   `first=275394` ×2; E15 `eval 1: 275394` ×2).
2. **It wraps `math.randomseed`** in a Rust function that holds the original.
   - With no arguments, the wrapper returns `Error::Denied { module: "math", operation:
     "randomseed", detail: "a seed is required — the no-argument form reads the clock" }`.
   - With any arguments, it calls the original with them and returns its values.
     `math.randomseed(42)` returns `42, 0`, and seeding with 42 again repeats the sequence (E7
     `returns=42,0`, `repeat=161510,161510`).
   - An argument error from the original reaches the script as mlua reports it, without Lua's own
     function name: `runtime error: bad argument #1 to '?' (number expected, got nil)`, followed by
     a traceback (E7 `nil-arg=`). On 0.1.6 the same call reads
     `bad argument #1 to 'math.randomseed' …`. The author accepted this change for `Minimal`.

The seed is set once per engine. A reused engine continues the sequence across evaluations, the
same way it keeps globals (`CLAUDE.md`): E15 shows `eval 1: 275394` and `eval 2: 909833` on one
state, in both runs. `Restricted` and `Full` keep Lua's own seeding and Lua's own `randomseed`.

## 5. Host modules

Every check runs inside the Rust function, before the operation (`CLAUDE.md`: enforcement lives in
Rust). The check and its refusal text are written once, in a new crate-private
`crates/airsl/src/modules/access.rs` (§10). Each module captures `context.grants()` at install
(`InstallContext::grants`, `registry.rs`), as `fs`, `env` and `proc` already do.

**Which error wins.** mlua converts a function's arguments before the function body runs, so an
argument error wins over a refusal. E5 shows this today: `glob.walk` raised `bad argument #2`
before the grant was ever consulted. Inside a function body, the order is as each subsection below
states.

### 5.1 `stdio` and `hook`

| Function (install line) | Member checked |
|---|---|
| `stdio.read` (`stdio.rs:80`), `stdio.lines` (`:96`) | `stdin` |
| `stdio.write` (`:117`) | `stdout` |
| `stdio.error` (`:136`) | `stderr` |
| `stdio.isatty(s)` (`:153`) | `stdin` for `"stdin"`, `stderr` for `"stderr"`, otherwise `stdout`. That is the mapping the function already uses to pick a stream (`stdio.rs:140-151`) |
| `hook.payload` (`hook.rs:98`) | `stdin`, checked before stdin is read, even when stdin is empty |
| `hook.emit` (`:106`), `hook.context` (`:119`) | `stdout` |

### 5.2 `time.now`, `time.monotonic`

Both (`time.rs:90`, `:109`) check `clock`. `time.format` (`:128`) is unchanged: it renders an
explicit instant with `TimeZone::UTC`, or as RFC 3339, and consults no zone database
(`time.rs:111-125`).

### 5.3 `time.parse` and named zones

`time.parse` (`time.rs:151`) has two paths.

- **No pattern.** `text.parse::<jiff::Timestamp>()` never reads the tzdb, even with a bracketed
  zone annotation. The result is the same with `TZDIR` swapped, and an unknown zone name still
  parses (E9 `rfc9557 … Ok(1719849600)` in both runs). This path is unchanged.
- **With a pattern.** `strtime::parse` runs first, then `to_zoned()`. When the pattern names a
  zone, `to_zoned` reads it from `TZDIR` or the system zoneinfo (E9 `%Q to_zoned`: `1719849600`,
  then `1719802800` with the swapped file; E10). The parsed value names the zone before any lookup
  (E9 `iana_time_zone() before lookup: Some("America/New_York")`).

The rule for the pattern path: after `strtime::parse` succeeds and before `to_zoned`, check `zones`
if `iana_time_zone()` is `Some`.

- Text that does not match the pattern fails in `strtime::parse`, first, with its existing error.
- Offset-only patterns (`%z`) report `None` and read nothing (E9 `%z iana_time_zone(): None`, the
  same result in both runs).
- A pattern with both `%z` and `%Q` reports the zone, and `to_zoned` reads it (E16
  `Some("America/New_York")`, then an error against the swapped file). So it needs `zones`.

### 5.4 `path.absolute`

`path.absolute` (`path.rs:128-133`, `fn absolute` at `:279`) checks `cwd` only when its argument is
not absolute according to `std::path::Path::is_absolute`. That is the predicate `path.is_absolute`
already uses (`path.rs:121-126`). An absolute argument needs nothing, and with `cwd` granted,
behaviour is unchanged.

- On this host, `""`, `x`, `\a` and `C:a` are not absolute (E18).
- On Windows, `std` documents that a path is absolute only "if it has a prefix and starts with the
  root: `c:\windows` is absolute, while `c:temp` and `\temp` are not" (E18, `path.rs:2495-2496`).

### 5.5 The filesystem guard (`fs`, `hash.hash_file`, `glob.walk`)

All three reach the filesystem through `PathGuard` (`guard.rs`; `hash.rs:73`, `glob.rs:99`).
`PathGuard::read` and `PathGuard::write` (`guard.rs:77`, `:90`) resolve the path first and check
the grant after. Resolution (`resolve`, from `:164`) calls `std::path::absolute` and then
`canonicalize`, and that order is what leaks the cwd and the host layout (E5).

When the grant set is not unrestricted, the new order is:

1. **Lexical validation, as today.** `native::reject_unrepresentable(raw)` (`guard.rs:165-176`)
   still runs first, and still raises `Error::UncheckablePath` for an interior NUL, a verbatim
   `\\?\` path or a device name. It only reads the string (`paths/rules.rs:167-185`), so a raw path
   containing NUL never reaches a refusal message.
2. **Rule 2: the axis has no roots.** If the root list for this access (read or write) is empty,
   refuse before any filesystem access. The message is the existing one, with the path exactly as
   the script passed it: `` `{raw}` is outside the granted {read|write} roots: none are granted ``
   (wording from `guard.rs:108-129`).
3. **Rule 1: a relative path without `cwd`.** If `cwd` is not granted and `raw` is not absolute
   (`Path::is_absolute`), refuse before resolution with shape B (§9):
   `` the relative path `{raw}` needs `cwd`, which is not granted — {granted} ``.
4. **Otherwise, resolve and check as today.** That includes the existing `UncheckablePath` cases:
   "the working directory could not be read", "it climbs through a directory that does not exist"
   and "no part of it exists" (`guard.rs:178-221`).

Rule 2 comes before rule 1, because with no roots nothing could be allowed, and its message needs no
cwd. An operation on two paths (for example `fs.copy`) checks each path against its own axis, as
today. Rules 2 and 1 apply under every policy that is not unrestricted. Inputs that used to fail in
step 4 now fail in step 2 or 3, with `Error::Denied` (§14).

### 5.6 `ext.granted()`

`granted_table` (`ext.rs:154-205`) gains an `access` key: a sequence of member names in `ALL` order.

| Grant set | `access` |
|---|---|
| `declared()` | `{"stdin","stdout",…}`, all seven |
| `unrestricted()` | all seven |
| `sealed()` | `{}` |

The existing keys `env`, `fs`, `limits`, `proc` and `unrestricted` (E5 `ext.granted:` line) are
unchanged.

## 6. `require`

`RequireDisposition::decide` (`require_loader.rs:71-77`) also takes the grants, and `engine.rs:281`
passes it `self.policy.grants()`. Its doc (`require_loader.rs:59-67`) is rewritten to match.

| Surface | `require` granted | Script has a folder | Disposition | What the script sees |
|---|---|---|---|---|
| `Full` | either | either | `Native` | Lua's own (unchanged) |
| `Restricted` / `Minimal` | yes | yes | `Confined(root)` | the confined loader, as `confined` has today (E1 `function`, `true 42`) |
| `Restricted` / `Minimal` | yes | no (built from source) | `Absent` | `nil`, as today |
| `Restricted` / `Minimal` | no | either | `Refused` (new) | a function that returns `Error::RequireDenied` without reading its argument |

- `Error::RequireDenied { detail: String }` is a new variant (`Error` is `#[non_exhaustive]`), with
  `#[error("require denied: {detail}")]`. Its `detail` is shape A (§9). `Error::Denied` does not
  fit, because its text is `{module}.{operation} denied`.
- The refusing function takes any arguments (`mlua::MultiValue`), so no argument error can come
  before the refusal.
- `confined` holds `require`, so every `confined` script gets the same disposition as on 0.1.6.
- `Policy::pure()` with `require` granted gives a multi-file script the confined loader, under the
  same containment (`RequireEscape`, `RequireCycle`) as `Restricted`.
- Granting `require` hands each module its canonical absolute path, both as its second argument and
  in its chunk name. That is the behaviour shipped in 0.1.5 (`require_loader.rs:203-217`), and the
  book states that granting `require` reveals the absolute location of the script's folder.
- The module cache is kept across evaluations, as today. It is unreachable while `require` is
  `Refused` or `Absent`.

## 7. Extension negotiation

Today `negotiate` (`negotiate.rs:275`) starts from `GrantSet::declared()` (`:279`) and replaces the
policy's grants with the result (`:286-288`). It changes to start from `GrantSet::sealed()` plus
every member that `ceiling.policy().grants().access()` yields. It then applies the manifest's fs,
env and proc requests as today. The negotiated surface already comes from the ceiling (`:287`).

- A `confined` ceiling gives each extension all seven members, as 0.1.6 did.
- A `pure` ceiling gives none. A host that wants its extensions to have `require` or `stderr` puts
  those members in the ceiling.
- Manifests gain no field, so an extension cannot request host access.

## 8. The CLI

`Grants` (`cli.rs:90-120`) gains this field:

```rust
/// Host access the sandbox may use: stdin, stdout, stderr, clock, cwd, require or zones.
/// Repeat for several. Only `pure` starts without them.
#[arg(long = "allow-access", value_name = "NAME", value_parser = <PossibleValuesParser over HostAccess::ALL names, mapped through HostAccess::from_str>)]
pub access: Vec<HostAccess>,
```

E11 proves this parser shape against clap 4.6.6, with a `String` result. Mapping that string
through `HostAccess::from_str` cannot fail, because the possible values are exactly the names in
`ALL`. A rejected value prints clap's own text and exits with status 2 (E11; §9).

- **Where it applies.** `Grants` is flattened into `run`, `test`, `ext doctor` and `ext fire` (the
  last two via `ExtFlags`, `cli.rs:216-218`), and newly into `doctor`. All five accept
  `--allow-access`.
- **How the flags fold.** `into_grant_set` (`cli.rs:112-120`) starts from the preset's own grants
  (`policy.grants().clone()`) instead of `GrantSet::declared()`. It adds each `--allow-access`
  member, then the fs, env and exec flags as today.

  | Preset | Result |
  |---|---|
  | `pure` | `sealed()` plus the flags |
  | `confined` | `declared()` plus the flags. `--allow-access` changes nothing, because every member is already there |
  | `trusted` | the flags are ignored, as every grant flag is today (`cli.rs:283-290`) |

- **`ext doctor` and `ext fire`.** Their ceiling is always `confined` (`resolve_ceiling`,
  `cli.rs:322-330`), so there `--allow-access` is accepted but cannot change the ceiling.
  `appendix-01-cli.md` states this; the behaviour does not change.
- **Duplicates.** `--allow-access stdout --allow-access stdout` is accepted (E11 `ok ["stdout",
  "stdout"]`) and grants `stdout` once.
- **`doctor`.** `Command::Doctor` gains `#[command(flatten)] grants: Grants`. `main.rs:57-60`
  builds its policy with `cli::resolve_policy(policy, None, None, grants)`, the same function `run`
  and `test` use (`main.rs:45`, `:53`). So `doctor --policy pure --allow-access stderr` prints
  `grants:       access stderr`. Its ceiling flags are not added (§16).

## 9. Error handling

Every new refusal is a catchable Lua error, raised before any part of the operation runs.

**Shapes.** These are built only in `modules/access.rs`:

```
A  `{member}` is not granted — {granted}
B  {subject} needs `{member}`, which is not granted — {granted}

{granted} = "the granted access is m1, m2, …"   members in ALL order, ", "-joined
          | "no access is"                       when the set holds no member
```

- The tail mirrors `env`'s `` `HOME` is not granted — no environment variables are `` (E3) and
  `proc`'s `` `ls` is not granted — no executables are `` (E5).
- An unrestricted grant set never reaches these.
- `{subject}` is only ever ``the relative path `{raw}` `` or ``the named time zone `{name}` ``,
  with the value exactly as the script passed it.
- `Denied`'s `module` and `operation` are the `&'static str` values each module already uses.

| Trigger | Error | Exact text (nothing granted, unless stated) |
|---|---|---|
| `stdio.read` / `stdio.lines` | `Denied{stdio, read\|lines}` | ``stdio.read denied: `stdin` is not granted — no access is`` |
| `stdio.write` | `Denied{stdio, write}` | ``stdio.write denied: `stdout` is not granted — no access is`` |
| `stdio.error` | `Denied{stdio, error}` | ``stdio.error denied: `stderr` is not granted — no access is`` |
| `stdio.isatty(s)` | `Denied{stdio, isatty}` | ``stdio.isatty denied: `stdout` is not granted — no access is`` |
| `hook.payload` / `emit` / `context` | `Denied{hook, …}` | ``hook.emit denied: `stdout` is not granted — no access is`` |
| `time.now` / `monotonic` | `Denied{time, now\|monotonic}` | ``time.now denied: `clock` is not granted — no access is`` |
| `time.parse` with a named zone | `Denied{time, parse}` | ``time.parse denied: the named time zone `America/New_York` needs `zones`, which is not granted — no access is`` |
| `path.absolute` of a relative path | `Denied{path, absolute}` | ``path.absolute denied: the relative path `x` needs `cwd`, which is not granted — no access is`` |
| Guard rule 1 | `Denied{fs\|hash\|glob, op}` | with `stderr` granted: ``fs.read denied: the relative path `p.lua` needs `cwd`, which is not granted — the granted access is stderr`` |
| Guard rule 2 | `Denied{fs\|hash\|glob, op}` | ``fs.read denied: `/etc/hosts` is outside the granted read roots: none are granted`` |
| `require(…)`, not granted | `RequireDenied` | with `stderr` granted: ``require denied: `require` is not granted — the granted access is stderr`` |
| `math.randomseed()` on `Minimal` | `Denied{math, randomseed}` | ``math.randomseed denied: a seed is required — the no-argument form reads the clock`` |
| `math.randomseed(nil)` on `Minimal` | Lua's, via mlua | ``runtime error: bad argument #1 to '?' (number expected, got nil)``, then a traceback (E7) |
| `print(…)` on `Minimal` | Lua's | ``{chunk}:{line}: attempt to call a nil value (global 'print')`` (E15) |
| `HostAccess::from_str` | `InvalidName` | ``invalid host access `bogus`: expected one of stdin, stdout, stderr, clock, cwd, require, zones`` |
| `--allow-access bogus` / `STDOUT` | clap, exit 2 | ``error: invalid value 'bogus' for '--allow-access <NAME>'``, then ``  [possible values: stdin, stdout, stderr, clock, cwd, require, zones]`` (E11) |
| `--allow-access`, `--allow-access=` | clap, exit 2 | ``error: a value is required for '--allow-access <NAME>' but none was supplied``, then the same possible-values line (E11) |

## 10. Architecture

The author approved this section on its own, before the spec was written. The review's findings add
rustdoc edits only, marked *(doc)* below. They add no files and change no module boundaries.

### 10.1 Module tree

```
crates/airsl/src/
├── sandbox/
│   ├── host_access.rs      NEW   HostAccess (as_str, FromStr, ALL, Display)
│   ├── grant_set.rs        EDIT  access bit set, sealed, with_access, allows, access, is_empty,
│   │                             Display, impl Default = declared()
│   ├── policy.rs           EDIT  pure() → sealed(); the pure end-to-end test; pure() doc
│   ├── language_surface.rs EDIT  Minimal's withheld name and seeding; Minimal doc (§4.1)
│   └── mod.rs              EDIT  `pub mod host_access;`, `pub use`, a Responsibilities bullet (doc)
├── builder.rs              EDIT  applies the Minimal extras after the state is built
├── modules/
│   ├── access.rs           NEW   pub(crate): the HostAccess check and shapes A and B
│   ├── mod.rs              EDIT  `mod access;` only
│   ├── guard.rs            EDIT  rules 1 and 2, after lexical validation
│   ├── stdio.rs, hook.rs   EDIT  stream checks; module docs drop "needs no grant" (doc)
│   ├── time.rs             EDIT  clock and zones checks; module doc drops "needs no authority" (doc)
│   ├── path.rs             EDIT  cwd check in absolute; module doc (doc)
│   └── ext.rs              EDIT  ext.granted().access
├── require_loader.rs       EDIT  decide(surface, root, grants), Refused and its stub; decide doc
├── engine.rs               EDIT  set_require passes the grants
├── error.rs                EDIT  Error::RequireDenied
├── lib.rs                  EDIT  re-export HostAccess
└── extension/negotiate.rs  EDIT  base grants = sealed() + the ceiling's access
crates/airsl-cli/src/
├── cli.rs                  EDIT  --allow-access; fold onto the preset's grants; Doctor takes Grants
└── main.rs                 EDIT  Doctor goes through resolve_policy
```

### 10.2 Why each new unit exists

- **`sandbox/host_access.rs`.** The members are one vocabulary, read by Rust callers, the CLI,
  `doctor`, `ext.granted()` and every refusal. They are unparameterised, while `grants.rs` documents
  itself as the parameterised allowlists (`grants.rs:1-5`). So `HostAccess` is a concept of its own,
  beside `language_surface.rs` and `resource_limits.rs`, not a qualifier file of `grants`. The bit
  set is a private detail of `GrantSet`, not a public type.
- **`modules/access.rs`.** Seven call sites (`stdio`, `hook`, `time` ×2, `path`, `guard`,
  `require_loader`) need the same check and the same text. With both in one place, a message cannot
  name one member while the check tested another. `guard.rs` exists for the same reason
  (`guard.rs:1-7`).
- **`Error::RequireDenied`.** It sits in `error.rs`, beside the other `require` errors
  (`RequireEscape`, `RequireCycle`, `error.rs:216-232`).

### 10.3 Dependency direction

```
airsl-cli ─► airsl
engine ─► require_loader ─► modules::access ─► sandbox::{GrantSet, HostAccess} ─► error
modules::{stdio, hook, time, path, guard} ─► modules::access
extension::negotiate ─► sandbox
builder ─► sandbox::language_surface
```

The one new edge is `require_loader` → `modules::access`, and it cannot form a cycle, because
nothing under `modules/` uses `require_loader`:

- `grep -rln require_loader crates/airsl/src` lists only `lib.rs`, `engine.rs` and
  `extension/manifest.rs`.
- As a control, `grep -rln sandbox crates/airsl/src/modules` finds `registry.rs`, `env.rs` and
  `time.rs`, so the same search does find references that exist.

`sandbox/` stays free of the VM (`policy.rs:13-14`).

### 10.4 Guideline rules applied (`claudestacks-guideline-rust:rust-guidelines` 0.1.5)

| Reference | Applied as |
|---|---|
| `modularity.md` rule 1 | `access.rs` owns the check and its text; `host_access.rs` owns the vocabulary |
| `modularity.md` rule 2 | the CLI takes its values from `HostAccess::ALL` (E11), with no mirror enum. `PolicyName` stays, because it names presets, which are a CLI concept |
| `strong-types.md` | `with_access(HostAccess)` and `allows(HostAccess)`, with no `bool` parameter; `FromStr` is the single parse |
| `static-dispatch.md` | no `dyn` is added; the `randomseed` wrapper is an `mlua` function that captures an `mlua::Function` |
| `mod-rs-export-only.md` | `sandbox/mod.rs` and `modules/mod.rs` gain only `mod`/`pub use` lines and a doc bullet |
| `unit-test-mandate.md` | both new files ship `#[cfg(test)] mod tests`, and every edited file gains tests for its new branches (§11) |
| `doc-comment-discipline.md` | four-question module docs; no issue numbers or workflow words in source; the stale module docs listed in §10.1 are corrected |
| `strict-quality.md` | `cargo make dod` is the gate; tests opt out of `unwrap_used` with `#[expect(.., reason)]` |
| `workspace.md` | both crates move to 0.2.0 together, and the workspace dependency on `airsl` follows |
| `microsoft-guidelines.md` | `HostAccess` derives `Debug` (M-PUBLIC-DEBUG) and implements `Display` (M-PUBLIC-DISPLAY); `GrantSet`'s constructors stay `const` (§3.2) |
| Global module layout | no `<concept>_<qualifier>` files and no `access_error.rs`: refusals are `Error::Denied`/`RequireDenied`, built in `access.rs` |

## 11. Testing

Tests are inline `#[cfg(test)] mod tests` (`CLAUDE.md`: there are no `tests/` directories), and each
test name is a sentence.

### 11.1 New tests

| File | Tests |
|---|---|
| `host_access.rs` | `every_member_round_trips_through_its_name`; `parsing_rejects_an_unknown_name_with_the_expected_list` (`bogus`, `""`, `STDOUT`); `all_lists_the_members_in_declaration_order` |
| `grant_set.rs` | `declared_holds_every_host_access_member`; `sealed_grants_nothing_and_is_empty`; `unrestricted_allows_every_member`; `with_access_is_idempotent`; `display_lists_access_first_then_the_allowlists`; `the_constructors_can_build_constants` (a `const` item from `declared()`, and one from `sealed().with_access(..)`) |
| `policy.rs` | `pure_reaches_nothing_a_script_could_observe` and `confined_keeps_every_host_access_member` (described below this table) |
| `language_surface.rs` / `builder.rs` | `a_pure_engine_withholds_print`; `minimal_random_is_the_same_sequence_on_every_fresh_engine`; `minimal_refuses_randomseed_without_a_seed`; `minimal_randomseed_with_a_seed_repeats`; `restricted_keeps_print_and_lua_seeding` |
| `modules/access.rs` | shapes A and B, with nothing granted and with two members granted |
| `guard.rs` | `no_roots_refuses_before_touching_the_filesystem` (a symlinked absolute path comes back as written); `a_relative_path_without_cwd_is_refused_before_resolution`; `no_roots_wins_over_the_relative_rule`; `a_nul_path_is_still_uncheckable_before_either_rule`; `a_relative_path_with_cwd_resolves_as_before` |
| `stdio.rs`, `hook.rs`, `time.rs`, `path.rs` | one refusal test per function in §5.1–§5.4; `time.parse` with `%z` works without `zones`; `time.parse` with `%z %Q` needs `zones`; `path.absolute` of an absolute path works without `cwd` |
| `ext.rs` | `granted_lists_access_members_in_order`, for `declared`, `sealed` and `unrestricted` |
| `require_loader.rs` | the four dispositions in §6; `a_pure_script_with_require_granted_loads_its_siblings`; `a_pure_script_with_require_granted_cannot_load_outside_its_folder` (`RequireEscape`); `the_refusing_require_ignores_its_argument` |
| `negotiate.rs` | `a_pure_ceiling_negotiates_no_access`; `a_confined_ceiling_negotiates_every_member`; `a_ceiling_with_stderr_and_require_passes_exactly_those` |
| `airsl-cli` `cli.rs` / `doctor.rs` | `allow_access_parses_each_member_and_repeats`; `allow_access_rejects_an_unknown_name`; `pure_flags_fold_onto_sealed_not_declared`; `confined_allow_access_changes_nothing`; `doctor_accepts_the_grant_flags`; `doctor_reports_pure_with_stderr_as_access_stderr` |

The two `policy.rs` tests:

- **`pure_reaches_nothing_a_script_could_observe`** runs one script under `Policy::pure()`. It
  asserts that `print == nil`; that every call in §5 and §6, plus `math.randomseed()`, is refused
  with its §9 text; and that `math.random` matches a second fresh `pure` engine's.
- **`confined_keeps_every_host_access_member`** runs under `confined`. It asserts that
  `ext.granted().access` lists all seven members. It also asserts that these calls do not refuse:
  `stdio.isatty` for each stream, `time.now`, `time.monotonic`, `path.absolute("x")`, `time.parse`
  with `%Q`, and `require` from a file script.
  - It calls nothing that reads stdin or writes stdout or stderr, so it neither blocks nor writes
    past libtest's output capture.
  - The function bodies behind those checks are unchanged code, already covered by the existing
    tests.

### 11.2 Existing tests whose expectation changes

| Test (location on `f281887`) | Why |
|---|---|
| `declared_grants_nothing_and_waives_nothing` (`grant_set.rs:196-201`) | `declared()` is no longer empty; the test moves to `sealed()` |
| `each_reach_renders_for_a_report` (`grant_set.rs:216-218`) | `declared()` now renders the access list |
| `display_renders_roots_through_the_script_vocabulary…` (`grant_set.rs:221-233`) | it builds on `declared()`, so the expected text gains the `access …; ` prefix |
| `confined_restricts_the_surface_and_imposes_both_ceilings` (`policy.rs`) | it asserts `grants().is_empty()` |
| `the_report_states_the_grants_the_policy_extends` (`doctor.rs:99-101`) | `confined` now prints the access list |
| `ext_doctor.rs:509`, `:514` | whole-output comparisons with `grants:       none` under a confined ceiling |
| the `eval` helpers on `Policy::pure()` at `time.rs:168`, `stdio.rs:199`, `path.rs:630` | they test `time.now`/`monotonic`, stdio and `path.absolute`, which `pure` now refuses. Each moves to `Policy::pure().with_grants(GrantSet::sealed().with_access(..))`, granting the member under test |

The other `Policy::pure()` helpers (`hash.rs:143`, `glob.rs:157`, `regex.rs:254`, `:412`,
`builder.rs`, `engine.rs`) call no gated function, except where §5.5 changes a refusal path. Any of
them that asserts a resolved path in a no-roots refusal changes with rule 2.

## 12. Documentation

This follows the airsl Book's rules (`CLAUDE.md` § Documentation). Every snippet and transcript is
run against the release build and pasted.

| Item | Change |
|---|---|
| `ch09-03-what-it-promises.md` | What `pure` guarantees (see the list below this table) |
| `ch03-05-presets.md`, `appendix-03-presets.md` | `pure`: no `print`, a repeatable `math.random`, no `randomseed()`, no host access. A `require` row covering `pure` both without and with the grant |
| `ch03-01-nothing-by-default.md`, `ch06-03-granting.md` | Host access: the seven members, `sealed()` versus `declared()`, `--allow-access`, building a `pure`-based policy on `sealed()`, and that host access bounds host modules only (§3.1). The sentence at `ch06-03-granting.md:30` ("`GrantSet::declared()` starts with nothing granted") is rewritten |
| `appendix-02-modules.md` | The "No grant needed." lines under `path` (`:34`), `time` (`:100`), `stdio` (`:120`) and `hook` (`:132`) now name each function's member |
| `ch05-02-files-and-paths.md`, `ch05-03-time-and-hashing.md`, `ch05-04-streams-and-hooks.md` | Each function's member and its refusal text |
| `ch02-00-first-project.md`, `ch06-02-scripts-and-values.md` | `require` needs the `require` member, and `confined` holds it |
| `ch08-02-negotiation.md` | An extension gets the ceiling's access members; `ext doctor`'s `grants:` line |
| `ch01-03-airsl-doctor.md`, `appendix-01-cli.md` | `--allow-access` on the five subcommands (§8); `doctor` taking the grant flags; the new `grants:` line; that `--allow-access` cannot change `ext`'s confined ceiling |
| **new** `appendix-06-surfaces.md`, "F. Lua Functions by Surface" | Every base global and library function each surface loads, and which preset uses which surface. Linked from `SUMMARY.md` after E, and from `appendix-00.md` (`:6-10`). How it is generated is described below this table |
| `CHANGELOG.md` | An `airsl 0.2.0 — airsl-cli 0.2.0` entry: Breaking (`pure`); Added (`HostAccess`, `sealed`, `--allow-access`, `doctor` flags, `RequireDenied`); Changed (§14) |

**What `ch09-03` guarantees.** Unless a member is granted, a `pure` script reaches no stdin,
stdout or stderr, no clock, no cwd, no host filesystem and no zone database. **On a fresh engine**,
the same script and input give the same output, with three exceptions:

- `pairs` order over non-sequence keys;
- the addresses that `tostring` and `%p` print;
- the order `table.sort` gives elements its comparator treats as equal. A comparator that breaks
  every tie gives one order.

The chapter also says that a reused engine carries globals and the random sequence between
evaluations, and that granting `require` reveals the absolute location of the script's folder.

**How appendix F is generated.** Run `probes/globals.lua` against the release build under
`trusted`, `confined`, and `pure --allow-access stdout` (the script prints through `stdio.write`),
then paste the output. Two `pure` entries are noted under the table:

- `require` appears as a function: it is the refusing stub that host access puts there;
- `math.randomseed` is airsl's wrapper (§4.2).

Examples live in `crates/airsl/examples` and are run in CI by `cargo make examples`
(`ci.yml:90-91`):

| Example | What breaks | Change |
|---|---|---|
| `text-toolkit` | `main.rs:23` runs `toolkit.lua` under `Policy::pure()`, and the script calls `print` and `time.monotonic` (`:140`) | `toolkit.lua` writes through `airsstack.stdio.write`, and `main.rs` runs it under `Policy::pure().with_grants(GrantSet::sealed().with_access(HostAccess::Stdout).with_access(HostAccess::Clock))`. `README.md:3`, `README.md:101` and `examples/README.md:52` stop saying it grants nothing |
| `multi-file-project` | `main.rs:61` runs `which_require.lua` under `Policy::pure()`; the script calls `print` and treats `require == nil` as "absent" | `which_require.lua` writes through `stdio.write`, and recognises the refusing `require` by its `require denied:` text. `main.rs` grants `stdout` on its `pure` row, and adds a `pure` row with `require` granted |
| `policy-presets` | `main.rs:28`, `:41` | Its printed output changes (`grants=access …` under `confined`, and the `pure` probe line); its README transcript is re-run |
| `hello-eval` | `README.md:20` says `grants: none`; `:37` says the policy "grants nothing" | The transcript is re-run and the sentence rewritten |

The plan re-runs every other example and README transcript against the release build, and rewrites
whatever changed.

## 13. Release

`airsl` and `airsl-cli` go from 0.1.6 to 0.2.0 and are published together, library first
(`CLAUDE.md`; the intent: the binary's lockfile pins the library). The jump to 0.2 is required,
because `pure` scripts break and the documented meaning of `GrantSet::declared()` changes.

## 14. What changes for `trusted` and `confined`

Scripts under `trusted` and `confined` run as on 0.1.6: every member is granted, and `require`,
`print` and the random seeding are unchanged. These observable differences remain:

| Change | Who sees it |
|---|---|
| A no-roots refusal names the path as the script passed it, not as resolved: `/etc/hosts` rather than `/private/etc/hosts`, and `p.lua` rather than the path joined to the cwd | a `confined` script that reads a refusal's text while no fs roots are granted (the author's decision) |
| With no roots, a path that used to fail resolution with `Error::UncheckablePath` now gets `Error::Denied` (rule 2). This covers the reasons "the working directory could not be read", "it climbs through a directory that does not exist" and "no part of it exists". The lexical `UncheckablePath` cases (NUL, verbatim, device) are unchanged | a `confined` caller that matches the error variant or text for those inputs while no fs roots are granted |
| The `grants:` lines of `doctor` and `ext doctor` read `access stdin,stdout,stderr,clock,cwd,require,zones…` instead of `none` | CLI users, and tests that match that output |
| `ext.granted()` gains an `access` key | scripts that iterate its keys |
| `GrantSet::declared().is_empty()` is `false`, and `GrantSet::declared()` and `default()` hold members | Rust callers that relied on "declared grants nothing" |
| `HostAccess`, `GrantSet::sealed`/`with_access`/`allows`/`access` and `Error::RequireDenied` are new | additive (`#[non_exhaustive]`); every `const fn` stays `const` |

## 15. Risks

- **A `pure` embedder that rebuilds grants on `declared()` or `default()`** reopens all seven
  members. This follows from the author's placement choice (§3.2). Mitigation: the `pure()` and
  `declared()` rustdoc, and the book's `ch06-03` example, which builds on `sealed()`.
- **Windows path grammar.** Rule 1 and `path.absolute` rely on `Path::is_absolute`, whose Windows
  behaviour is cited from `std`'s documentation (E18), not run here. The CI Windows job
  (`ci.yml:38`, `:149`, `windows-latest`) runs the inline tests that exercise it.

## 16. Non-goals

- Changing what a `trusted` or `confined` script can do. The refusal changes in §14 are the only
  exception.
- Making `pairs` order, the addresses `tostring` and `%p` print, or the order of `table.sort` ties
  deterministic. These come from the VM's hash seed, its allocator and its pivot choice (E6, E13,
  E14), and the book names them (§12).
- A policy-dependent module set (`stdlib_for(&Policy)`).
- Manifest fields for host access, and a `without_*` API.
- `doctor` gaining `--memory-limit`/`--instruction-limit`; it takes the grant flags only.
- Letting `ext doctor`/`ext fire` choose a ceiling other than `confined`.
- Bumping agentstacks or buildl, which happens in their own repos after 0.2.0 is published.
- Bundling a tzdb into airsl.
