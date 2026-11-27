#!/bin/bash
# Builds, packs and loads both Linux binary rocks inside a manylinux2014 container.
#
#   bash manylinux.sh <checkout directory> <output directory> <version>
#
# The checkout is only read (it may be mounted read-only), the two `.rock` files are
# written to the output directory, and <version> names the rockspecs
# `crates/airsl-lua/rockspecs/<package>-<version>-1.rockspec`. The caller has already
# checked that <version> is `<major>.<minor>.<patch>` and that both rockspecs exist,
# so this script only counts its arguments.
#
# It runs under `docker run` of the manylinux2014 image rather than as a job container:
# GitHub's node-based actions cannot run on that image's glibc 2.17, and a rock built
# on anything newer would carry a glibc floor the manylinux image exists to keep low.
# The one Linux script serves both architectures, x86_64 and aarch64.
#
# Lua 5.4, LuaJIT and LuaRocks are built from pinned sources, because the rocks are
# loaded against the interpreters this script built and a moving toolchain would let a
# green build change under it. LuaJIT itself is built with `make`; the `airsstack-luajit`
# rock is then built through the Lua 5.4 LuaRocks, pointed at LuaJIT with `--lua-dir`,
# because a LuaJIT-hosted LuaRocks could not load the build backend it had just installed.

set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "manylinux.sh: expected 3 arguments (checkout directory, output directory, version), got $#"
  exit 1
fi

SRC=$1
OUT=$(mkdir -p "$2" && cd "$2" && pwd)
VERSION=$3

LUA_SHA256=4f18ddae154e793e46eeab727c59ef1c0c0c2b744e7b94219710d76f530629ae
LUAROCKS_SHA256=b0e0c85205841ddd7be485f53d6125766d18a81d226588d2366931e9a1484492
LUAJIT_COMMIT=96f7b297326e9fd454b7186c6858b2773f94a0dc

# Runs a build step with its output in a log, and shows the log's tail only when the step
# fails, so a passing toolchain build does not bury the rock builds that follow it.
quiet() {
  local log=$1
  shift
  "$@" >"$log" 2>&1 || {
    local status=$?
    tail -n 60 "$log"
    return "$status"
  }
}

# A failed check names the file, so a changed upstream tarball is not mistaken for a
# network fault.
verify_sha256() {
  local file=$1 want=$2 got
  got=$(sha256sum "$file" | cut -d' ' -f1)
  if [ "$got" != "$want" ]; then
    echo "manylinux.sh: $file has SHA-256 $got, expected $want"
    exit 1
  fi
}

echo "== host: $(uname -m); $(ldd --version 2>&1 | head -1)"

if ! command -v unzip >/dev/null || ! command -v zip >/dev/null; then
  quiet /tmp/yum.log yum install -y -q unzip zip
fi

# rustup-init is a pinned binary, not the `sh.rustup.rs` script, which carries no checksum
# and always fetches the newest rustup. static.rust-lang.org publishes one `rustup-init` per
# triple under a versioned path; each triple's SHA-256 is pinned here, and the container's
# architecture picks the pair.
RUSTUP_VERSION=1.29.3
case $(uname -m) in
  x86_64)
    RUSTUP_TRIPLE=x86_64-unknown-linux-gnu
    RUSTUP_SHA256=b1c8cb14e30889e7e2999e6b69e62d9e9b9ffc0f4df8180564d306d9680e6c55
    ;;
  aarch64)
    RUSTUP_TRIPLE=aarch64-unknown-linux-gnu
    RUSTUP_SHA256=9f608c142e0e38a78957d0661ba1dfaf0b6fb37184cfaa4b9a0aa57af6184282
    ;;
  *)
    echo "manylinux.sh: no pinned rustup-init for the architecture $(uname -m)"
    exit 1
    ;;
esac
mkdir -p /tmp/rustup
curl --proto '=https' --tlsv1.2 -sSfL -o /tmp/rustup/rustup-init \
  "https://static.rust-lang.org/rustup/archive/$RUSTUP_VERSION/$RUSTUP_TRIPLE/rustup-init"
