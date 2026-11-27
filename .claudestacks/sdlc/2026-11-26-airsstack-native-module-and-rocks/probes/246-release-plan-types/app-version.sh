#!/usr/bin/env bash
# Probe 246: build-rocks' `version` job read of the plan's airsl-cli app_version with the plan's JSON types checked
# (spec §5.6.6). Reads the plan from $PLAN. Prints version=<version>, or one message and exit 1. (The file test after
# it is probe 236 (e)'s files.sh, unchanged.)
set -euo pipefail
read=$(jq -r -s '
  def word: {"null": "null", "boolean": "a boolean", "number": "a number", "string": "a string",
             "array": "an array", "object": "an object"}[type];
  if length == 0 then "none"
  elif length > 1 then "many \(length)"
  else .[0]
    | if type != "object" then "type the top level is \(word), not an object"
      elif .releases == null then "none"
      elif (.releases | type) != "array" then "type releases is \(.releases | word), not an array"
      else .releases
        | ([to_entries[] | select(.value | type != "object")] | first) as $bad
        | if $bad != null then "type releases[\($bad.key)] is \($bad.value | word), not an object"
          else [.[] | select(.app_name == "airsl-cli")]
            | if length != 1 then "none"
              elif (.[0].app_version | type) != "string" then "type the airsl-cli release'"'"'s app_version is \(.[0].app_version | word), not a string"
              elif .[0].app_version == "" then "type the airsl-cli release'"'"'s app_version is an empty string"
              else "version \(.[0].app_version)" end
          end
      end
  end' <<<"$PLAN") || { code=$?; echo "build-rocks: could not read the release plan: jq exit status $code"; exit 1; }
case $read in
  none) echo "build-rocks: the release plan names no single airsl-cli release"; exit 1 ;;
  "many "*) echo "build-rocks: the release plan holds ${read#many } JSON values, not one"; exit 1 ;;
  "type "*) echo "build-rocks: in the release plan, ${read#type }"; exit 1 ;;
esac
version=${read#version }
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "build-rocks: the release plan's airsl-cli version $version is not <major>.<minor>.<patch>"; exit 1
fi
echo "version=$version"
