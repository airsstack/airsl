#!/bin/sh
# usage: luals-require.sh <airsl> <workdir>
# One case per directory: `airsl lua setup`, then an edit to the generated type file standing in
# for a candidate shape, then LuaLS --check. Each case prints the type file's first line and tail.
AIRSL=$1; W=$2
LS=$HOME/.local/share/nvim/mason/bin/lua-language-server
echo "LuaLS: $("$LS" --version)"
case_() { # name root edit userfile
  NAME=$1; ROOT=$2; EDIT=$3; USER=$4
  D=$W/$NAME; rm -rf "$D"; mkdir -p "$D"; cd "$D" || exit 1
  git init -q .; printf '.airsl/\n' > .gitignore
  "$AIRSL" lua setup . >/dev/null || exit 1
  T=.airsl/types/airsstack.lua
  if [ "$ROOT" != airsstack ]; then sed "s/airsstack/$ROOT/g" $T > .airsl/types/$ROOT.lua; rm $T; T=.airsl/types/$ROOT.lua; fi
  case $EDIT in
    none) ;;
    return) printf '\nreturn %s\n' "$ROOT" >> $T ;;
    named) sed -i '' "1s/^---@meta\$/---@meta $ROOT/" $T; printf '\nreturn %s\n' "$ROOT" >> $T ;;
  esac
  [ "$USER" = yes ] && printf 'return { from_user_file = true }\n' > $ROOT.lua
  printf 'local a = require("%s")\nprint(a.json.nope)\nprint(%s.json.nope)\nprint(a.json.encode)\nlocal b = require("airsstack")\nprint(b.json.nope)\n' "$ROOT" "$ROOT" > main.lua
  echo "== $NAME  (root $ROOT, type file edit: $EDIT, user $ROOT.lua: $USER)"
  echo "type file line 1: $(head -1 $T)   last line: $(tail -1 $T)"
  echo "--- main.lua"; cat -n main.lua
  echo "\$ lua-language-server --check . --checklevel=Information"
  "$LS" --check . --checklevel=Information 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | tr '\r' '\n' | grep -E '^main|^airsstack|^agentstacks|Diagnosis complete' | sed "s|$D/||"
}
case_ control     airsstack   none   no
case_ return      airsstack   return no
case_ named       airsstack   named  no
case_ return_user airsstack   return yes
case_ named_user  airsstack   named  yes
case_ host_return agentstacks return no
case_ host_named  agentstacks named  no
