#!/bin/bash
# Y1: runs inside quay.io/pypa/manylinux2014_<arch>. $1 = repo checkout (copied), $2 = output dir.
# Builds Lua 5.4.8 + LuaJIT v2.1 from source, luarocks 3.12.2 from source once per interpreter,
# then `luarocks make` + `luarocks pack` both real-shaped rockspecs.
set -u
SRC=$1; OUT=$2; mkdir -p "$OUT"
echo "== host: $(uname -m); $(ldd --version 2>&1 | head -1); $(cat /etc/redhat-release 2>/dev/null)"
echo "== tools: $(gcc --version | head -1); $(make --version | head -1); git=$(git --version 2>&1); unzip=$(command -v unzip || echo absent); zip=$(command -v zip || echo absent)"
if ! command -v unzip >/dev/null || ! command -v zip >/dev/null; then yum install -y -q unzip zip >/tmp/yum.log 2>&1; echo "yum install unzip zip exit=$?"; tail -2 /tmp/yum.log; fi
echo "after: unzip=$(command -v unzip || echo absent) zip=$(command -v zip || echo absent)"
PY=$(ls /opt/python/cp3*/bin/python3 2>/dev/null | sort -V | tail -1); echo "python for zip listing: $PY"
curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.94 >/dev/null 2>&1; echo "rustup exit=$?"
. "$HOME/.cargo/env"; rustc -V; cargo -V
B=/tmp/y1; mkdir -p $B; cd $B
curl -sSfL -o lua.tgz https://www.lua.org/ftp/lua-5.4.8.tar.gz; echo "lua-5.4.8.tar.gz sha256: $(sha256sum lua.tgz | cut -d' ' -f1)"
tar xzf lua.tgz; make -s -C lua-5.4.8 linux >/tmp/lua.log 2>&1; echo "lua make exit=$?"; tail -3 /tmp/lua.log
make -s -C lua-5.4.8 install INSTALL_TOP=/opt/lua54 >/dev/null 2>&1; echo "lua install exit=$?"
git clone -q --depth 1 --branch v2.1 https://github.com/LuaJIT/LuaJIT.git lj; echo "luajit commit $(git -C lj log -1 --format='%H %cI')"
make -s -C lj PREFIX=/opt/luajit >/tmp/lj.log 2>&1; echo "luajit make exit=$?"; tail -3 /tmp/lj.log
make -s -C lj install PREFIX=/opt/luajit >/tmp/lji.log 2>&1; echo "luajit install exit=$?"; tail -3 /tmp/lji.log
ls /opt/lua54/bin /opt/luajit/bin /opt/luajit/include
/opt/lua54/bin/lua -v; /opt/luajit/bin/luajit -v
curl -sSfL -o lr.tgz https://luarocks.github.io/luarocks/releases/luarocks-3.12.2.tar.gz; echo "luarocks-3.12.2.tar.gz sha256: $(sha256sum lr.tgz | cut -d' ' -f1)"
for v in 54 jit; do
  rm -rf lr-$v; mkdir lr-$v; tar xzf lr.tgz -C lr-$v --strip-components=1; cd lr-$v
  if [ $v = 54 ]; then ./configure --prefix=/opt/lr54 --with-lua=/opt/lua54 >/tmp/cfg.log 2>&1
  else ./configure --prefix=/opt/lrjit --with-lua=/opt/luajit --with-lua-interpreter=luajit --with-lua-include=/opt/luajit/include/luajit-2.1 >/tmp/cfg.log 2>&1; fi
  echo "luarocks($v) configure exit=$?"; tail -12 /tmp/cfg.log
  make >/tmp/lrm.log 2>&1 && make install >>/tmp/lrm.log 2>&1; echo "luarocks($v) make+install exit=$?"; tail -3 /tmp/lrm.log
  cd $B
done
cp -r "$SRC" /tmp/repo; cd /tmp/repo; rm -rf target
# Runs 1-2: the LuaJIT-hosted luarocks could not load the backend it installed into --tree.
# Run 3 adds two LuaJIT variants: jit-sys (backend preinstalled in /opt/lrjit, the luarocks' own
# tree) and jit-via54 (the Lua 5.4-hosted luarocks targeting LuaJIT with --lua-version 5.1 --lua-dir).
for v in 54 jit jit-sys jit-via54; do
  LR=/opt/lr$v/bin/luarocks; LRA=""
  case $v in
    jit-sys) LR=/opt/lrjit/bin/luarocks
      echo "=== (jit-sys) preinstall backend into /opt/lrjit"
      $LR install luarocks-build-rust-mlua 0.2.7 2>&1; echo "install exit=$?"; find /opt/lrjit -name 'rust-mlua*' ;;
    jit-via54) LR=/opt/lr54/bin/luarocks; LRA="--lua-version 5.1 --lua-dir /opt/luajit" ;;
  esac
  if [ $v = 54 ]; then RS=crates/airsl-lua/rockspecs/airsstack-0.0.0-1.rockspec; PKG=airsstack; LUA=/opt/lua54/bin/lua
  else RS=crates/airsl-lua/rockspecs/airsstack-luajit-0.0.0-1.rockspec; PKG=airsstack-luajit; LUA=/opt/luajit/bin/luajit; fi
  T=/tmp/tree-$v
  echo "=== ($v) $LR $LRA --version: $($LR --version | head -1)"
  echo "config lua_version=$($LR $LRA config lua_version)  arch=$($LR $LRA config arch)  LUA=$($LR $LRA config variables.LUA)  INCDIR=$($LR $LRA config variables.LUA_INCDIR)"
  $LR $LRA make --tree $T $RS 2>&1 | grep -v '^ *\(Compiling\|Downloaded\|Downloading\)'; echo "make exit=${PIPESTATUS[0]}"
  find $T -name '*.so' | sort
  # subshell: run 1 leaked the 5.4 `luarocks path` LUA_PATH into the LuaJIT luarocks
  ( eval "$($LR $LRA path --tree $T)"; $LUA ci/probe.lua; echo "require exit=$?" )
  mkdir -p /tmp/pack-$v; (cd /tmp/pack-$v && $LR $LRA pack --tree $T $PKG 2>&1; echo "pack exit=$?"; ls -la)
  for r in /tmp/pack-$v/*.rock; do
    echo "== rock file: $(basename $r)"
    if command -v unzip >/dev/null; then unzip -l $r; else $PY -m zipfile -l $r; fi
    rm -rf /tmp/x; mkdir /tmp/x; (cd /tmp/x && $PY -m zipfile -e $r .)
    for so in $(find /tmp/x -name '*.so'); do
      echo "-- $so"
      echo "max GLIBC: $(objdump -T $so | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)"
      echo "all GLIBC: $(objdump -T $so | grep -o 'GLIBC_[0-9.]*' | sort -uV | tr '\n' ' ')"
      echo "NEEDED: $(objdump -p $so | awk '/NEEDED/{print $2}' | tr '\n' ' ')"
    done
    cp $r "$OUT"/
  done
done
ls -la "$OUT"
