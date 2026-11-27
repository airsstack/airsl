#!/bin/sh
# Probe 186 (P6 root part, run 2): debian:bookworm arm64 as root; why --local did not refuse in run 1.
r() { echo "\$ $*"; "$@" 2>&1; echo "[exit $?]"; echo; }
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq >/dev/null 2>&1 && apt-get install -y -qq --no-install-recommends lua5.4 luarocks >/dev/null 2>&1
echo "apt-get install exit: $?"
r dpkg-query -W -f '${Package} ${Version}\n' luarocks lua5.4
echo "USER=${USER-<unset>} HOME=${HOME-<unset>} id -u=$(id -u)"; echo
f=/usr/share/lua/5.1/luarocks/fs/unix.lua
echo "== $f"; grep -n 'function unix.is_superuser' -A3 "$f"; echo
# a pure-Lua rock (no compiler, arch "all") so an install can succeed
mkdir -p /work/src && cd /work
printf 'return { ok = true }\n' > src/airsstack.lua
cat > airsstack-0.0.2-1.rockspec <<'RS'
package = "airsstack"
version = "0.0.2-1"
source = { url = "file:///work/src" }
dependencies = { "lua == 5.4" }
build = { type = "builtin", modules = { airsstack = "src/airsstack.lua" } }
RS
r luarocks --lua-version 5.4 make --pack-binary-rock airsstack-0.0.2-1.rockspec
r ls /work
echo "=== (a) USER unset (docker default)"
r luarocks --lua-version 5.4 install --local /work/airsstack-0.0.2-1.all.rock
r luarocks --lua-version 5.4 list --local --porcelain
echo "=== (b) USER=root (as a login shell / sudo -i sets it)"
r env USER=root luarocks --lua-version 5.4 install --local /work/airsstack-0.0.2-1.all.rock
r env USER=root luarocks --lua-version 5.4 config lua_version
r env USER=root luarocks --lua-version 5.4 path --local
r env USER=root luarocks --lua-version 5.4 install --tree /root/.luarocks /work/airsstack-0.0.2-1.all.rock
echo "=== (c) USER=nobody-name, as root"
r env USER=alice luarocks --lua-version 5.4 install --local --force /work/airsstack-0.0.2-1.all.rock
