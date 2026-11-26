#!/usr/bin/env bash
# Probe driver: runs app-version.sh (build-rocks' read, probe 236 (c)) on every plan of probe 236 (c).
# Usage: run-app.sh <directory holding app-version.sh, plan-tag.json, plan-pr.json>
d=$1
run() { # $1 = label shown after "$ PLAN=", $2 = the plan text
  printf '$ PLAN=%s bash app-version.sh\n' "$1"
  PLAN=$2 bash "$d/app-version.sh" 2>&1; printf '[exit %s]\n' "$?"
}
run '<plan-tag.json>' "$(cat "$d/plan-tag.json")"
run '<plan-pr.json>' "$(cat "$d/plan-pr.json")"
for p in '{"releases":[]}' '{}' '' \
         '{"releases":[{"app_name":"airsl","app_version":"0.2.0"},{"app_name":"airsl-cli","app_version":"0.2.0"}]}' \
         '{"releases":[{"app_name":"airsl-cli","app_version":"0.2"}]}' \
         '{"releases":[{"app_name":"airsl-cli","app_version":"0.2.0"},{"app_name":"airsl-cli","app_version":"0.3.0"}]}' \
         'not json'; do
  run "'$p'" "$p"
done
