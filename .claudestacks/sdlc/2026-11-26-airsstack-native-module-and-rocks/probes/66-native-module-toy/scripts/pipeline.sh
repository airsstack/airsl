#!/bin/sh
# $1 = docker platform (linux/arm64|linux/amd64), $2 = arch label, $3 = manylinux image
# Every image is pinned to the digest of the requested platform's manifest, so two
# pipelines running side by side never share a locally retagged image.
set -u
PLAT=$1; A=$2; ML=$3
S=/private/tmp/claude-501/-Users-hiraq--herdr-worktrees-airsl-luarocks-integrations/4060dc68-5ed5-4b9d-947b-ba456a144f29/scratchpad/probes-D
OUT=$S/out-$A; rm -rf $OUT; mkdir -p $OUT
echo "### date: $(date -u +%Y-%m-%dT%H:%M:%SZ)  host: $(uname -sm)  docker: $(docker version --format '{{.Server.Version}}')"
MLD=$(sh $S/scripts/resolve.sh $ML $PLAT); BW=$(sh $S/scripts/resolve.sh rust:1-bookworm $PLAT)
BU=$(sh $S/scripts/resolve.sh debian:bullseye $PLAT); UB=$(sh $S/scripts/resolve.sh ubuntu:22.04 $PLAT)
[ -n "$MLD" ] || MLD=$ML@$(docker buildx imagetools inspect $ML | awk '/^Digest:/{print $2}')
for img in $MLD $BW $BU $UB; do
  docker pull -q --platform $PLAT $img >/dev/null
  echo "### image $img : arch=$(docker image inspect --format '{{.Architecture}}' $img)"
done
t0=$(date +%s)
echo; echo "### step 1: build in $ML"
docker run --name pD-$A-ml --platform $PLAT -v $S/toy:/toy:ro -v $OUT:/out -v $S/scripts:/s:ro $MLD sh /s/build-in.sh ml-$A 2>&1
docker rm pD-$A-ml >/dev/null
echo "### step 1 wall: $(( $(date +%s) - t0 ))s"; t0=$(date +%s)
echo; echo "### step 2: build in rust:1-bookworm"
docker run --name pD-$A-bw --platform $PLAT -v $S/toy:/toy:ro -v $OUT:/out -v $S/scripts:/s:ro $BW sh /s/build-in.sh bw-$A 2>&1
docker rm pD-$A-bw >/dev/null
echo "### step 2 wall: $(( $(date +%s) - t0 ))s"
echo; echo "### step 3: load in $ML (glibc 2.17) with Lua 5.4.7 / LuaJIT 2.1 built from source"
docker run --name pD-$A-l1 --platform $PLAT -v $OUT:/out:ro -v $S/scripts:/s:ro $MLD sh /s/load-in-src.sh 2>&1
docker rm pD-$A-l1 >/dev/null
for img in $BU $UB; do
  echo; echo "### step 4: load in ${img%@*} with apt lua5.4 / luajit"
  docker run --name pD-$A-l2 --platform $PLAT -v $OUT:/out:ro -v $S/scripts:/s:ro $img sh /s/load-in-apt.sh 2>&1
  docker rm pD-$A-l2 >/dev/null
done
echo "### done $(date -u +%Y-%m-%dT%H:%M:%SZ)"
