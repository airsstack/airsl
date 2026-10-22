# Your First Engine

An **engine** is a Lua interpreter together with its rules. You build one, then hand it scripts to
run.

```rust
use airsl::{Engine, Policy, Script};

fn main() -> Result<(), airsl::Error> {
    let engine = Engine::builder().policy(Policy::confined()).build()?;

    let script = Script::from_source("return 6 * 7", "answer")?;
    let answer: i64 = engine.eval_to(&script)?;

    println!("the answer is {answer}");
    Ok(())
}
```

```console
$ cargo run
the answer is 42
```

## You cannot forget the policy

Look at the builder again: `Engine::builder().policy(...).build()`. The `policy` call is not
optional. If you leave it out, there is no `build` method to call, and the program does not compile.

```rust,ignore
let engine = Engine::builder().build()?;
// error[E0599]: no method named `build` found for struct `EngineBuilder<Missing>`
```

So "did I remember to sandbox this?" is a question your program can never get wrong. Every engine
has a policy, because there is no way to make one without.

`Policy::confined()`, `Policy::trusted()` and `Policy::pure()` are the same three presets you used
with `--policy` in [chapter 3](ch03-05-presets.md).

## Two ways to run a script

| Method | Use it when |
|---|---|
| `engine.eval(&script)` | you run the script for what it does, and ignore what it returns |
| `engine.eval_to::<T>(&script)` | you want the value the script returns, converted to `T` |

`T` can be any type Lua values convert into: `String`, `i64`, `f64`, `bool`, `Option<_>`, or a tuple
for a script that returns several values.

`Script::from_source` takes the code and a name. The name appears in error messages, so pick one
that will tell you where the script came from.
