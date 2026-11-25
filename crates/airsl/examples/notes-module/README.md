# notes-module

The program behind the book's chapter [Writing Your Own Module](https://airsstack.github.io/airsl/ch07-00-your-own-module.html):
a `notes` module whose `add` function collects strings from a script, installed next to the
built-ins under a root table named `myapp`.

The chapter prints this code with mdBook's include markers, so the text a reader sees is code the
gate compiles, and a change that breaks it fails the build instead of shipping a stale page.

## Files

| File | What it is |
| --- | --- |
| `notes.rs` | The `Notes` module. Both programs pull it in with `include!`, so there is one copy. |
| `main.rs` | Installs `Notes` into the built-ins and reads back what a script added. |
| `setup.rs` | The second example, `notes-module-setup`: writes the editor files for `myapp` into the working directory, and holds the `#[test]` that calls `airsl::luals::verify`. |

`setup.rs` is declared in the crate manifest with `test = true`, so `cargo make dod` runs its test.
It is not run by `cargo make examples`, which runs only the directory's `main.rs`, because it writes
into the working directory.

## Run

```bash
cargo run -p airsl --example notes-module
```

To see the editor files, run the second program in a scratch folder:

```bash
cargo build -p airsl --example notes-module-setup
mkdir /tmp/notes-demo && cd /tmp/notes-demo
/path/to/target/debug/examples/notes-module-setup
```

## Output

```
note: disk is 91% full
note: backup finished in 42s
```

The second program prints, on a first run in an empty folder:

```
created .airsl/types/myapp.lua
created .luarc.json
```

## What it demonstrates

- **A module is `name`, `install` and `describe`.** `describe` lists what `install` sets, and the
  test in `setup.rs` fails when the two disagree.
- **Nothing here needs a grant.** Collecting strings reaches nothing, so `install` ignores its
  `InstallContext`. [`custom-module`](../custom-module/) shows one that reads it.
