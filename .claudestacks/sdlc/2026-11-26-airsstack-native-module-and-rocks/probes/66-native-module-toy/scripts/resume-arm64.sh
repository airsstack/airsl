#!/bin/sh
# Steps 2-4 of the aarch64 pipeline, one container at a time, each limited to 2 CPUs / 4 GiB
# and CARGO_BUILD_JOBS=2 (the host was overloaded by parallel containers).
set -u
S=/private/tmp/claude-501/-Users-hiraq--herdr-worktrees-airsl-luarocks-integrations/4060dc68-5ed5-4b9d-947b-ba456a144f29/scratchpad/probes-D
OUT=$S/out-arm64; PLAT=linux/arm64; LIM="--cpus 2 --memory 4g -e CARGO_BUILD_JOBS=2"
MLD=quay.io/pypa/manylinux2014_aarch64@sha256:382236bfd11d86894d9792a6fa5286190b8dd3d075a7032284e123c288711e4f
BW=docker.io/library/rust:1-bookworm@sha256:b820ae3ce24753f523e6f50391196a3ef2474b4bc067f1ff80080a4b000c6961
BU=docker.io/library/debian:bullseye@sha256:32fc609441a9c8417b2bb5faa62a60afd6af19013e8e82fdeed5d08dc7ac1ad9
UB=docker.io/library/ubuntu:22.04@sha256:2926e8e6ef0b816e19849e33ccc728c72b921a428286e6595d52fa92c5716626
echo "### resumed $(date -u +%Y-%m-%dT%H:%M:%SZ) with: docker run $LIM"
t0=$(date +%s)
echo; echo "### step 2: build in rust:1-bookworm"
rm -f $OUT/bw-*
docker run --name pD-arm64-bw $LIM --platform $PLAT -v $S/toy:/toy:ro -v $OUT:/out -v $S/scripts:/s:ro $BW sh /s/build-in.sh bw-arm64 2>&1
docker rm pD-arm64-bw >/dev/null
echo "### step 2 wall: $(( $(date +%s) - t0 ))s"
echo; echo "### step 3: load in manylinux2014_aarch64 (glibc 2.17) with Lua 5.4.7 / LuaJIT 2.1 built from source"
docker run --name pD-arm64-l1 $LIM --platform $PLAT -v $OUT:/out:ro -v $S/scripts:/s:ro $MLD sh /s/load-in-src.sh 2>&1
docker rm pD-arm64-l1 >/dev/null
for img in $BU $UB; do
  echo; echo "### step 4: load in ${img%@*} with apt lua5.4 / luajit"
  docker run --name pD-arm64-l2 $LIM --platform $PLAT -v $OUT:/out:ro -v $S/scripts:/s:ro $img sh /s/load-in-apt.sh 2>&1
  docker rm pD-arm64-l2 >/dev/null
done
echo "### done $(date -u +%Y-%m-%dT%H:%M:%SZ)"
