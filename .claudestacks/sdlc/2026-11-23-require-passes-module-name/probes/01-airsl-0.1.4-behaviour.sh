#!/bin/sh
# Probes for chain 2026-11-23-require-passes-module-name, run against the airsl 0.1.4 checkout.
# usage: run-probes.sh <airsl-binary> <fixture-dir>
B="$1"; F="$2"
rm -rf "$F"; mkdir -p "$F/core/hooks/lib" "$F/lib" "$F/book/lib"

# P1: the issue's repro
printf 'local f = require("core.hooks.enforce")\nairsstack.stdio.write(f() .. "\\n")\n' > "$F/main.lua"
printf 'local base = (...):match("^(.-)[^.]*$")\nlocal lib = require(base .. "lib.x")\nreturn function() return "lib=" .. lib.v .. " base=[" .. base .. "]" end\n' > "$F/core/hooks/enforce.lua"
printf 'return { v = "ok" }\n' > "$F/core/hooks/lib/x.lua"
# P2: idiom pieces
printf 'for _, n in ipairs({"core.hooks.enforce", "enforce", "hooks.enforce"}) do\n  airsstack.stdio.write(n .. " -> base=[" .. n:match("^(.-)[^.]*$") .. "]\\n")\nend\n' > "$F/pattern.lua"
printf 'airsstack.stdio.write("from R: " .. require("core.hooks.lib.x").v .. "\\n")\n' > "$F/from_r.lua"
printf 'airsstack.stdio.write("from R/core/hooks: " .. require("lib.x").v .. "\\n")\n' > "$F/core/hooks/from_hooks.lua"
printf 'local function f(...) return (...) end\nairsstack.stdio.write(f("core.hooks.enforce", "/abs/path.lua") .. "\\n")\n' > "$F/paren.lua"
# P3: argument count, bare expression, both, syntax error
printf 'airsstack.stdio.write("count=" .. select("#", ...) .. "\\n")\nreturn true\n' > "$F/argc.lua"
printf 'require("argc")\n' > "$F/main_argc.lua"
printf '{ v = "bare" }\n' > "$F/bare.lua"
printf 'airsstack.stdio.write(require("bare").v .. "\\n")\n' > "$F/main_bare.lua"
printf 'tostring(1)\n' > "$F/both.lua"
printf 'airsstack.stdio.write(tostring(require("both")) .. "\\n")\n' > "$F/main_both.lua"
printf 'local x = \n' > "$F/broken.lua"
printf 'require("broken")\n' > "$F/main_broken.lua"
# P4: caching and return values, stock vs confined
printf 'RUNS = (RUNS or 0) + 1\nreturn { name = ... }\n' > "$F/lib/init.lua"
printf 'local a = require("lib")\nlocal b = require("lib.init")\nairsstack.stdio.write("runs=" .. RUNS .. " same=" .. tostring(a == b) .. "\\n")\n' > "$F/main_twice.lua"
printf 'return { v = 1 }\n' > "$F/tmod.lua"
printf 'NILRUNS = (NILRUNS or 0) + 1\n' > "$F/nilmod.lua"
printf 'FRUNS = (FRUNS or 0) + 1\nreturn false\n' > "$F/fmod.lua"
printf 'return select("#", ...), ...\n' > "$F/native.lua"
cat > "$F/stock.lua" <<EOF
package.path = "$F/?.lua"
local v, extra = require("native")
print("module argc=" .. tostring(v) .. " second result=" .. tostring(extra))
print("first  n=" .. select("#", require("tmod")))
print("cached n=" .. select("#", require("tmod")))
local a = require("nilmod"); local b = require("nilmod")
print("nil module: first=" .. tostring(a) .. " second=" .. tostring(b) .. " runs=" .. NILRUNS)
local c = require("fmod"); local d = require("fmod")
print("false module: first=" .. tostring(c) .. " second=" .. tostring(d) .. " runs=" .. FRUNS)
EOF
cat > "$F/confined.lua" <<EOF
airsstack.stdio.write("first  n=" .. select("#", require("tmod")) .. "\n")
airsstack.stdio.write("cached n=" .. select("#", require("tmod")) .. "\n")
local a = require("nilmod"); local b = require("nilmod")
airsstack.stdio.write("nil module: first=" .. tostring(a) .. " second=" .. tostring(b) .. " runs=" .. NILRUNS .. "\n")
local c = require("fmod"); local d = require("fmod")
airsstack.stdio.write("false module: first=" .. tostring(c) .. " second=" .. tostring(d) .. " runs=" .. FRUNS .. "\n")
EOF
# P5: the book example compiles
printf 'local M = {}\n\nfunction M.headings(text)\n  return airsstack.regex.find_all([[(?m)^#+ .+$]], text)\nend\n\nreturn M\n' > "$F/book/lib/markdown.lua"
cat > "$F/book/lib/toc.lua" <<'EOF'
-- lib/toc.lua
local name, path = ...
local base = name:match("^(.-)[^.]*$")
local markdown = require(base .. "markdown")

