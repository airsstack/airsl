# Granting from Rust

Every flag from [chapter 3](ch03-00-asking-permission.md) has a Rust equivalent. This example
grants a script one folder to read, one to write, one environment variable and one program:

```rust
use airsl::{Engine, GrantSet, Policy};

fn main() -> Result<(), airsl::Error> {
    let policy = Policy::confined().with_grants(
        GrantSet::declared()
            .with_fs(|fs| fs.read("/srv/app/config").write("/srv/app/cache"))
            .with_env(|env| env.read(["APP_ENV"]))
            .with_proc(|proc| proc.allow(["git"])),
    );

    let engine = Engine::builder().policy(policy).build()?;
    # let _ = engine;
    Ok(())
}
```

That is the same as this command line:

```console
$ airsl run --allow-read /srv/app/config --allow-write /srv/app/cache \
            --allow-env APP_ENV --allow-exec git script.lua
```

`GrantSet::declared()` starts with nothing granted. Each `with_` call adds one kind of grant:

| Method | Grants | Command-line flag |
|---|---|---|
| `with_fs(\|fs\| fs.read(dir))` | reading under `dir` | `--allow-read` |
| `with_fs(\|fs\| fs.write(dir))` | writing under `dir` | `--allow-write` |
| `with_env(\|env\| env.read(names))` | using those variables | `--allow-env` |
| `with_proc(\|proc\| proc.allow(names))` | running those programs | `--allow-exec` |

The calls chain, so `fs.read(a).read(b).write(c)` grants two read folders and one write folder.

## Changing the limits

Limits work the same way, through `with_limits`:

```rust
use airsl::{InstructionLimit, MemoryLimit, Policy, ResourceLimits};

let policy = Policy::confined().with_limits(ResourceLimits::new(
    Some(MemoryLimit::mebibytes(8)),
    Some(InstructionLimit::count(1_000_000)),
));
# let _ = policy;
```

Pass `None` for a limit to remove it, or start from `ResourceLimits::none()` for no limits at all.

## Checking what you built

An engine can tell you its own policy. The output is the same text `airsl doctor` prints:

```rust
# use airsl::{Engine, Policy};
# fn main() -> Result<(), airsl::Error> {
# let engine = Engine::builder().policy(Policy::confined()).build()?;
println!("language: {}", engine.policy().language());
println!("grants:   {}", engine.policy().grants());
# Ok(())
# }
```

This is worth doing in your program's own diagnostics. When a user asks why their script was
refused, the answer is whatever this prints.
