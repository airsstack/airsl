#!/usr/bin/env bash
# Probe 246: publish-rocks' tag read with the plan's JSON types checked (spec §5.6.4). Reads the plan from $PLAN, as the
# job does (`env: PLAN: ${{ inputs.plan }}`, probe 220:57). Prints version=<version>, or one message and exit 1.
# jq -s slurps every JSON text of the input into one array, so an empty plan is [] and two texts are counted.
set -euo pipefail
read=$(jq -r -s '
  def word: {"null": "null", "boolean": "a boolean", "number": "a number", "string": "a string",
             "array": "an array", "object": "an object"}[type];
  if length == 0 then "none"
  elif length > 1 then "many \(length)"
  else .[0]
    | if type != "object" then "type the top level is \(word), not an object"
      elif .announcement_tag == null or .announcement_tag == "" then "none"
      elif (.announcement_tag | type) != "string" then "type announcement_tag is \(.announcement_tag | word), not a string"
      else "tag \(.announcement_tag)" end
  end' <<<"$PLAN") || { code=$?; echo "publish-rocks: could not read the release plan: jq exit status $code; nothing was uploaded"; exit 1; }
case $read in
  none) echo "publish-rocks: the release plan names no tag; nothing was uploaded"; exit 1 ;;
  "many "*) echo "publish-rocks: the release plan holds ${read#many } JSON values, not one; nothing was uploaded"; exit 1 ;;
  "type "*) echo "publish-rocks: in the release plan, ${read#type }; nothing was uploaded"; exit 1 ;;
esac
tag=${read#tag }
if [[ ! "$tag" =~ ^airsl-cli-v([0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
  echo "publish-rocks: the release tag $tag is not airsl-cli-v<major>.<minor>.<patch>; nothing was uploaded"; exit 1
fi
version=${BASH_REMATCH[1]}
echo "version=$version"
