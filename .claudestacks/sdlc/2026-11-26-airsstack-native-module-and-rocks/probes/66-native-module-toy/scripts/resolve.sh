#!/bin/sh
# $1 = image tag, $2 = platform (linux/amd64|linux/arm64) -> prints repo@sha256 of that platform's manifest
docker buildx imagetools inspect "$1" | awk -v p="$2" '/^  Name:/{n=$2} /^  Platform:/{ if ($2==p || $2==p"/v8") {print n; exit} }'
