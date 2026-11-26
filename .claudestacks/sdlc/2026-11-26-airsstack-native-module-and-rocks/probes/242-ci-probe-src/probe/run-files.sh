#!/usr/bin/env bash
# Probe driver: runs files.sh (build-rocks' file test, probe 236 (e)) on the checkout for 0.2.0 and 0.3.0.
# Usage: run-files.sh <directory holding files.sh> <checkout root>
for v in 0.2.0 0.3.0; do
  printf '$ bash files.sh <checkout> %s\n' "$v"
  bash "$1/files.sh" "$2" "$v" 2>&1; printf '[exit %s]\n' "$?"
done
