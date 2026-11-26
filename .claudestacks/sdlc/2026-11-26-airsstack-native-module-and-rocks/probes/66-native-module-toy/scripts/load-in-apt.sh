#!/bin/sh
# Runs inside a Debian/Ubuntu image: apt lua5.4 + luajit, loads every .so in /out.
set -u
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq >/dev/null
# Each interpreter is installed on its own so a mirror gap for one does not hide the other.
for p in lua5.4 luajit; do apt-get install -y -qq $p >/dev/null 2>&1 || echo "apt-get install $p FAILED: $(apt-get install -y -qq $p 2>&1 | grep '^E:' | head -1)"; done
echo "== loader: $(. /etc/os-release; echo $PRETTY_NAME); $(ldd --version 2>&1 | head -1)"; uname -m
dpkg-query -W lua5.4 luajit libluajit-5.1-2 2>/dev/null || true
lua5.4 -v; luajit -v
for f in /out/*-lua54.so /out/*-luajit.so; do
  n=$(basename $f .so); mkdir -p /m/$n; cp $f /m/$n/airsstack.so
  case $n in *-lua54) I=lua5.4;; *) I=luajit;; esac
  command -v $I >/dev/null || { echo "$n: $I not installed, skipped"; continue; }
  printf '%s via %s -> ' "$n" "$I"
  $I -e "package.cpath='/m/$n/?.so;'..package.cpath; print(require('airsstack').ok)" 2>&1 || echo "(exit $?)"
done
