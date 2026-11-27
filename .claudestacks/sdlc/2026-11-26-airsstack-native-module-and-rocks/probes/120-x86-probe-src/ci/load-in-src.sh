#!/bin/sh
# Runs inside manylinux2014_x86_64 (glibc 2.17): builds Lua 5.4.8 and LuaJIT v2.1 from source, loads each .so in /out.
set -u
echo "== loader: $(ldd --version 2>&1 | head -1)"; uname -m
cd /tmp
curl -sSL -o lua.tgz https://www.lua.org/ftp/lua-5.4.8.tar.gz
echo "lua-5.4.8.tar.gz sha256: $(sha256sum lua.tgz | cut -d' ' -f1)"
tar xzf lua.tgz; make -s -C lua-5.4.8 linux >/dev/null 2>&1 || echo "lua make FAILED"
curl -sSL -o lj.tgz https://github.com/LuaJIT/LuaJIT/archive/refs/heads/v2.1.tar.gz
echo "LuaJIT v2.1 branch tarball sha256: $(sha256sum lj.tgz | cut -d' ' -f1)"
mkdir lj; tar xzf lj.tgz -C lj --strip-components=1; make -s -C lj >/dev/null 2>&1 || make -C lj 2>&1 | tail -5
LUA=/tmp/lua-5.4.8/src/lua; LJ=/tmp/lj/src/luajit
$LUA -v; $LJ -v
for f in /out/ml-lua54.so /out/ml-luajit.so; do
  n=$(basename $f .so); mkdir -p /m/$n; cp $f /m/$n/airsstack.so
  case $n in *-lua54) I=$LUA;; *) I=$LJ;; esac
  echo "-- $n via $I"
  $I /s/probe.lua /m/$n 2>&1; echo "exit=$?"
done
