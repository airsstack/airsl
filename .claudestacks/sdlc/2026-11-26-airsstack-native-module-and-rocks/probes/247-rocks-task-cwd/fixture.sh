#!/usr/bin/env bash
# Probe 247 fixture: a tree under $1 holding the Makefile above and the two relative paths the rocks task uses.
set -euo pipefail
F=$1; rm -rf "$F"; mkdir -p "$F"/crates/airsl-lua/rockspecs "$F"/crates/airsl-lua/lua
cp "$2" "$F"/Makefile.toml
touch "$F"/crates/airsl-lua/rockspecs/airsstack-0.2.0-1.rockspec "$F"/crates/airsl-lua/rockspecs/airsstack-luajit-0.2.0-1.rockspec
touch "$F"/crates/airsl-lua/lua/check.lua
