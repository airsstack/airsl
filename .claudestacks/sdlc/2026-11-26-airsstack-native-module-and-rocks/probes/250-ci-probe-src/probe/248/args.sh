#!/bin/bash
# Probe 248: the argument handling at the top of crates/airsl-lua/ci/manylinux.sh (spec §9.4), cut down to what it decides:
# three arguments -- the read-only checkout, the output directory, <version> from the `version` job -- and, per package, the
# rockspec path `luarocks make` gets (relative to the copied checkout, as probe 162's script used RS=) and the `pack` argument.
# The build itself (probe 160's y1-manylinux.sh) is replaced by echo.
set -euo pipefail
if [ $# -ne 3 ]; then
  echo "manylinux.sh: expected 3 arguments (checkout directory, output directory, version), got $#"
  exit 1
fi
SRC=$1; OUT=$2; VERSION=$3
echo "source $SRC, output $OUT"
for PKG in airsstack airsstack-luajit; do
  RS=crates/airsl-lua/rockspecs/$PKG-$VERSION-1.rockspec
  echo "$PKG: luarocks make $RS; luarocks pack $PKG $VERSION-1"
done
