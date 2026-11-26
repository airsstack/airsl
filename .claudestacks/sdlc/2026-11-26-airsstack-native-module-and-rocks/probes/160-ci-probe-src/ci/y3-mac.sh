#!/bin/bash
# Y3: Homebrew lua@5.4 + luajit + luarocks; make + pack both rockspecs. Run from the repo root.
set -u
export HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_CLEANUP=1
echo "ImageOS=${ImageOS:-} ImageVersion=${ImageVersion:-}"; uname -m; sw_vers
echo "== preinstalled: lua=$(command -v lua || echo none) luajit=$(command -v luajit || echo none) luarocks=$(command -v luarocks || echo none)"
brew --version | head -1
brew install lua@5.4 luajit luarocks 2>&1 | tail -20; echo "brew install exit=${PIPESTATUS[0]}"
brew list --versions lua@5.4 luajit luarocks lua
brew info luarocks 2>&1 | sed -n '1,12p'
L54=$(brew --prefix lua@5.4); LJ=$(brew --prefix luajit)
echo "brew --prefix lua@5.4 = $L54"; ls -la $L54/bin; echo "brew --prefix luajit = $LJ"; ls -la $LJ/bin
echo "which lua lua5.4 luajit luarocks: $(which lua lua5.4 luajit luarocks 2>&1 | tr '\n' ' ')"
lua -v 2>&1; $L54/bin/lua -v 2>&1; $L54/bin/lua5.4 -v 2>&1; $LJ/bin/luajit -v
luarocks --version | head -2
echo "default lua_version: $(luarocks config lua_version 2>&1)"
echo "--lua-version 5.4 --lua-dir lua@5.4: lua_version=$(luarocks --lua-version 5.4 --lua-dir $L54 config lua_version 2>&1)  LUA=$(luarocks --lua-version 5.4 --lua-dir $L54 config variables.LUA 2>&1)"
echo "--lua-version 5.4 (no --lua-dir): lua_version=$(luarocks --lua-version 5.4 config lua_version 2>&1) LUA=$(luarocks --lua-version 5.4 config variables.LUA 2>&1)"
echo "--lua-version 5.1 --lua-dir luajit: lua_version=$(luarocks --lua-version 5.1 --lua-dir $LJ config lua_version 2>&1) LUA=$(luarocks --lua-version 5.1 --lua-dir $LJ config variables.LUA 2>&1)"
rustup toolchain install 1.94 --profile minimal >/dev/null 2>&1; rustc -V; cargo -V
rm -rf target
for v in 54 jit; do
  if [ $v = 54 ]; then A="--lua-version 5.4 --lua-dir $L54"; RS=crates/airsl-lua/rockspecs/airsstack-0.0.0-1.rockspec; PKG=airsstack; LUA=$L54/bin/lua
  else A="--lua-version 5.1 --lua-dir $LJ"; RS=crates/airsl-lua/rockspecs/airsstack-luajit-0.0.0-1.rockspec; PKG=airsstack-luajit; LUA=$LJ/bin/luajit; fi
  [ -x $LUA ] || LUA=$L54/bin/lua5.4
  T=$RUNNER_TEMP/tree-$v
  echo "=== ($v) luarocks $A make"
  luarocks $A make --tree $T $RS 2>&1 | grep -v '^ *\(Compiling\|Downloaded\|Downloading\)'; echo "make exit=${PIPESTATUS[0]}"
  find $T -name 'airsstack*' | sort
  ( eval "$(luarocks $A path --tree $T)"; $LUA ci/probe.lua; echo "require exit=$?" )
  P=$RUNNER_TEMP/pack-$v; mkdir -p $P; (cd $P && luarocks $A pack --tree $T $PKG 2>&1; echo "pack exit=$?"; ls -la)
  for r in $P/*.rock; do echo "== rock file: $(basename $r)"; unzip -l $r; done
done
