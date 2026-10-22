# airsl

Run Lua scripts from Rust, and decide what each one may reach.

`airsl` embeds Lua 5.4 in your program. Every engine is built with a policy: which folders a script
may read or write, which environment variables and programs it may use, and how much memory and time
it may spend. Scripts get JSON, files, regular expressions, hashing, time and subprocesses through
one `airsstack` table, and every call is checked against that policy in Rust before it runs.

```rust
use airsl::{Engine, Policy, Script};

fn main() -> Result<(), airsl::Error> {
    let engine = Engine::builder().policy(Policy::confined()).build()?;

    let script = Script::from_source("return airsstack.json.encode({ ok = true })", "demo")?;
    let json: String = engine.eval_to(&script)?;

    assert_eq!(json, r#"{"ok":true}"#);
    Ok(())
}
```

## Granting access

Nothing is granted unless you say so. Here a script may read the project, write only its `build`
folder, read `HOME`, and run `git`:

```rust
use airsl::{Engine, GrantSet, Policy};

fn main() -> Result<(), airsl::Error> {
    let policy = Policy::confined().with_grants(
        GrantSet::declared()
            .with_fs(|fs| fs.read("/repo").write("/repo/build"))
            .with_env(|env| env.read(["HOME"]))
            .with_proc(|proc| proc.allow(["git"])),
    );
    let _engine = Engine::builder().policy(policy).build()?;
    Ok(())
}
```

Anything else is refused with an error that names what was granted, so a script that asks for the
wrong folder tells you which folder it was allowed.

## Telling a stopped script from a failed one

A script that loops forever or allocates without end is stopped by the engine. That is reported
differently from an ordinary error, and a script cannot fake it:

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

## Learn more

[The airsl Book](https://airsstack.github.io/airsl/) teaches all of this step by step:

- [Embedding airsl in Rust](https://airsstack.github.io/airsl/ch06-00-embedding.html): engines,
  values, grants, reuse and failures
- [Writing Your Own Module](https://airsstack.github.io/airsl/ch07-00-your-own-module.html): give
  scripts a capability of your own
- [Extensions](https://airsstack.github.io/airsl/ch08-00-extensions.html): load third-party scripts
  with permissions negotiated against a ceiling you set
- [Module Reference](https://airsstack.github.io/airsl/appendix-02-modules.html): every function a
  script can call

The API reference is on [docs.rs](https://docs.rs/airsl). Runnable examples live in
[`examples/`](https://github.com/airsstack/airsl/tree/main/crates/airsl/examples).

## Building

`airsl` builds on Linux, macOS and Windows (`x86_64-pc-windows-msvc`). It needs a C compiler: Lua is
compiled from its C source and linked in statically, so there is no system Lua to install.

## Releases

See [CHANGELOG.md](https://github.com/airsstack/airsl/blob/main/CHANGELOG.md). This crate and
`airsl-cli` are versioned independently and share one changelog.
