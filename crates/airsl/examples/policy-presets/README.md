# policy-presets

Runs one unchanged script under `trusted`, `confined` and `pure`, and prints what each one can see.

This gets its own example because the language surface is the axis of the policy that is hardest to
observe from outside. A grant refusal announces itself with a message naming what *was* granted; a
withheld standard library is just a `nil` the script finds when it looks. Two presets side by side
is the only way to make that visible — which is why `probe.lua` is a single file run three times
rather than three scripts. Nothing in it asks what policy it is under, so every difference in the
output below is the sandbox and not the script.

## Run

```bash
cargo run -p airsl --example policy-presets
```

## Output

```
trusted  airsstack=yes coroutine=yes io=yes load=yes os=yes package=yes require=no string=yes utf8=yes | os.time=yes os.getenv=yes
confined airsstack=yes coroutine=yes io=no load=no os=yes package=no require=yes string=yes utf8=yes | os.time=yes os.getenv=no
pure     airsstack=yes coroutine=no io=no load=no os=no package=no require=no string=yes utf8=yes | os.time=no os.getenv=no

trusted  surface=full grants=unrestricted memory=unbounded instructions=unbounded
confined surface=restricted grants=none memory=67108864 bytes instructions=100000000 instructions
pure     surface=minimal grants=none memory=16777216 bytes instructions=10000000 instructions
```

## What it demonstrates

- **A preset decides all three axes at once.** `Policy::trusted()` is not merely a wider language
  surface — it also carries unrestricted grants and no ceilings (`src/sandbox/policy.rs:61`).
  Reading only the first row would make it look like a smaller step than it is.
- **`airsstack` is on every surface.** The host modules are installed regardless: the language
  surface governs *Lua's own* libraries, while what a host module may touch is the grant set's
  question and an entirely separate one. A module is present even when ungranted — see
  [denials-are-data](../denials-are-data/).
- **`os` is narrowed, not removed, on `confined`.** The table survives with the functions that
  reach outside the process taken out of it — `execute`, `exit`, `getenv`, `remove`, `rename`,
  `tmpname`, `setlocale` (`src/sandbox/language_surface.rs:28`). Probing only for `os` would report
  it intact while `os.getenv` had in fact been withheld, which is why `probe.lua` checks the fields
  as well as the table.
  `setlocale` is on that list for a subtler reason than the rest: Lua compares strings with
  `strcoll`, so a script that changed the locale would change the sort order of every subsequent
  `table.sort`.
- **`pure` drops `os` entirely** (`src/sandbox/language_surface.rs:86`). `os.time` and `os.clock`
  are the last nondeterminism a script can reach without going through a host module — they return
  something different on every run — and `pure` exists to be the configuration whose guarantees are
  easiest to state.
- **`utf8` survives every surface**, `pure` included (`src/sandbox/language_surface.rs:71`). The
  criterion is authority and nondeterminism, not size: `utf8` is arithmetic over an encoding.
  Without it `#s` counts bytes with no way to count characters, so a confined script could not
  truncate text without risking a cut through the middle of one.
- **The chunk loaders go everywhere below `full`.** `load`, `loadstring`, `dofile` and `loadfile`
  (`src/sandbox/language_surface.rs:20`) each hand a script a second way to load code, bypassing
  both the surface and the confined `require`.

### One thing the output contradicts

`require=no` under `trusted`, while `package=yes`. `RequireLoader::applies_to`
(`src/require_loader.rs:52`) documents `Full` as keeping Lua's own `require` — "deliberately left
alone — a first-party script may depend on it" — but `Engine::set_require` (`src/engine.rs:189`)
reaches its `_` arm for every surface other than `Restricted` and clears the global
(`src/engine.rs:194`), including the one the `package` library had just installed. So a `trusted`
script gets `package.path` and `package.loaded` but no `require` to use them with. The output above
is what the runtime does today, not what the doc comment describes.

## See also

- [sandbox](../../docs/sandbox.md) — the three questions a policy answers, at length.
- [resource-limits](../resource-limits/) — the ceilings in the second block, made to fire.
- [architecture](../../docs/architecture.md) — the confined `require` that `restricted` gets, and
  why `full` keeps Lua's own instead.
