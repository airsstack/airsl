# Hosting Extensions in Rust

`ExtensionHost` does everything from the last page in one place: it reads manifests, checks them
against your ceiling, asks the approver, starts each extension, and fires events at them.

This host loads every extension in a folder, then fires one `count` event at all of them:

```rust
use airsl::extension::Ceiling;
use airsl::{EventName, ExtensionHost, Policy};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut host = ExtensionHost::builder()
        .ceiling(Ceiling::new(Policy::confined())?)
        .events(["count"])?
        .build()?;

    let report = host.load_dir("extensions")?;
    for name in report.loaded() {
        println!("loaded: {name}");
    }
    for (dir, error) in report.failed() {
        println!("failed: {} ({error})", dir.display());
    }

    let payload = json!({ "text": "the quick brown fox jumps over the lazy dog" });
    for dispatch in host.broadcast(&EventName::new("count")?, &payload) {
        match dispatch.result() {
            Ok(Some(value)) => println!("{}: {value}", dispatch.name()),
            Ok(None) => println!("{}: (not subscribed)", dispatch.name()),
            Err(error) => println!("{}: error: {error}", dispatch.name()),
        }
    }
    Ok(())
}
```

With `word-count` and an extension that asks for too much side by side in `extensions/`:

```console
$ cargo run
loaded: word-count
failed: extensions/greedy (extension `greedy` was not loaded: fs.read `/`: outside the granted read roots: none)
word-count: {"longest":"quick","words":9}
```

## Walking through it

**`.ceiling(...)`** sets the most this host will ever allow. `Ceiling::new` refuses a policy that
would bound nothing, such as `Policy::trusted()`, because an unlimited ceiling is no ceiling. Like
the engine builder, there is no `build()` until you have given it a ceiling.

**`.events([...])`** lists the events this host fires. An extension that calls `ext.on` for any
other name is refused when it loads, so a typo in an event name shows up immediately rather than as a
handler that is never called.

**`load_dir`** loads every subfolder that holds an `extension.toml`, in name order. It never stops
at the first failure. Every outcome goes into the report, so one broken extension never hides the
others. Use `host.load(dir)` instead to load a single extension.

**`broadcast`** fires an event at every loaded extension, in load order, and returns one result each.
A failing handler does not stop the others either. `Ok(None)` means the extension did not subscribe
to that event, which is normal.

## Choices the host makes

A few settings on the builder are worth knowing:

| Method | Does |
|---|---|
| `.approver(a)` | uses your own approver instead of `ManifestApprover` |
| `.variables([("NAME", "value")])` | supplies values for `$NAME` in manifests |
| `.root_table(RootTable::new("myapp")?)` | names the global extensions see |
| `.modules(factory)` | chooses the modules each extension gets, including your own from [chapter 7](ch07-00-your-own-module.md) |

Some things are deliberately left to you. Grants are fixed for the life of a loaded extension; to
change what an extension may reach, load it again. And `broadcast` reports every failure but never
disables an extension on its own. Whether three failures in a row means "stop calling this one" is
your program's decision.

## Keeping extensions working

The manifest's `api = 1` is a promise from `airsl`. Within one API version, modules and events only
ever gain new functions and names. Nothing an extension already relies on changes meaning or goes
away. A change that would break that gets a new API number, and an extension that asks for an API
version this `airsl` does not support is refused before any of its code runs.

## Summary

- An extension is a folder with an `extension.toml` and an entry script that registers handlers with
  `ext.on`.
- The manifest asks; the host's ceiling and approver decide. Each step can only narrow the grant.
- A required capability that cannot be granted stops the extension from loading at all.
- `airsl ext doctor` shows the decision, and `airsl ext fire` tries an event.
- `ExtensionHost` loads a folder of extensions and broadcasts events to them, reporting every
  failure without stopping.
