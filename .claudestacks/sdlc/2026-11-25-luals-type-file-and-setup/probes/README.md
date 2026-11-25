# Probes

Rust probes run on rustc 1.94.1, the toolchain `rust-toolchain.toml` pins. Each Rust record's
first line names its toolchain. The LuaLS and `airsl` records were made on checkout `577e401` and
re-run on `bb95c10`, which changes no code (macOS, LuaLS 3.19.2).

## LuaLS (`luals/`)

`sh luals/run.sh <empty-dir>` rebuilds every fixture and prints, for each case, the luarc and
`.gitignore` contents, the exact command, and the diagnostics. `luals/run.out` is its output.
The `*-typefile.lua`, `*-good.lua` and `*-bad.lua` files are the fixtures it copies.
`luals/check-typefiles.out` is `cargo run -q -p airsl-cli -- check` on three fixtures, run from the
checkout. The `records` case and its fixtures (`records-typefile.lua`, `records-good.lua`,
`records-bad.lua`) were added on checkout `bb95c10`; re-running `run.sh` there left every earlier
case's output unchanged.

## Rust

Each `*-main.rs` is `src/main.rs` of a throwaway crate run with `cargo +1.94 run -q`. Each crate
has its own `[workspace]`. The checkout's `Cargo.lock` is copied in where `airsl` is a dependency.
`sh rerun.sh <checkout> <empty-scratch-dir>` rebuilds every crate below, rewrites each record
(header line plus output) and deletes the build. It creates the `w/` directory `io-main.rs`
writes into.

| Record | Source | `[dependencies]` |
|---|---|---|
| `probe-surface.txt` | `surface-main.rs` | `airsl = { path = "<checkout>/crates/airsl" }` |
| `probe-mlua-typename.txt` | `typename-main.rs` | `airsl = { path = … }` (mlua 0.12.0 via `airsl::mlua`) |
| `probe-serde.txt` | `serde-main.rs` | `serde_json = "=1.0.151"` |
| `probe-more.txt` | `more-main.rs` | `airsl = { path = … }`, `serde_json = "=1.0.151"` |
| `probe-io.txt` | `io-main.rs` | none (std) |
| `probe-fs-special.txt` | `fs-special-main.rs` | none (std; uses `mkfifo`) |
| `probe-socket.txt` | `socket-main.rs` | none (std) |

`surface-greps.txt` records two `grep` runs over `crates/airsl/src/modules/`, each with its command.
`luals-provider-53-54.lua` is lines 50–58 of LuaLS 3.19.2
`libexec/script/provider/provider.lua` (Mason install).

## Other records

- `probe-shapes.txt`: `shapes.lua` run twice with `airsl run`, from the chain directory, with
  stdin `"x"`. Line 1 states the working directory and the `<chain>` placeholder, and each command
  is printed before its output. It shows the key and value types of what `proc.run`,
  `regex.captures`, `ext.granted`, `fs.stat`, `json.decode` and `hook.payload` return.

- `cargo-tree-serde-json.txt`: `cargo tree -e features -i serde_json` in the checkout.
- `surface-inventory.md`: per-key arguments and returns of the 12 modules, read from the sources with `file:line`.
