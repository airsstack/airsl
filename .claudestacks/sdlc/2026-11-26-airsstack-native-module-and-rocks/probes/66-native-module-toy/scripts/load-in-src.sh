#!/bin/sh
# Runs inside the glibc-2.17 manylinux image: builds PUC Lua 5.4.7 and LuaJIT 2.1 from source, loads every .so in /out.
set -eu
echo "== loader glibc: $(ldd --version 2>&1 | head -1)"; uname -m
cd /tmp
curl -sSL -o lua.tgz https://www.lua.org/ftp/lua-5.4.7.tar.gz
echo "lua-5.4.7.tar.gz sha256: $(sha256sum lua.tgz | cut -d' ' -f1)"
tar xzf lua.tgz; make -s -C lua-5.4.7 linux >/dev/null 2>&1
curl -sSL -o lj.tgz https://github.com/LuaJIT/LuaJIT/archive/refs/heads/v2.1.tar.gz
echo "LuaJIT v2.1 branch tarball sha256: $(sha256sum lj.tgz | cut -d' ' -f1)"
mkdir lj; tar xzf lj.tgz -C lj --strip-components=1; make -s -C lj >/dev/null 2>&1 || make -C lj 2>&1 | tail -5
LUA=/tmp/lua-5.4.7/src/lua; LJ=/tmp/lj/src/luajit
$LUA -v; $LJ -v
for f in /out/*-lua54.so /out/*-luajit.so; do
  echo "$(basename $f): undefined lua* symbols: $(objdump -T $f | awk '$3=="*UND*" && $NF ~ /^lua/' | wc -l)"
  n=$(basename $f .so); mkdir -p /m/$n; cp $f /m/$n/airsstack.so
  case $n in *-lua54) I=$LUA;; *) I=$LJ;; esac
  printf '%s via %s -> ' "$n" "$(basename $I)"
  $I -e "package.cpath='/m/$n/?.so;'..package.cpath; print(require('airsstack').ok)" 2>&1 || echo "(exit $?)"
done
