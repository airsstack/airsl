# values-and-json

Moves values across the Rust/Lua boundary, and shows that the JSON coming back is byte-stable.

This gets its own example because byte-stability is a correctness property of the crate rather than
a formatting preference, and the only way to demonstrate it is to encode the same table twice and
compare. A snippet can assert that JSON came out; it cannot assert that the same input produced the
same bytes.

## Run

```bash
cargo run -p airsl --example values-and-json
```

## Output

```
compact: {"count":3,"meta":{},"name":"widget","observed_at":1700000000,"tags":["beta","alpha"]}
pretty:
{
  "count": 3,
  "meta": {},
  "name": "widget",
  "observed_at": 1700000000,
  "tags": [
    "beta",
    "alpha"
  ]
}
decoded back: name=widget observed_at=1700000000 tags=2
```

## What it demonstrates

- **Arguments arrive in Lua's own `arg` table.** `Script::with_args` (`src/script.rs:130`) carries
  them on the script rather than installing them into the state, so one engine can run two scripts
  and give each the arguments it was built with. A ported shell script reads `arg[1]` where it read
  `$1`.
- **Object keys are sorted; array elements are not.** Lua iterates a table in hash order, which
  varies between runs, so encoding straight through `serde_json` produced different bytes each
  time. `sorted` (`src/convert.rs:34`) makes sorting the behaviour rather than an option — which is
  what lets a script write an index or a lockfile that diffs cleanly. `tags` comes back
  `["beta","alpha"]`, not `["alpha","beta"]`: element order is data the script chose, while key
  order was never anything but an artefact of hashing. The example asserts the two encodings of the
  same table are identical.
- **An empty Lua table encodes as `{}`.** Lua has no distinct empty-sequence value, so there is
  nothing to disambiguate an empty object from an empty array with (`src/convert.rs:41`).
- **Lua 5.4's integers survive the round trip.** `observed_at` stays `1700000000` rather than
  becoming `1.7e9`. Only Lua 5.3+ distinguishes integers from floats, which is why the crate pins
  5.4 rather than the more commonly embedded 5.1 or LuaJIT.
- **`encode_pretty` ends with a newline** (`src/convert.rs:64`, `src/modules/json.rs:69`), so a
  script can write it straight to a file without appending one.
- **A returned table is read through `mlua`.** `eval_to::<T>` (`src/engine.rs:237`) converts through
  `FromLuaMulti`, so `airsl::mlua::Table` is a legitimate target. Depend on `airsl::mlua`, never on
  a separately declared `mlua` — a version mismatch produces type errors that never name the real
  cause.

`Script::from_file` reading `encode.lua` is a **host** read and is not governed by `FsGrant`. The
grant governs `airsstack.fs.*` calls made *by* the script, which is why this example loads a file
from disk while running on the confined preset's empty grant set.

## See also

- [hello-eval](../hello-eval/) — the smaller version, with the Lua inline.
- [stdlib](../../docs/stdlib.md) — the full `airsstack.json` roster.
