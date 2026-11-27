#!/usr/bin/env bash
# Probe 236 (d): build-rocks' reading of <version> from the plan's airsl-cli release (present on pull requests and tags).
set -euo pipefail
version=$(jq -r '[.releases[]? | select(.app_name == "airsl-cli") | .app_version] | if length == 1 then .[0] else "" end' <<<"$PLAN") || { code=$?; echo "build-rocks: could not read the release plan: jq exit status $code"; exit 1; }
if [ -z "$version" ]; then
  echo "build-rocks: the release plan names no single airsl-cli release"; exit 1
fi
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "build-rocks: the release plan's airsl-cli version $version is not <major>.<minor>.<patch>"; exit 1
fi
echo "version=$version"