local M = {}

function M.print(text)
  print("-- " .. name .. " (" .. path:match("[^/]*$") .. ")")
  for _, heading in ipairs(markdown.headings(text)) do
    print(heading)
  end
end

return M
EOF
cat > "$F/book/toc.lua" <<'EOF'
-- toc.lua
require("lib.toc").print("# Shopping\n## Hardware\n")
EOF

echo "\$ $B --version"; "$B" --version
echo; echo "## P1 issue repro: airsl run --policy confined main.lua"; "$B" run --policy confined "$F/main.lua" 2>&1 | head -1
echo; echo "## P2 idiom pieces"
"$B" run --policy confined "$F/pattern.lua"
"$B" run --policy confined "$F/from_r.lua"
"$B" run --policy confined "$F/core/hooks/from_hooks.lua"
"$B" run --policy confined "$F/paren.lua"
echo; echo "## P3 module arguments and evaluation (confined)"
"$B" run --policy confined "$F/main_argc.lua"
echo "bare expression module: $("$B" run --policy confined "$F/main_bare.lua")"
echo "expression-and-statement module: $("$B" run --policy confined "$F/main_both.lua")"
echo "syntax error module:"; "$B" run --policy confined "$F/main_broken.lua" 2>&1 | head -1
echo; echo "## P4 caching and return values"
echo "same file, two names (confined): $("$B" run --policy confined "$F/main_twice.lua")"
echo "stock Lua 5.4 require (--policy trusted):"; "$B" run --policy trusted "$F/stock.lua"
echo "confined require:"; "$B" run --policy confined "$F/confined.lua"
echo; echo "## P5 book example compiles: airsl check book/"; "$B" check "$F/book"
echo "## P6 require under each policy"
printf 'airsstack.stdio.write(type(require) .. "\\n")\n' > "$F/typereq.lua"
for p in trusted confined pure; do echo "$p: $("$B" run --policy $p "$F/typereq.lua")"; done
echo "## P7 a module file that is not UTF-8"
printf 'return "\223"\n' > "$F/notutf8.lua"
printf 'require("notutf8")\n' > "$F/main_notutf8.lua"
"$B" run --policy confined "$F/main_notutf8.lua" 2>&1 | head -1
echo "## P8 require in a multi-value position, and a module returning two values"
printf 'return { v = "a" }\n' > "$F/ma.lua"
printf 'return { v = "b" }\n' > "$F/mb.lua"
printf 'return { v = 1 }, "extra"\n' > "$F/multi.lua"
cat > "$F/multipos_confined.lua" <<'EOT'
local parts = { require("ma"), require("mb") }
airsstack.stdio.write("constructor { require(ma), require(mb) } parts=" .. #parts .. "\n")
local ok, err = pcall(table.insert, {}, require("ma"))
airsstack.stdio.write("table.insert({}, require(ma)) ok=" .. tostring(ok) .. " " .. tostring(err) .. "\n")
airsstack.stdio.write("module returning two values: n=" .. select("#", require("multi")) .. " type=" .. type((require("multi"))) .. "\n")
EOT
cat > "$F/multipos_stock.lua" <<EOT
package.path = "$F/?.lua"
local parts = { require("ma"), require("mb") }
print("constructor { require(ma), require(mb) } parts=" .. #parts .. "  (mb is a first load)")
local ok, err = pcall(table.insert, {}, require("ma"))
print("table.insert({}, require(ma)) ok=" .. tostring(ok) .. " " .. tostring(err) .. "  (ma is a cache hit)")
local ok2, err2 = pcall(table.insert, {}, require("multi"))
print("table.insert({}, require(multi)) ok=" .. tostring(ok2) .. " " .. tostring(err2) .. "  (multi is a first load)")
print("module returning two values: type=" .. type((require("multi"))))
EOT
echo "confined:"; "$B" run --policy confined "$F/multipos_confined.lua"
echo "stock (--policy trusted):"; "$B" run --policy trusted "$F/multipos_stock.lua"
echo "## P9 refusals raised before a target is validated, and the main script's ..."
cat > "$F/badargs.lua" <<'EOT'
for _, case in ipairs({ { "table", {} }, { "nil", nil }, { "number 1", 1 }, { "non-UTF-8", "\147" } }) do
  local ok, err = pcall(require, case[2])
  airsstack.stdio.write(case[1] .. ": " .. tostring(ok) .. " " .. tostring(err):gsub("\n.*", "") .. "\n")
end
airsstack.stdio.write("main script select('#', ...)=" .. select("#", ...) .. "\n")
EOT
"$B" run --policy confined "$F/badargs.lua" x y
