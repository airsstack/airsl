# Writing Your Own Module

The twelve built-in modules cover what most scripts need. But your program almost certainly has
things of its own that scripts should be able to reach: its configuration, its data, its way of
logging. A **host module** is how you give them that, without handing over anything more.

In this chapter you will write a module called `notes`. Scripts call `notes.add("...")`, and your
Rust program collects what they added.

```
  Lua script                             Rust program
  ──────────                             ────────────
  myapp.notes.add("disk is 91% full")  ──►  Vec<String> ["disk is 91% full"]
```

## The module

A host module is a type that implements the `HostModule` trait. The trait has two methods: `name`,
which says what the module is called, and `install`, which puts its functions into a Lua table.

```rust
use std::sync::{Arc, Mutex};

use airsl::mlua::{Lua, Table};
use airsl::{HostModule, InstallContext, ModuleName};

struct Notes {
    name: ModuleName,
    collected: Arc<Mutex<Vec<String>>>,
}

impl HostModule for Notes {
    fn name(&self) -> &ModuleName {
        &self.name
    }

    fn install(&self, lua: &Lua, table: &Table, _context: &InstallContext<'_>) -> airsl::Result<()> {
        let fail = |e| airsl::Error::lua("notes", e);

        let collected = Arc::clone(&self.collected);
        let add = lua
            .create_function(move |_, text: String| {
                collected.lock().expect("notes lock poisoned").push(text);
                Ok(())
            })
            .map_err(fail)?;

        table.set("add", add).map_err(fail)
    }
}
```

`install` receives the Lua state, the table that will become `myapp.notes`, and a context that this
module does not need yet. It creates a Rust function and stores it in the table under the name
`add`. When a script calls `notes.add("...")`, Lua calls that closure with the string.

`airsl::mlua` is the Lua binding `airsl` itself uses. Always import it from `airsl` rather than
adding `mlua` to your own `Cargo.toml`. If the two versions ever differ, the compiler reports type
errors that never mention the real cause.

## Installing it

Start from the built-in modules, add yours, and give the engine its own root table name:

```rust
# use std::sync::{Arc, Mutex};
# use airsl::mlua::{Lua, Table};
# use airsl::{HostModule, InstallContext, ModuleName};
# struct Notes { name: ModuleName, collected: Arc<Mutex<Vec<String>>> }
# impl HostModule for Notes {
#     fn name(&self) -> &ModuleName { &self.name }
#     fn install(&self, lua: &Lua, table: &Table, _context: &InstallContext<'_>) -> airsl::Result<()> {
#         let fail = |e| airsl::Error::lua("notes", e);
#         let collected = Arc::clone(&self.collected);
#         let add = lua.create_function(move |_, text: String| {
#             collected.lock().expect("notes lock poisoned").push(text);
#             Ok(())
#         }).map_err(fail)?;
#         table.set("add", add).map_err(fail)
#     }
# }
use airsl::{Engine, Policy, RootTable, Script};

fn main() -> Result<(), airsl::Error> {
    let collected = Arc::new(Mutex::new(Vec::new()));

    let mut modules = airsl::modules::stdlib()?;
    modules.insert(Box::new(Notes {
        name: ModuleName::new("notes")?,
        collected: Arc::clone(&collected),
    }))?;

    let engine = Engine::builder()
        .policy(Policy::confined())
        .root_table(RootTable::new("myapp")?)
        .stdlib(modules)
        .build()?;

    let script = Script::from_source(
        r#"
          myapp.notes.add("disk is 91% full")
          myapp.notes.add("backup finished in " .. 42 .. "s")
        "#,
        "report",
    )?;
    engine.eval(&script)?;

    for note in collected.lock().expect("notes lock poisoned").iter() {
        println!("note: {note}");
    }
    Ok(())
}
```

```console
$ cargo run
note: disk is 91% full
note: backup finished in 42s
```

Two things happened here besides adding the module.

**`airsl::modules::stdlib()` is the list of built-ins.** You add to it, so scripts keep `json`,
`regex` and the rest alongside your module. You could also start from an empty `ModuleSet` and offer
only your own.

**`RootTable::new("myapp")` renamed the global.** Scripts now write `myapp.notes.add` and
`myapp.json.encode`, instead of `airsstack.…`. Your users are writing scripts for *your* program, so
the global should carry your program's name.

## A module that asks permission

`notes` needs no permission, because collecting strings reaches nothing. A module that does reach
something, such as one that writes to a database or calls an API, should check a grant first, just
as `fs` does.

The `InstallContext` that `install` receives carries the engine's policy. Read the grants from it:

```rust,ignore
fn install(&self, lua: &Lua, table: &Table, context: &InstallContext<'_>) -> airsl::Result<()> {
    let may_write = !context.grants().fs().write_roots().is_empty();
    // ...create functions that refuse when `may_write` is false
}
```

Two rules keep a module like this honest:

1. **Check inside the Rust function, before doing the work.** Never trust the script to check for
   itself. A check written in Lua is one a script can skip.
2. **Read the grants from the context.** Do not keep a separate copy. The context holds the policy
   the engine was built with, the same one `engine.policy()` and `airsl doctor` report, so what your
   module enforces can never drift from what your program says it enforces.

And when the module refuses, raise an error that names what was granted, so the person reading it
knows what to change. The [`custom-module` example](https://github.com/airsstack/airsl/tree/main/crates/airsl/examples/custom-module)
shows a complete module that does this.

## Summary

- A host module implements `HostModule`: a `name`, and an `install` that fills a Lua table.
- Import `mlua` through `airsl::mlua`.
- Add your module to `airsl::modules::stdlib()` to keep the built-ins.
- Name the root table after your program with `RootTable`.
- A module that reaches outside the script checks its grant in Rust, from the `InstallContext`.
