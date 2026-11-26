#!/usr/bin/env bash
# Probe 241 driver: runs tag.sh on every plan of probe 236 (b), printing each command, its output and exit status.
# The newline case is run twice: as the JSON escape \n (what probe 236 ran) and as a raw newline inside the JSON string.
# Usage: run.sh <directory holding tag.sh, plan-tag.json, plan-pr.json>
d=$1
run() { # $1 = label shown after "$ PLAN=", $2 = the plan text
  printf '$ PLAN=%s bash tag.sh\n' "$1"
  PLAN=$2 bash "$d/tag.sh" 2>&1; printf '[exit %s]\n' "$?"
}
run '<plan-tag.json>' "$(cat "$d/plan-tag.json")"
run '<plan-pr.json>' "$(cat "$d/plan-pr.json")"
for p in '{"announcement_tag":"airsl-cli-v0.0.0","announcement_is_prerelease":false,"publish_prereleases":false,"artifacts":{}}' \
         '{"announcement_tag":null}' '{}' '' '{"announcement_tag":""}' '{"announcement_tag":"airsl-cli-v0.2"}' \
         '{"announcement_tag":"airsl-cli-v0.2.0-rc.1"}' '{"announcement_tag":"airsl-v0.2.0"}' '{"announcement_tag":5}' \
         '{"announcement_tag":"airsl-cli-v0.2.0\n"}' '{"announcement_tag":"airsl-cli-v0.2.0
"}' 'not json'; do
  run "'$p'" "$p"
done
