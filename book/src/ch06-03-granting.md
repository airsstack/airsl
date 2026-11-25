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

`GrantSet::declared()` starts with all seven host access members and no folder, variable or
program. It is the baseline `confined` uses. Each `with_` call adds one kind of grant:

| Method | Grants | Command-line flag |
|---|---|---|
| `with_fs(\|fs\| fs.read(dir))` | reading under `dir` | `--allow-read` |
| `with_fs(\|fs\| fs.write(dir))` | writing under `dir` | `--allow-write` |
| `with_env(\|env\| env.read(names))` | using those variables | `--allow-env` |
| `with_proc(\|proc\| proc.allow(names))` | running those programs | `--allow-exec` |

The calls chain, so `fs.read(a).read(b).write(c)` grants two read folders and one write folder.

## Host access

[Chapter 3](ch03-01-nothing-by-default.md) introduced the seven host access members. In Rust they
are the variants of `HostAccess`: `Stdin`, `Stdout`, `Stderr`, `Clock`, `Cwd`, `Require` and
`Zones`. `with_access` adds one, and `allows` asks whether a grant set holds one. It matches the
`--allow-access` flag.

Two constructors give you a starting point:

| Constructor | Holds | Use it for |
|---|---|---|
| `GrantSet::declared()` | all seven members, and no folder, variable or program | `confined`, and anything built on it |
| `GrantSet::sealed()` | nothing at all | `pure`, and anything built on it |

`GrantSet::default()` is the same as `declared()`.

To widen `Policy::pure()`, start from `sealed()` and add what you want. Starting from `declared()` or
`default()` would hand back all seven members, and the policy would no longer be `pure`. There is no
method to take a member away, so build upward from `sealed()`:

```rust
use airsl::{Engine, GrantSet, HostAccess, Policy, Script};

fn main() -> Result<(), airsl::Error> {
    // `sealed()` holds no host access, so this policy has exactly what it names.
    let policy = Policy::pure().with_grants(
        GrantSet::sealed()
            .with_access(HostAccess::Stdout)
            .with_access(HostAccess::Clock),
    );
    let engine = Engine::builder().policy(policy).build()?;

    let script = Script::from_source(
        r#"
        airsstack.stdio.write("clock works: " .. tostring(airsstack.time.now() > 0) .. "\n")
        local ok, err = pcall(airsstack.stdio.error, "to stderr\n")
        airsstack.stdio.write(tostring(err):match("^[^\n]*") .. "\n")
        "#,
        "demo",
    )?;
    engine.eval(&script)?;

    println!("grants: {}", engine.policy().grants());
    Ok(())
}
```

```console
$ cargo run
clock works: true
stdio.error denied: `stderr` is not granted — the granted access is stdout, clock
grants: access stdout,clock
```

The script can write and read the clock, and `stderr` is refused with a message naming what it does
hold.

### Host access does not reach Lua's own functions

Host access bounds the `airsstack` modules and the confined `require`. The parts of Lua that a
preset keeps, such as `print` and `os.time` on `confined`, do not ask it. This engine has the
`confined` preset and a grant set that holds nothing:

```rust
use airsl::{Engine, GrantSet, Policy, Script};

fn main() -> Result<(), airsl::Error> {
    let policy = Policy::confined().with_grants(GrantSet::sealed());
    let engine = Engine::builder().policy(policy).build()?;

    let script = Script::from_source(
        r#"
        print("print still works")
        print("os.time still works: " .. tostring(os.time() > 0))
        local _, err = pcall(airsstack.time.now)
        print(tostring(err):match("^[^\n]*"))
        "#,
        "surface",
    )?;
    engine.eval(&script)
}
```

```console
$ cargo run
print still works
os.time still works: true
time.now denied: `clock` is not granted — no access is granted
```

To keep a script from printing or reading the clock through Lua itself, use `pure`, whose surface
has neither.

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
