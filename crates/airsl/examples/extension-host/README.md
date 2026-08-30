# extension-host

Loads a directory of third-party extensions under one ceiling, and broadcasts one event to every
extension that made it in.

This is the seam that turns the crate from "a host embeds one script it wrote" into "a host runs a
plugin directory it never audited line by line". `word-count` and `broken` sit side by side in
`extensions/`, and `ExtensionHost::load_dir` treats them the same way: each manifest is read,
negotiated against the host's [`Ceiling`], and either started or refused — one refusal never stops
the other extension from loading, because a directory of five extensions where one has a typo in
its manifest should not come up with zero.

## Run

```bash
cargo run -p airsl --example extension-host
```

`extensions/broken/main.lua` compiles on its own — `airsl check` parses without running, so it
never needs the `airsstack` global its `extension.toml` cannot actually reach:

```bash
airsl check crates/airsl/examples/extension-host/
```

Running the directory through the `airsl` binary is not how this example is meant to be driven —
`ExtensionHost` is a library seam a host program builds around, not a CLI subcommand — which is why
this example is a `main.rs` rather than a script under `airsl run`.

## Output

```
loaded: word-count
failed: broken — extension `broken` was not loaded: fs.read `/`: outside the granted read roots: none

word-count: {"longest":"quick","words":9}
```

The `fs.read` value above is this platform's filesystem root — unix shows `/`; an equivalent
Windows build shows the drive root `main.rs`'s `OUTSIDE_ROOT` supplies instead.

## What it demonstrates

- **`load_dir` never short-circuits.** `extensions/broken` asks for `fs.read = ["$OUTSIDE"]`, which
  `main.rs` expands to the filesystem root, and the host is built with
  `Ceiling::new(Policy::confined())` (`src/extension/ceiling.rs:27`), whose grants start empty — the
  root is outside every read root the ceiling allows, so negotiation denies the request before
  `broken/main.lua` is opened for evaluation. `word-count` loads regardless: one
  directory's manifest error is data in the returned [`LoadReport`]
  (`src/extension/load_report.rs:20`), never an early return that would have stopped `word-count`
  from being tried.
- **Fail-closed, by construction.** The denial happens in [`Extension::approve`]
  (`src/extension/loaded.rs:175`), a step before [`Approved::start`]
  (`src/extension/loaded.rs:96`) exists to be called — there is no engine for `broken` to have run
  code in, which is what "extensions/broken" is standing in for rather than merely a manifest that
  fails to parse.
- **`broadcast` visits every *loaded* extension and returns one [`Dispatch`] per one, in load
  order** (`src/extension/host.rs:312`). Only `word-count` is in the registry by the time the event
  fires, so the output has exactly one line — with two extensions loaded it would have two, neither
  one able to hide the other's answer or its error.
- **The output is byte-stable.** `word-count/main.lua` builds `{ words = ..., longest = ... }` in
  that field order, but this crate's `serde_json` build has the `preserve_order` feature off, so
  `serde_json::Value`'s map is a `BTreeMap` and `to_string` renders `longest` before `words`
  regardless — the same bytes on every run, on every machine, independent of what order the Lua
  table happened to be constructed in.
- **`regex = true` in the manifest is a presence assertion, not a grant.** `airsstack.regex` needs
  no authority (`src/modules/regex.rs:8`) — the capability line only tells negotiation "this
  runtime must install a module named `regex`" (`src/extension/negotiate.rs:236`), which is why
  `word-count` gets the module with an empty [`GrantSet`] and the manifest still has a reason for
  the line to exist: `\w+` finds words the way `string.gmatch` cannot state directly, with no
  alternation and no character-class shorthand of its own.

## See also

- [custom-module](../custom-module/) — the module seam this example's `word-count` extension
  reaches through (`airsstack.regex`), from the host-authoring side rather than the third-party
  side.
- [extensions](../../docs/extensions.md) — the extension host in full: the manifest format,
  negotiation, and what an approver can override.
