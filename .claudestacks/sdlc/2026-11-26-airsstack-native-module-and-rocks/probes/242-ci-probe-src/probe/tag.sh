#!/usr/bin/env bash
# Probe 241: probe 236's tag.sh with the unreadable-plan message ending like the other two ("; nothing was uploaded").
# Reads the plan JSON from $PLAN (the job passes `inputs.plan` so, probe 220:57). Prints the version or the message; exit 1 on refusal.
set -euo pipefail
tag=$(jq -r '.announcement_tag // ""' <<<"$PLAN") || { code=$?; echo "publish-rocks: could not read the release plan: jq exit status $code; nothing was uploaded"; exit 1; }
if [ -z "$tag" ]; then
  echo "publish-rocks: the release plan names no tag; nothing was uploaded"; exit 1
fi
if [[ ! "$tag" =~ ^airsl-cli-v([0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
  echo "publish-rocks: the release tag $tag is not airsl-cli-v<major>.<minor>.<patch>; nothing was uploaded"; exit 1
fi
version=${BASH_REMATCH[1]}
echo "version=$version"
