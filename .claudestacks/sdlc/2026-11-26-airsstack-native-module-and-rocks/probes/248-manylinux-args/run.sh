#!/bin/bash
# Probe 248 driver. Usage: run.sh <directory holding args.sh>
d=$1
run() { printf '$ bash args.sh%s\n' "$( [ $# -gt 0 ] && printf ' %q' "$@")"; bash "$d/args.sh" "$@" 2>&1; printf '[exit %s]\n' "$?"; }
run
run /src /out
run /src /out 0.2.0
run /src /out 0.2.0 extra
