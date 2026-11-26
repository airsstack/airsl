#!/bin/bash
# Builds Lua 5.4.8 and LuaJIT v2.1 from source into $1 (fallback / second interpreter set on macOS).
set -u
D=$1; mkdir -p $D; cd $D
curl -sSL -o lua.tgz https://www.lua.org/ftp/lua-5.4.8.tar.gz
echo "lua-5.4.8.tar.gz sha256: $(shasum -a 256 lua.tgz | cut -d' ' -f1)"
tar xzf lua.tgz; make -s -C lua-5.4.8 macosx >/dev/null 2>&1 || echo "lua make FAILED"
curl -sSL -o lj.tgz https://github.com/LuaJIT/LuaJIT/archive/refs/heads/v2.1.tar.gz
echo "LuaJIT v2.1 branch tarball sha256: $(shasum -a 256 lj.tgz | cut -d' ' -f1)"
mkdir lj; tar xzf lj.tgz -C lj --strip-components=1
MACOSX_DEPLOYMENT_TARGET=11.0 make -s -C lj >/dev/null 2>&1 || MACOSX_DEPLOYMENT_TARGET=11.0 make -C lj 2>&1 | tail -5
$D/lua-5.4.8/src/lua -v; $D/lj/src/luajit -v; file $D/lua-5.4.8/src/lua $D/lj/src/luajit
