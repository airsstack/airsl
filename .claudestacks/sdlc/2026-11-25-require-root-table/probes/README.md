# Probes

Rust probes run on rustc 1.94.1 (the toolchain `rust-toolchain.toml` pins), macOS. Each `.txt`
record's first line names its toolchain and dependency. Build output was deleted after each run;
only the source (`.rs`, `.Cargo.toml`) and the output (`.txt`, `.out`) are kept.

| Record | Source | Question |
|---|---|---|
| `airsl-0.2.0-require-today.txt` | `airsl-0.2.0-require-today.rs` + `.Cargo.toml` (airsl as a path dependency on checkout `4620389`) | What `require("<root>")` does today, per preset, from a file and from source, with and without a user file named after the root, for roots `airsstack` and `agentstacks` |
| `airsl-0.2.0-utf8-root.txt` | `airsl-0.2.0-utf8-root.rs` (same `Cargo.toml`) | Whether `utf8` is an accepted root, and what `package.loaded.utf8` holds under `trusted` |
| `mlua-0.12.0-require.txt` | `mlua-0.12.0-require.rs` + `.Cargo.toml` (mlua `=0.12.0`, features `lua54,vendored,serialize,send`, airsl's set) | Lua 5.4 `require` on a `package.loaded` hit (value count, precedence over a file); a fresh load's value count; a Rust wrapper's effect on error text; a registry table's identity after the global is replaced; how a Rust function taking `LuaString` and Lua's own `require` treat no argument, non-string arguments and a second argument (`f:`, `g:`); a second argument on a `package.loaded` hit (`h:`); a script replacing the `package.loaded` entry (`k:`). Re-run after the spec review fixed the no-argument calls, which the first run had formatted as `pcall(f, )` |
| `luals-require.out` | `luals-require.sh` (run as `sh luals-require.sh <checkout>/target/debug/airsl <empty-dir>`, airsl debug build of `4620389`) | LuaLS 3.19.2: does `require("<root>")` get the type file's types with no edit, with `return <root>` appended, with `---@meta <root>` plus the return, with a user file named after the root in the workspace, and under a host root |
| `airsl-check-typefile-with-return.txt` | inline in the record | `airsl check` compiles a generated type file with `\n\nreturn airsstack\n` at its end; the file's final bytes |
| `luals-wiki-annotations.mdx` | `curl -sS -L https://raw.githubusercontent.com/LuaLS/LuaLS.github.io/main/src/content/wiki/annotations.mdx` | LuaLS's own description of `---@meta [name]` (line 788). Fetched 2026-11-25 from the `main` branch, which is not pinned to 3.19.2: the decision rests on the 3.19.2 probe, and this quote only supports it |
