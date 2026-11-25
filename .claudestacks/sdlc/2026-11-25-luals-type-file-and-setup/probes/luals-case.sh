#!/bin/sh
# usage: luals-case.sh <airsl> <workdir> <name> <luarc-json-or-NONE> <run-setup-after: yes|no>
AIRSL=$1; W=$2; NAME=$3; LUARC=$4; SETUP=$5
LS=$HOME/.local/share/nvim/mason/bin/lua-language-server
D=$W/$NAME; rm -rf "$D"; mkdir -p "$D"; cd "$D" || exit 1
git init -q .; printf '.airsl/\n' > .gitignore; printf 'print(airsstack.json.nope)\n' > nope.lua
"$AIRSL" lua setup . >/dev/null || exit 1
if [ "$LUARC" = NONE ]; then rm -f .luarc.json; else printf '%s' "$LUARC" > .luarc.json; fi
echo "## $NAME  .luarc.json before: $( [ -f .luarc.json ] && cat .luarc.json || echo '(none)')"
if [ "$SETUP" = yes ]; then echo "\$ airsl lua setup ."; "$AIRSL" lua setup . 2>&1; echo; echo "--- .luarc.json after:"; cat .luarc.json; fi
"$LS" --check . --checklevel=Information 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | tr '\r' '\n' | grep -E 'Undefined|Diagnosis complete' | sed "s|$D/||"
