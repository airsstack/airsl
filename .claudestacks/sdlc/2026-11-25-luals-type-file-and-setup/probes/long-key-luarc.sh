#!/bin/sh
# usage: long-key-luarc.sh <dots> <dir>
# Writes <dir>/.luarc.json: one key of <dots>+1 `k` segments set to 1, then "z.y": 1 and
# "z": {"y": 2}, which conflict on `z.y`, so setup refuses after walking the long key.
DOTS=$1; DIR=$2; mkdir -p "$DIR"
awk -v n="$DOTS" 'BEGIN { k = "k"; for (i = 0; i < n; i++) k = k ".k"; printf "{\"%s\": 1, \"z.y\": 1, \"z\": {\"y\": 2}}", k }' > "$DIR/.luarc.json"
