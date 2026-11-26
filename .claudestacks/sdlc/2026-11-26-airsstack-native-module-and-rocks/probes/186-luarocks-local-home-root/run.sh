#!/bin/sh
# Probe 186 (P6 root part): runs inside debian:bookworm arm64 as root.
r() { echo "\$ $*"; "$@" 2>&1; echo "[exit $?]"; echo; }
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq >/dev/null 2>&1 && apt-get install -y -qq --no-install-recommends lua5.4 luarocks ca-certificates >/dev/null 2>&1
echo "apt-get install exit: $?"
r id -u
r uname -m
r cat /etc/debian_version
r dpkg-query -W -f '${Package} ${Version}\n' luarocks lua5.4
r luarocks --version
r lua5.4 -v
r luarocks --lua-version 5.4 config lua_version
r luarocks --lua-version 5.4 config arch
r luarocks --lua-version 5.4 install --local --only-server=/srv airsstack 0.0.2
r luarocks --lua-version 5.4 install --local /srv/airsstack-0.0.2-1.macosx-aarch64.rock
r luarocks --lua-version 5.4 install --tree /root/.luarocks --only-server=/srv airsstack 0.0.2
r luarocks --lua-version 5.4 install --tree /root/.luarocks /srv/airsstack-0.0.2-1.macosx-aarch64.rock
f=$(dpkg -L luarocks | grep '/luarocks/cmd.lua$' | head -1)
echo "== $f"
grep -n "meant for operating in a user's home directory" -A3 -B3 "$f"
r grep -n '^function cmd.run_command' "$f"
grep -n 'local function die' -A12 "$f"
