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
trusted  airsstack=yes coroutine=yes io=yes load=yes os=yes package=yes require=yes string=yes utf8=yes | os.time=yes os.getenv=yes
confined airsstack=yes coroutine=yes io=no load=no os=yes package=no require=yes string=yes utf8=yes | os.time=yes os.getenv=no
pure     airsstack=yes coroutine=no io=no load=no os=no package=no require=yes string=yes utf8=yes | os.time=no os.getenv=no

trusted  surface=full grants=unrestricted memory=unbounded instructions=unbounded
confined surface=restricted grants=access stdin,stdout,stderr,clock,cwd,require,zones memory=67108864 bytes instructions=100000000 instructions
pure     surface=minimal grants=none memory=16777216 bytes instructions=10000000 instructions
```

## What it demonstrates

- **A preset decides all three axes at once.** `Policy::trusted()` is not merely a wider language
  surface — it also carries unrestricted grants and no ceilings (`Policy::trusted` in
  `src/sandbox/policy.rs`).
  Reading only the first row would make it look like a smaller step than it is.
- **`airsstack` is on every surface.** The host modules are installed regardless: the language
  surface governs *Lua's own* libraries, while what a host module may touch is the grant set's
  question and an entirely separate one. A module is present even when ungranted — see
  [denials-are-data](../denials-are-data/).
- **`os` is narrowed, not removed, on `confined`.** The table survives with the functions that
  reach outside the process taken out of it — `execute`, `exit`, `getenv`, `remove`, `rename`,
  `tmpname`, `setlocale` (`UNSAFE_OS_FUNCTIONS` in `src/sandbox/language_surface.rs`). Probing only
  for `os` would report it intact while `os.getenv` had in fact been withheld, which is why
  `probe.lua` checks the fields as well as the table.
  `setlocale` is on that list for a subtler reason than the rest: Lua compares strings with
  `strcoll`, so a script that changed the locale would change the sort order of every subsequent
  `table.sort`.
- **`pure` drops `os` entirely** (`LanguageSurface::Minimal` in
  `src/sandbox/language_surface.rs`). `os.time` and `os.clock` are the last nondeterminism a script
  can reach without going through a host module — they return something different on every run —
  and `pure` exists to be the configuration whose guarantees are easiest to state.
- **`utf8` survives every surface**, `pure` included (`LanguageSurface::libraries` in
  `src/sandbox/language_surface.rs`). The criterion is authority and nondeterminism, not size:
  `utf8` is arithmetic over an encoding. Without it `#s` counts bytes with no way to count
  characters, so a confined script could not truncate text without risking a cut through the middle
  of one.
- **The chunk loaders go everywhere below `full`.** `load`, `loadstring`, `dofile` and `loadfile`
  (`CHUNK_LOADERS` in `src/sandbox/language_surface.rs`) each hand a script a second way to load
  code, bypassing both the surface and the confined `require`.

### `require=yes` on all three rows means three different functions

Every row reports a `require`, and they are not the same one.
`RequireDisposition::decide` (in `src/require/disposition.rs`) is the four-way answer: `Full` keeps
Lua's own loader, which resolves through `package.path` and can reach anywhere on the filesystem.
Below `Full` the answer follows the `require` host access. `confined` holds it and a script read
from a file gets a Rust function confined to the script's own directory, with no `package` table to
configure and no way to widen it. `pure` does not hold it, so its `require` is a function that
refuses every call except `require("airsstack")` with `require denied:` and a message naming what
was granted. A script built from source has no directory to confine a loader to, so a policy that
holds the access still gives it a `require` that answers only the root table. `probe.lua` can only
see that the global is non-`nil`, so this is the one place the output understates a difference
rather than showing it — [`multi-file-project`](../multi-file-project/) runs
`require("../secrets")` under four policies, and `require("lib")` from a script built from source,
and prints four different kinds of answer.

The match in `decide` is exhaustive, so a new surface cannot acquire an answer by default.

## See also

- [Presets](https://airsstack.github.io/airsl/ch03-05-presets.html) — the three presets side by side, from the command line.
- [resource-limits](../resource-limits/) — the ceilings in the second block, made to fire.
- [`multi-file-project`](../multi-file-project/) — that confined `require`, given a module tree to
  resolve against.
- [Three Layers](https://airsstack.github.io/airsl/ch09-01-three-layers.html) — the language surface, the grants and the
  limits as three separate questions a policy answers.
