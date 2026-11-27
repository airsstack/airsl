#!/usr/bin/env bash
# Probe driver: builds work/<case>/ for the rocks lookups, each a cwd holding Makefile.toml and crates/airsl-lua/rockspecs.
# real = the two rockspecs; leftover = real plus airsstack-0.1.0-1.rockspec; empty = none;
# malformed = airsstack.rockspec, airsstack-0.2.0.rockspec, airsstack-luajit-0.2.0.rockspec (probe 237's fixtures).
set -euo pipefail
src=crates/airsl-lua/rockspecs
for c in real leftover empty malformed; do
  mkdir -p "work/$c/$src"; cp probe/Makefile.toml "work/$c/"
done
cp $src/*.rockspec work/real/$src/
cp $src/*.rockspec work/leftover/$src/; cp $src/airsstack-0.2.0-1.rockspec work/leftover/$src/airsstack-0.1.0-1.rockspec
cp $src/airsstack-0.2.0-1.rockspec work/malformed/$src/airsstack.rockspec
cp $src/airsstack-0.2.0-1.rockspec work/malformed/$src/airsstack-0.2.0.rockspec
cp $src/airsstack-luajit-0.2.0-1.rockspec work/malformed/$src/airsstack-luajit-0.2.0.rockspec
for c in real leftover empty malformed; do echo "\$ ls work/$c/$src"; ls "work/$c/$src"; done
