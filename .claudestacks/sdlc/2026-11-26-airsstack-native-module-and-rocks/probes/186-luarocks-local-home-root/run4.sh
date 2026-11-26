#!/bin/sh
# Probe 186 run 4: luarocks 3.12.2 (the Homebrew Lua sources, mounted read-only at /lr) run by Debian's lua5.4, as root.
r() { echo "\$ $*"; "$@" 2>&1; echo "[exit $?]"; echo; }
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq >/dev/null 2>&1 && apt-get install -y -qq --no-install-recommends lua5.4 liblua5.4-dev >/dev/null 2>&1
echo "apt-get install exit: $?"
mkdir -p /etc/luarocks /usr/local/bin
cp /probe/luarocks-3.12.2-launcher /usr/local/bin/luarocks
chmod +x /usr/local/bin/luarocks
echo "USER=${USER-<unset>} HOME=${HOME-<unset>} id -u=$(id -u) whoami=$(whoami)"; echo
r luarocks --version
r lua5.4 -e 'print(pcall(require, "posix"))'
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
echo "=== (a) USER unset"
r luarocks --lua-version 5.4 install --local /work/airsstack-0.0.2-1.all.rock
echo "=== (b) USER=root"
r env USER=root luarocks --lua-version 5.4 install --local /work/airsstack-0.0.2-1.all.rock
echo "=== (c) USER=alice (still uid 0)"
r env USER=alice luarocks --lua-version 5.4 install --local /work/airsstack-0.0.2-1.all.rock
r luarocks --lua-version 5.4 install --tree /root/.luarocks /work/airsstack-0.0.2-1.all.rock
