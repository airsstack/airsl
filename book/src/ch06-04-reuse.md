# One Engine, Many Scripts

Building an engine is not free. It starts a Lua interpreter and installs twelve modules into it. Running
a small script on an engine that already exists is about thirty times faster than building a new
one for it.

So if your program runs scripts often, such as once per request or once per keystroke, build the
engine once and keep it:

```rust
use airsl::{Engine, Policy, Script};

fn main() -> Result<(), airsl::Error> {
    let engine = Engine::builder().policy(Policy::confined()).build()?;
    let double = Script::from_source("return tonumber(arg[1]) * 2", "double")?;

    let mut total = 0;
    for n in 1..=3 {
        total += engine.eval_to::<i64>(&double.clone().with_args([n.to_string()]))?;
    }

    println!("total: {total}");
    Ok(())
}
```

```console
$ cargo run
total: 12
```

## What starts fresh, and what carries over

When you reuse an engine, some things are reset for each script and some are kept:

| Fresh for every script | Kept between scripts |
|---|---|
| the instruction limit, which starts again from zero | modules loaded with `require`, so each is loaded once |
| the `arg` table | global variables a script set |
| the folder `require` looks in | memory not yet freed by Lua's garbage collector |

The right-hand column is the part to watch. If one script sets a global variable, the next script
on the same engine sees it. That is useful for a set of scripts that work together, and a problem for
scripts that should not affect each other.

The rule is simple: **share an engine only between scripts that trust each other.** Give scripts
from different sources their own engines.

## Sharing an engine between threads

`Engine` is `Send + Sync`, so you can put it in an `Arc` and use it from several threads:

```rust
use std::sync::Arc;
use std::thread;

use airsl::{Engine, Policy, Script};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Arc::new(Engine::builder().policy(Policy::confined()).build()?);

    let handles: Vec<_> = (0..4)
        .map(|n| {
            let engine = Arc::clone(&engine);
            thread::spawn(move || -> Result<i64, airsl::Error> {
                let script = Script::from_source("return tonumber(arg[1])", "echo")?
                    .with_args([n.to_string()]);
                engine.eval_to(&script)
            })
        })
        .collect();

    for handle in handles {
        println!("{}", handle.join().expect("thread panicked")?);
    }
    Ok(())
}
```

Each script still gets its own arguments, because the engine runs one script at a time and makes the
others wait. That is the thing to understand about sharing: it saves you building a new engine, but
it does not make scripts run in parallel. A single Lua interpreter can only do one thing at a time,
however you hold it. If you need scripts to run in parallel, give each thread its own engine.
