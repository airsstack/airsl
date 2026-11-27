#!/usr/bin/env bash
# Probe 246 driver. Usage: run.sh <directory holding tag.sh, app-version.sh, plan-tag.json, plan-pr.json>
# Every input of probes 236 (b)/(c) and 241 again (each must print what it printed there), then the new shapes.
d=$1
run() { # $1 = script, $2 = label shown after "$ PLAN=", $3 = the plan text
  printf '$ PLAN=%s bash %s\n' "$2" "$1"
  PLAN=$3 bash "$d/$1" 2>&1; printf '[exit %s]\n' "$?"
}
echo "## publish-rocks: tag.sh"
run tag.sh '<plan-tag.json>' "$(cat "$d/plan-tag.json")"
run tag.sh '<plan-pr.json>' "$(cat "$d/plan-pr.json")"
for p in '{"announcement_tag":"airsl-cli-v0.0.0","announcement_is_prerelease":false,"publish_prereleases":false,"artifacts":{}}' \
         '{"announcement_tag":null}' '{}' '' '{"announcement_tag":""}' '{"announcement_tag":"airsl-cli-v0.2"}' \
         '{"announcement_tag":"airsl-cli-v0.2.0-rc.1"}' '{"announcement_tag":"airsl-v0.2.0"}' \
         '{"announcement_tag":"airsl-cli-v0.2.0\n"}' '{"announcement_tag":"airsl-cli-v0.2.0
"}' 'not json' \
         '{"announcement_tag":5}' '{"announcement_tag":false}' '{"announcement_tag":true}' '{"announcement_tag":["airsl-cli-v0.2.0"]}' \
         '{"announcement_tag":{}}' '[1]' '5' '"x"' 'null' 'true' \
         '{"announcement_tag":"airsl-cli-v0.2.0"} {"announcement_tag":"airsl-cli-v0.2.0"}' '{} 5' '{"announcement_tag":"airsl-cli-v0.2.0"} not json'; do
  run tag.sh "'$p'" "$p"
done
echo "## build-rocks: app-version.sh"
run app-version.sh '<plan-tag.json>' "$(cat "$d/plan-tag.json")"
run app-version.sh '<plan-pr.json>' "$(cat "$d/plan-pr.json")"
for p in '{"releases":[{"app_name":"airsl","app_version":"0.2.0"},{"app_name":"airsl-cli","app_version":"0.2.0"}]}' \
         '{"releases":[]}' '{}' '' \
         '{"releases":[{"app_name":"airsl-cli","app_version":"0.2.0"},{"app_name":"airsl-cli","app_version":"0.2.0"}]}' \
         '{"releases":[{"app_name":"airsl-cli","app_version":"0.2"}]}' 'not json' \
         '{"releases":null}' '{"releases":5}' '{"releases":{"app_name":"airsl-cli"}}' '{"releases":"x"}' \
         '{"releases":[5]}' '{"releases":[{"app_name":"airsl-cli","app_version":"0.2.0"},null]}' \
         '{"releases":[{"app_name":"airsl-cli","app_version":5}]}' '{"releases":[{"app_name":"airsl-cli","app_version":null}]}' \
         '{"releases":[{"app_name":"airsl-cli"}]}' '{"releases":[{"app_name":"airsl-cli","app_version":["0.2.0"]}]}' \
         '{"releases":[{"app_name":"airsl-cli","app_version":""}]}' '{"releases":[{"app_name":5,"app_version":"0.2.0"}]}' \
         '[1]' '5' '"x"' 'null' 'false' '{} {}' '{"releases":[{"app_name":"airsl-cli","app_version":"0.2.0"}]} {}'; do
  run app-version.sh "'$p'" "$p"
done
