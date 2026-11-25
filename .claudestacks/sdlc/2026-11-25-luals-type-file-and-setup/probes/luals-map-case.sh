#!/bin/sh
# usage: luals-map-case.sh <airsl> <workdir> <name> <luarc-json> <run-setup-after: yes|no>
# Like luals-case.sh, plus io.lua, which uses the `io` global: a luarc that disables the `io`
# builtin makes LuaLS report it, which shows the map-valued `runtime.builtin` setting was applied.
AIRSL=$1; W=$2; NAME=$3; LUARC=$4; SETUP=$5
LS=$HOME/.local/share/nvim/mason/bin/lua-language-server
D=$W/$NAME; rm -rf "$D"; mkdir -p "$D"; cd "$D" || exit 1
git init -q .; printf '.airsl/\n' > .gitignore; printf 'print(airsstack.json.nope)\n' > nope.lua
printf 'io.write("x")\n' > io.lua
"$AIRSL" lua setup . >/dev/null || exit 1
printf '%s' "$LUARC" > .luarc.json
echo "## $NAME  .luarc.json before: $(cat .luarc.json)"
if [ "$SETUP" = yes ]; then echo "\$ airsl lua setup ."; "$AIRSL" lua setup . 2>&1; echo "exit $?"; echo "--- .luarc.json after:"; cat .luarc.json; echo; fi
"$LS" --check . --checklevel=Information 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | tr '\r' '\n' | grep -E 'Undefined|Diagnosis complete' | sed "s|$D/||"
