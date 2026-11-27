#!/bin/bash
# Fails unless both binary rocks of a platform are in the output directory.
#
#   bash rocks-present.sh <output directory> <version> <arch>
#
# <arch> is the platform tag in the rock's file name, as in
# `airsstack-<version>-1.<arch>.rock`.
#
# It is one script because all five build legs end with this check, on three kinds of
# runner, and a copy per leg is a copy that can drift. It is a check of its own, after the
# build, because the upload step's `if-no-files-found: error` is satisfied by any one file:
# a build that packed only one of the two rocks would otherwise upload half a platform.
# `bash` is on every runner, Windows included, through Git for Windows.

set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "rocks-present.sh: expected 3 arguments (output directory, version, arch), got $#"
  exit 1
fi

out=$1
version=$2
arch=$3

for package in airsstack airsstack-luajit; do
  rock="$out/$package-$version-1.$arch.rock"
  if [ ! -f "$rock" ]; then
    echo "::error::build-rocks: $rock is missing"
    ls -la "$out"
    exit 1
  fi
done
