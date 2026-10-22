# Handling Failures

When a script fails, `eval` and `eval_to` return an `airsl::Error`. Most of the time you print it
and move on. But one question is worth asking every time: did the script fail, or was it stopped?

```rust
use airsl::{Engine, ExhaustedLimit, Policy, Script};

fn main() -> Result<(), airsl::Error> {
    let engine = Engine::builder().policy(Policy::confined()).build()?;
    let script = Script::from_source("while true do end", "spin")?;

    if let Err(error) = engine.eval(&script) {
        match error.exhausted_limit() {
            Some(ExhaustedLimit::Instructions) => eprintln!("stopped: it never finished"),
            Some(ExhaustedLimit::Memory) => eprintln!("stopped: it used too much memory"),
            Some(_) => eprintln!("stopped: it hit a limit"),
            None => eprintln!("the script failed: {error}"),
        }
    }
    Ok(())
}
```

```console
$ cargo run
stopped: it never finished
```

The `Some(_)` arm is required. `ExhaustedLimit` is marked `#[non_exhaustive]`, which leaves room for
a new kind of limit in a future version without breaking your code.

`exhausted_limit` returns `Some` only when the engine itself stopped the script. That is worked out
from what the engine actually did, never from the error's text, so a script cannot pretend to have
hit a limit by raising an error with the right words in it, or hide a real one.

## Failing open from Rust

A program that runs scripts as hooks has the same problem `--fail-open` solves on the command line
([chapter 4](ch04-02-failing-safely.md)): a broken script must not break the program that ran it.
`FailurePolicy` names that choice:

| Variant | Means |
|---|---|
| `FailurePolicy::Report` | a failure is reported to whoever ran the script; the default |
| `FailurePolicy::FailOpen` | a failure is swallowed, because the caller must keep going |

Whichever you choose, a limit being hit is still worth reporting. The `airsl` command does exactly
that:

```rust
# use airsl::{Engine, FailurePolicy, Policy, Script};
# fn main() -> Result<(), airsl::Error> {
# let engine = Engine::builder().policy(Policy::confined()).build()?;
# let script = Script::from_source("error('boom')", "hook")?;
# let policy = FailurePolicy::FailOpen;
if let Err(error) = engine.eval(&script) {
    let stopped = error.exhausted_limit().is_some();
    if !policy.swallows_errors() || stopped {
        eprintln!("{error}");
    }
}
# Ok(())
# }
```

## Summary

- `Engine::builder().policy(...).build()` makes an engine, and it will not compile without a policy.
- `eval` runs a script and `eval_to` returns its value.
- `GrantSet` and `ResourceLimits` give you every grant and limit the command line has.
- Reuse engines between scripts that trust each other. It is much faster than building new ones.
- `error.exhausted_limit()` tells a stopped script apart from a failed one.

The next chapter goes one step further, and adds a module of your own to the `airsstack` table.
