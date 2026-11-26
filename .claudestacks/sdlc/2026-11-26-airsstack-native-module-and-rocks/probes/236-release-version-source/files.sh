#!/usr/bin/env bash
# Probe 236 (e): build-rocks' file test -- both rockspecs named exactly from <version>, `airsstack` first.
# Usage: files.sh <checkout root> <version>
set -euo pipefail
cd "$1"; version=$2
for package in airsstack airsstack-luajit; do
  file="crates/airsl-lua/rockspecs/$package-$version-1.rockspec"
  if [ ! -f "$file" ]; then echo "build-rocks: $file is missing"; exit 1; fi
  echo "found $file"
done
