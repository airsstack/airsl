# Scripts, Arguments and Values

Real scripts live in files. This example loads one from disk, passes it arguments, and reads back a
structured result.

Save the script next to your `Cargo.toml` as `scripts/summary.lua`:

```lua
-- scripts/summary.lua
local words = 0
for _ in arg[1]:gmatch("%S+") do
  words = words + 1
end

return airsstack.json.encode({ words = words, title = arg[2] })
```

And the program:

```rust
use airsl::{Engine, Policy, Script};

fn main() -> Result<(), airsl::Error> {
    let engine = Engine::builder().policy(Policy::confined()).build()?;

    let script = Script::from_file("scripts/summary.lua")?
        .with_args(["the quick brown fox", "Animals"]);

    let json: String = engine.eval_to(&script)?;
    println!("{json}");
    Ok(())
}
```

```console
$ cargo run
{"title":"Animals","words":4}
```

## Arguments

`with_args` fills the script's `arg` table, exactly as `airsl run` does from the command line. The
first value becomes `arg[1]`.

## Files and `require`

A script loaded with `Script::from_file` knows which folder it lives in, so it can `require` the
modules next to it, as you saw in [chapter 2](ch02-00-first-project.md).

A script built with `Script::from_source` has no folder, so it cannot `require` anything. If it needs
to, give it one with `.with_root(dir)`.

## Getting structured data back

The simplest way to return structured data is to have the script encode it as JSON, then decode it
on the Rust side with `serde_json`. JSON keys come out sorted, so the same result always produces the
same string, which also makes results easy to compare in tests.

For a few plain values, return them directly and let `eval_to` take a tuple:

```rust
# use airsl::{Engine, Policy, Script};
# fn main() -> Result<(), airsl::Error> {
# let engine = Engine::builder().policy(Policy::confined()).build()?;
let script = Script::from_source("return 'ok', 3", "pair")?;
let (status, count): (String, i64) = engine.eval_to(&script)?;
# assert_eq!((status.as_str(), count), ("ok", 3));
# Ok(())
# }
```
