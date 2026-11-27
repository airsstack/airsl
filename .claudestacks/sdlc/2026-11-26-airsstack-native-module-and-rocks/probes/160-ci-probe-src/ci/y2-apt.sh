#!/bin/bash
# Y2: Ubuntu apt Lua 5.4 + LuaJIT + luarocks on the runner itself. Run from the repo root.
set -u
echo "ImageOS=${ImageOS:-} ImageVersion=${ImageVersion:-}"; . /etc/os-release; echo "$PRETTY_NAME"; uname -m
echo "== preinstalled: lua=$(command -v lua || echo none) lua5.4=$(command -v lua5.4 || echo none) luajit=$(command -v luajit || echo none) luarocks=$(command -v luarocks || echo none)"
sudo apt-get update -qq >/dev/null 2>&1
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq lua5.4 liblua5.4-dev luajit libluajit-5.1-dev luarocks 2>&1 | tail -3; echo "apt-get install exit=${PIPESTATUS[0]}"
dpkg-query -W lua5.4 liblua5.4-dev luajit libluajit-5.1-dev libluajit-5.1-2 luarocks lua5.1 2>&1
echo "which: $(which lua lua5.4 lua5.1 luajit luarocks 2>&1 | tr '\n' ' ')"
ls -la /usr/bin/lua* /etc/alternatives/lua* 2>&1
lua -v 2>&1; lua5.4 -v; luajit -v
head -1 $(command -v luarocks); luarocks --version | head -2
echo "default lua_version: $(luarocks config lua_version 2>&1)  LUA=$(luarocks config variables.LUA 2>&1)"
echo "--lua-version 5.4: lua_version=$(luarocks --lua-version 5.4 config lua_version 2>&1) LUA=$(luarocks --lua-version 5.4 config variables.LUA 2>&1) INC=$(luarocks --lua-version 5.4 config variables.LUA_INCDIR 2>&1)"
echo "--lua-version 5.1: lua_version=$(luarocks --lua-version 5.1 config lua_version 2>&1) LUA=$(luarocks --lua-version 5.1 config variables.LUA 2>&1) INC=$(luarocks --lua-version 5.1 config variables.LUA_INCDIR 2>&1)"
echo "--lua-version 5.1 --lua-dir /usr: lua_version=$(luarocks --lua-version 5.1 --lua-dir /usr config lua_version 2>&1) LUA=$(luarocks --lua-version 5.1 --lua-dir /usr config variables.LUA 2>&1) INC=$(luarocks --lua-version 5.1 --lua-dir /usr config variables.LUA_INCDIR 2>&1)"
ls /etc/luarocks/ 2>&1; for f in /etc/luarocks/*; do echo "--- $f"; cat $f; done
rustup toolchain install 1.94 --profile minimal >/dev/null 2>&1; rustc -V; cargo -V
rm -rf target
# LuaJIT attempts, in order: plain --lua-version 5.1; --lua-dir /usr; an explicit config file naming luajit.
mkdir -p $RUNNER_TEMP/cfg
cat > $RUNNER_TEMP/cfg/luajit.lua <<'CFG'
variables = { LUA = "/usr/bin/luajit", LUA_INCDIR = "/usr/include/luajit-2.1" }
CFG
run_make() { # $1 label, $2 tree, $3 rockspec, $4 pkg, $5 interp, rest = luarocks args
  local label=$1 T=$2 RS=$3 PKG=$4 I=$5; shift 5
  echo "=== ($label) luarocks $* make"
  echo "luajit_version seen: $(luarocks "$@" config luajit_version 2>&1)"
  luarocks "$@" make --tree $T $RS 2>&1 | grep -v '^ *\(Compiling\|Downloaded\|Downloading\)'; echo "make exit=${PIPESTATUS[0]}"
  find $T -name 'airsstack*.so' | sort
  ( eval "$(luarocks "$@" path --tree $T)"; $I ci/probe.lua; echo "require via $I exit=$?" )
  P=$RUNNER_TEMP/pack-$label; mkdir -p $P; (cd $P && luarocks "$@" pack --tree $T $PKG 2>&1; echo "pack exit=$?"; ls)
}
run_make 54 $RUNNER_TEMP/t54 crates/airsl-lua/rockspecs/airsstack-0.0.0-1.rockspec airsstack lua5.4 --lua-version 5.4
run_make jit-plain $RUNNER_TEMP/tjit1 crates/airsl-lua/rockspecs/airsstack-luajit-0.0.0-1.rockspec airsstack-luajit luajit --lua-version 5.1
run_make jit-luadir $RUNNER_TEMP/tjit2 crates/airsl-lua/rockspecs/airsstack-luajit-0.0.0-1.rockspec airsstack-luajit luajit --lua-version 5.1 --lua-dir /usr
export LUAROCKS_CONFIG=$RUNNER_TEMP/cfg/luajit.lua
run_make jit-config $RUNNER_TEMP/tjit3 crates/airsl-lua/rockspecs/airsstack-luajit-0.0.0-1.rockspec airsstack-luajit luajit --lua-version 5.1
unset LUAROCKS_CONFIG
# Run 1: luarocks 3.8.0 (running on lua5.1) could not load the backend it had just installed into
# --tree, and never saw LuaJIT as the VM. Retry with the backend preinstalled where luarocks itself
# searches (--local, Lua 5.1 user tree), and with a --lua-dir holding only luajit.
echo "=== preinstall backend into the 5.1 user tree"
luarocks --lua-version 5.1 install --local luarocks-build-rust-mlua 0.2.7 2>&1; echo "install --local exit=$?"
find $HOME/.luarocks -name 'rust-mlua*' | sort
run_make 54-prebackend $RUNNER_TEMP/t54b crates/airsl-lua/rockspecs/airsstack-0.0.0-1.rockspec airsstack lua5.4 --lua-version 5.4
mkdir -p $RUNNER_TEMP/ljdir/bin; ln -sf /usr/bin/luajit $RUNNER_TEMP/ljdir/bin/luajit
ln -sfn /usr/include/luajit-2.1 $RUNNER_TEMP/ljdir/include; ln -sfn /usr/lib/x86_64-linux-gnu $RUNNER_TEMP/ljdir/lib
echo "ljdir config: lua_version=$(luarocks --lua-version 5.1 --lua-dir $RUNNER_TEMP/ljdir config lua_version 2>&1) LUA=$(luarocks --lua-version 5.1 --lua-dir $RUNNER_TEMP/ljdir config variables.LUA 2>&1) INC=$(luarocks --lua-version 5.1 --lua-dir $RUNNER_TEMP/ljdir config variables.LUA_INCDIR 2>&1)"
run_make jit-ljdir-prebackend $RUNNER_TEMP/tjit4 crates/airsl-lua/rockspecs/airsstack-luajit-0.0.0-1.rockspec airsstack-luajit luajit --lua-version 5.1 --lua-dir $RUNNER_TEMP/ljdir
echo "=== fallback: the toy built by cargo directly (when luarocks produced no .so), for the 2x2 load"
mkdir -p $RUNNER_TEMP/cargo54 $RUNNER_TEMP/cargojit
cargo build --release --locked --package airsl-lua --features lua54 --target-dir $RUNNER_TEMP/tg54 >/tmp/c54.log 2>&1; echo "cargo lua54 exit=$?"; tail -1 /tmp/c54.log
cargo build --release --locked --package airsl-lua --features luajit --target-dir $RUNNER_TEMP/tgjit >/tmp/cjit.log 2>&1; echo "cargo luajit exit=$?"; tail -1 /tmp/cjit.log
cp $RUNNER_TEMP/tg54/release/libairsstack.so $RUNNER_TEMP/cargo54/airsstack.so; cp $RUNNER_TEMP/tgjit/release/libairsstack.so $RUNNER_TEMP/cargojit/airsstack.so
echo "=== 2x2: each ABI's installed module under each interpreter"
J=$(find $RUNNER_TEMP/tjit* -name airsstack.so | head -1); F=$(find $RUNNER_TEMP/t54* -name airsstack.so | head -1)
[ -n "$F" ] || { F=$RUNNER_TEMP/cargo54/airsstack.so; echo "lua54 build: cargo fallback $F"; }
[ -n "$J" ] || { J=$RUNNER_TEMP/cargojit/airsstack.so; echo "luajit build: cargo fallback $J"; }
for so in "$F" "$J"; do
  [ -n "$so" ] || { echo "(missing a build)"; continue; }
  d=$(dirname $so)
  for I in lua5.4 luajit; do
    echo "-- $so via $I"; LUA_CPATH="$d/?.so" $I ci/probe.lua 2>&1; echo "exit=$?"
  done
done
