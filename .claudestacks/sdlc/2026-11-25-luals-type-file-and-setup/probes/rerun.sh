#!/bin/sh
# Re-records every throwaway-crate probe on the workspace's pinned toolchain.
# Usage: sh rerun.sh <checkout> <empty-scratch-dir>
# Each record's first line names the toolchain and the probe; the rest is the program's output.
set -u
CO="$1"; W="$2"; P="$(cd "$(dirname "$0")" && pwd)"; TC=1.94
rm -rf "$W"; mkdir -p "$W"
one() { # $1 record, $2 source, $3 deps ("" | airsl | serde | both)
  d="$W/$1"; mkdir -p "$d/src"
  printf '[package]\nname = "probe"\nversion = "0.0.0"\nedition = "2024"\n[workspace]\n[dependencies]\n' > "$d/Cargo.toml"
  case "$3" in *airsl*|both) printf 'airsl = { path = "%s/crates/airsl" }\n' "$CO" >> "$d/Cargo.toml"; cp "$CO/Cargo.lock" "$d/";; esac
  case "$3" in *serde*|both) printf 'serde_json = "=1.0.151"\n' >> "$d/Cargo.toml";; esac
  cp "$P/$2" "$d/src/main.rs"; mkdir -p "$d/w"   # io-main.rs writes into w/
  { echo "# $(cd "$d" && cargo +$TC --version | sed 's/cargo/toolchain/'), $(uname -sr), src/main.rs = $2, deps: ${3:-none}"
    (cd "$d" && cargo +$TC run -q 2>&1); } > "$P/$1"
  rm -rf "$d"
}
one probe-surface.txt surface-main.rs airsl
one probe-mlua-typename.txt typename-main.rs airsl
one probe-serde.txt serde-main.rs serde
one probe-more.txt more-main.rs both
one probe-io.txt io-main.rs ""
one probe-fs-special.txt fs-special-main.rs ""
one probe-socket.txt socket-main.rs ""
