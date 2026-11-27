-- Loads the built `airsstack` native module into the interpreter running this script and checks
-- what the module promises: the eleven modules, trusted authority, no global and no `ext`.
--
-- usage: <interpreter> check.lua <directory holding the built airsstack library> <lua54|luajit>
--
-- The interpreter is checked against the ABI argument before anything is required. A library
-- built for one Lua version does not load into another, so without that check a mismatched pair
-- would fail on `require` with a message that names neither side.
--
-- The name matches neither `*_test.lua` nor `test_*.lua`, so `airsl test` never collects it.

local USAGE = "check.lua: usage: <interpreter> check.lua "
  .. "<directory holding the built airsstack library> <lua54|luajit>"

local dir, abi = arg and arg[1], arg and arg[2]
if dir == nil or (abi ~= "lua54" and abi ~= "luajit") then
  error(USAGE)
end

local jit = rawget(_G, "jit")
if abi == "lua54" and (_VERSION ~= "Lua 5.4" or type(jit) == "table") then
  error("check.lua: this check is for lua54, but the interpreter is " .. _VERSION)
end
if abi == "luajit" and type(jit) ~= "table" then
  error("check.lua: this check is for luajit, but the interpreter is "
    .. _VERSION .. " without jit")
end

package.cpath = dir .. "/?.so;" .. dir .. "/?.dll;" .. package.cpath

local loaded, root = pcall(require, "airsstack")
if not loaded then
  error('check.lua: require("airsstack") failed: ' .. tostring(root))
end
if type(root) ~= "table" then
  error('check.lua: require("airsstack") returned a ' .. type(root) .. ", not a table")
end
if rawget(_G, "airsstack") ~= nil then
  error('check.lua: require("airsstack") set the global airsstack')
end
if package.loaded.airsstack ~= root then
  error("check.lua: package.loaded.airsstack is not the table require returned")
end
if root.ext ~= nil then
  error("check.lua: ext is present, but it exists only inside airsl")
end

local MODULES = {
  "json", "path", "fs", "env", "proc", "regex", "hash", "time", "glob", "stdio", "hook",
}
for _, name in ipairs(MODULES) do
  if type(root[name]) ~= "table" then
    error("check.lua: module " .. name .. " is missing")
  end
end

local read, problem = pcall(root.fs.read, arg[0])
if not read then
  error("check.lua: fs.read could not read the check script itself: " .. tostring(problem))
end

-- Lua 5.4 keeps `2.0` a float, LuaJIT has one number type, so the same table encodes differently.
local want = type(jit) == "table" and "[2,2]" or "[2,2.0]"
local got = root.json.encode({ 2, 2.0 })
if got ~= want then
  error("check.lua: json.encode({2, 2.0}) gave " .. tostring(got) .. ", expected " .. want)
end

if type(jit) == "table" and utf8 ~= nil then
  error("check.lua: utf8 is a " .. type(utf8) .. " under LuaJIT, expected nil")
end

if type(jit) == "table" then
  print("check.lua: ok (" .. _VERSION .. ", " .. jit.version .. ")")
else
  print("check.lua: ok (" .. _VERSION .. ")")
end
