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

A host module is a type that implements the `HostModule` trait. The trait has three methods: `name`,
which says what the module is called, `install`, which puts its functions into a Lua table, and
`describe`, which lists those functions so editors can complete and check them.

```rust
{{#rustdoc_include ../../crates/airsl/examples/notes-module/notes.rs:module}}
```

`install` receives the Lua state, the table that will become `myapp.notes`, and a context that this
module does not need yet. It creates a Rust function and stores it in the table under the name
`add`. When a script calls `notes.add("...")`, Lua calls that closure with the string.

`describe` says the same thing as data: `notes` has one function, `add`, that takes a string called
`text`. Nothing runs when it is called. It is the list that [Editor Support](ch01-04-editor-support.md)
turns into completion in your editor. It is a promise that has to stay true, so
[the last section](#keeping-the-description-honest) shows how to check it.

`airsl::mlua` is the Lua binding `airsl` itself uses. Always import it from `airsl` rather than
adding `mlua` to your own `Cargo.toml`. If the two versions ever differ, the compiler reports type
errors that never mention the real cause.

## Installing it

Start from the built-in modules, add yours, and give the engine its own root table name:

```rust
{{#rustdoc_include ../../crates/airsl/examples/notes-module/main.rs:run}}
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

## Keeping the description honest

A description that lists a function `install` never sets, or leaves one out, sends your editor and
your users the wrong way. `airsl::luals::verify` catches that. It installs the module into a scratch
Lua state, compares what it finds with what `describe` says, and returns an error that names the
difference. Call it from your own tests.

`airsl::luals::setup` does for your program what `airsl lua setup` does for the built-ins. Give it a
project folder, your module set and your root table, and it writes the description file your editor
reads, then adds it to the language server's settings file. The description file is named after the
root table, so here it is `.airsl/types/myapp.lua`.

```rust
{{#rustdoc_include ../../crates/airsl/examples/notes-module/setup.rs:setup}}
```

`cargo test` runs the check. `cargo run` in your project's folder sets it up:

```console
$ cargo test --quiet

running 1 test
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

$ cargo run --quiet
created .airsl/types/myapp.lua
created .luarc.json
```

`setup` checks every module with `verify` first, so a description that has drifted stops it before it
writes anything.

## Summary

- A host module implements `HostModule`: a `name`, an `install` that fills a Lua table, and a
  `describe` that lists what `install` sets.
- Import `mlua` through `airsl::mlua`.
- Add your module to `airsl::modules::stdlib()` to keep the built-ins.
- Name the root table after your program with `RootTable`.
- A module that reaches outside the script checks its grant in Rust, from the `InstallContext`.
- Call `airsl::luals::verify` in your tests, and `airsl::luals::setup` to write the editor files for
  your own root table.
