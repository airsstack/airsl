#!/bin/sh
# Runs inside ubuntu:22.04: apt lua5.4 + luajit, loads each .so in /out.
set -u
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq >/dev/null
for p in lua5.4 luajit; do apt-get install -y -qq $p >/dev/null 2>&1 || echo "apt-get install $p FAILED"; done
echo "== loader: $(. /etc/os-release; echo $PRETTY_NAME); $(ldd --version 2>&1 | head -1)"; uname -m
dpkg-query -W lua5.4 luajit libluajit-5.1-2 2>/dev/null || true
lua5.4 -v; luajit -v
for f in /out/ml-lua54.so /out/ml-luajit.so; do
  n=$(basename $f .so); mkdir -p /m/$n; cp $f /m/$n/airsstack.so
  case $n in *-lua54) I=lua5.4;; *) I=luajit;; esac
  echo "-- $n via $I"
  $I /s/probe.lua /m/$n 2>&1; echo "exit=$?"
done
