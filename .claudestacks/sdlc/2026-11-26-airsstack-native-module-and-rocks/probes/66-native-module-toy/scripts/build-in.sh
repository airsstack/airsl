#!/bin/sh
# Runs inside a build container. $1 = tag for output names. Toy source at /toy (ro), outputs to /out.
set -eu
TAG=$1
if ! command -v cargo >/dev/null 2>&1; then
  curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.94.1 >/dev/null 2>&1
  . "$HOME/.cargo/env"
fi
echo "== $TAG toolchain"
uname -m; rustc --version; cargo --version; (cc --version 2>/dev/null || gcc --version) | head -1
echo "== $TAG ldd --version"; ldd --version 2>&1 | head -1
cp -r /toy /work; cd /work
for f in lua54 luajit; do
  start=$(date +%s)
  cargo build --locked --release --features $f -q
  echo "== $TAG build $f: $(( $(date +%s) - start ))s"
  cp target/release/libairsstack.so /out/$TAG-$f.so
done
for f in /out/$TAG-*.so; do
  echo "== $(basename $f)"
  echo "max GLIBC: $(objdump -T $f | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)"
  echo "all GLIBC: $(objdump -T $f | grep -o 'GLIBC_[0-9.]*' | sort -uV | tr '\n' ' ')"
  echo "NEEDED: $(objdump -p $f | awk '/NEEDED/{print $2}' | tr '\n' ' ')"
  echo "undefined lua_ symbols (count, first 3): $(objdump -T $f | awk '$3=="*UND*" && $NF ~ /^lua/ {print $NF}' | sort | awk '{a[NR]=$0} END{printf "%d: %s %s %s", NR, a[1], a[2], a[3]}' | tr '\n' ' ')"
done
