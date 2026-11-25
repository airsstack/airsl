# Probes

Each record is the raw output of one run on 2026-11-25 (macOS, rustc 1.99.0, LuaLS 3.19.2,
checkout `577e401`).

## LuaLS (`luals/`)

`sh luals/run.sh <empty-dir>` rebuilds every fixture and prints, for each case, the luarc and
`.gitignore` contents, the exact command, and the diagnostics. `luals/run.out` is its output.
The `*-typefile.lua`, `*-good.lua` and `*-bad.lua` files are the fixtures it copies.
`luals/check-typefiles.out` is `cargo run -q -p airsl-cli -- check` on two fixtures, run from the
checkout.

## Rust

Each `*-main.rs` is `src/main.rs` of a throwaway crate run with `cargo run -q`. Each crate has its
own `[workspace]`. The checkout's `Cargo.lock` is copied in where `airsl` is a dependency.

| Record | Source | `[dependencies]` |
|---|---|---|
| `probe-surface.txt` | `surface-main.rs` | `airsl = { path = "<checkout>/crates/airsl" }` |
| `probe-mlua-typename.txt` | `typename-main.rs` | `airsl = { path = … }` (mlua 0.12.0 via `airsl::mlua`) |
| `probe-serde.txt` | `serde-main.rs` | `serde_json = "=1.0.151"` |
| `probe-more.txt` | `more-main.rs` | `airsl = { path = … }`, `serde_json = "=1.0.151"` |
| `probe-io.txt` | `io-main.rs` | none (std) |
| `probe-fs-special.txt` | `fs-special-main.rs` | none (std; uses `mkfifo`) |

`surface-greps.txt` records two `grep` runs over `crates/airsl/src/modules/`, each with its command.
`luals-provider-53-54.lua` is lines 50–58 of LuaLS 3.19.2
`libexec/script/provider/provider.lua` (Mason install).

## Other records

- `cargo-tree-serde-json.txt`: `cargo tree -e features -i serde_json` in the checkout.
- `surface-inventory.md`: per-key arguments and returns of the 12 modules, read from the sources with `file:line`.