(cd /tmp/rustup && echo "$RUSTUP_SHA256  rustup-init" | sha256sum -c --quiet -) || {
  echo "manylinux.sh: rustup-init $RUSTUP_VERSION for $RUSTUP_TRIPLE does not have the pinned SHA-256 $RUSTUP_SHA256"
  exit 1
}
chmod +x /tmp/rustup/rustup-init

# No toolchain yet: the checkout's rust-toolchain.toml names the channel and the components
# (rustfmt, clippy), and `rustup toolchain install` with no argument installs exactly that,
# so the container builds with the toolchain every other job uses instead of a second pin
# that could drift from it. The checkout is only read, so it can be mounted read-only.
/tmp/rustup/rustup-init -y --profile minimal --default-toolchain none >/dev/null
# shellcheck source=/dev/null
. "$HOME/.cargo/env"
(cd "$SRC" && rustup toolchain install --no-self-update >/dev/null && rustc -V && cargo -V && cargo clippy -V && cargo fmt --version)

B=/tmp/toolchain
mkdir -p "$B"
cd "$B"

curl -sSfL -o lua.tgz https://www.lua.org/ftp/lua-5.4.8.tar.gz
verify_sha256 lua.tgz "$LUA_SHA256"
tar xzf lua.tgz
quiet /tmp/lua.log make -C lua-5.4.8 linux
quiet /tmp/lua-install.log make -C lua-5.4.8 install INSTALL_TOP=/opt/lua54

# A full clone, not a shallow one: the pinned commit stops being the branch tip as soon as
# upstream moves on, and `checkout` of a full commit id lands on it or fails.
git clone -q --branch v2.1 https://github.com/LuaJIT/LuaJIT.git lj
git -C lj checkout -q "$LUAJIT_COMMIT"
quiet /tmp/lj.log make -C lj PREFIX=/opt/luajit
quiet /tmp/lj-install.log make -C lj install PREFIX=/opt/luajit

curl -sSfL -o lr.tgz https://luarocks.github.io/luarocks/releases/luarocks-3.12.2.tar.gz
verify_sha256 lr.tgz "$LUAROCKS_SHA256"
mkdir lr
tar xzf lr.tgz -C lr --strip-components=1
(
  cd lr
  quiet /tmp/lr-configure.log ./configure --prefix=/opt/lr54 --with-lua=/opt/lua54
  quiet /tmp/lr-make.log make
  quiet /tmp/lr-install.log make install
)

/opt/lua54/bin/lua -v
/opt/luajit/bin/luajit -v
/opt/lr54/bin/luarocks --version | head -1

# The checkout may be read-only, and the build writes `target/` under it.
cp -r "$SRC" /tmp/repo
cd /tmp/repo
rm -rf target

LUAROCKS=/opt/lr54/bin/luarocks

# $1 package, $2 Lua version luarocks targets, $3 that interpreter's install prefix,
# $4 the interpreter, $5 the ABI name check.lua takes.
build_rock() {
  local package=$1 lua_version=$2 lua_dir=$3 interpreter=$4 abi=$5
  local rockspec=crates/airsl-lua/rockspecs/$package-$VERSION-1.rockspec
  local tree=/tmp/tree-$abi
  local so=$tree/lib/lua/$lua_version/airsstack.so
  local glibc

  echo "=== $package $VERSION-1 ($abi)"
  "$LUAROCKS" --lua-version "$lua_version" --lua-dir "$lua_dir" make --tree "$tree" "$rockspec"
  (cd "$OUT" && "$LUAROCKS" --lua-version "$lua_version" --lua-dir "$lua_dir" --tree "$tree" pack "$package" "$VERSION-1")
  "$interpreter" crates/airsl-lua/lua/check.lua "$tree/lib/lua/$lua_version" "$abi"

  # The highest glibc symbol version the module needs is the floor of the machines it loads on.
  glibc=$(objdump -T "$so" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -n 1 || true)
  echo "highest GLIBC symbol version of $so: ${glibc:-none}"
}

build_rock airsstack 5.4 /opt/lua54 /opt/lua54/bin/lua lua54
build_rock airsstack-luajit 5.1 /opt/luajit /opt/luajit/bin/luajit luajit

ls -la "$OUT"
