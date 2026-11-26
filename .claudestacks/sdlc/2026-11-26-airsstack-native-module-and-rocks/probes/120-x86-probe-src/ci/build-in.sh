#!/bin/sh
# Runs inside quay.io/pypa/manylinux2014_x86_64. Toy at /toy (ro), outputs to /out.
set -u
curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.94 >/dev/null 2>&1
. "$HOME/.cargo/env"
echo "== toolchain"; uname -m; rustc -V; cargo -V; (cc --version 2>/dev/null || gcc --version) | head -1
echo "== ldd --version: $(ldd --version 2>&1 | head -1)"
cp -r /toy /work; cd /work; rm -rf target
for f in lua54 luajit; do
  echo "== cargo build --locked --release --features $f"
  cargo build --locked --release --features $f >/tmp/b.log 2>&1; echo "build $f exit=$?"; tail -3 /tmp/b.log
  cp target/release/libairsstack.so /out/ml-$f.so
done
for f in /out/ml-*.so; do
  echo "== $(basename $f)"
  echo "max GLIBC: $(objdump -T $f | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)"
  echo "all GLIBC: $(objdump -T $f | grep -o 'GLIBC_[0-9.]*' | sort -uV | tr '\n' ' ')"
  echo "NEEDED: $(objdump -p $f | awk '/NEEDED/{print $2}' | tr '\n' ' ')"
  echo "undefined lua* symbols: $(objdump -T $f | awk '$3=="*UND*" && $NF ~ /^lua/' | wc -l)"
done
