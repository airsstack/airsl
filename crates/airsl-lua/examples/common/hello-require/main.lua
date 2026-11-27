-- The smallest use of the native module: load it, look at what came back, use three modules.
--
-- `require("airsstack")` returns one table and sets no global, so a script keeps it in a local.
-- Everything is printed with `print`, never mixed with `airsstack.stdio.write`: under LuaJIT the
-- two are separate buffers, and a run piped into a file could reorder their lines.

local airsstack = require("airsstack")

-- `pairs` walks a table in an order that differs between runs, so the names are sorted before
-- they are printed.
local names = {}
for name in pairs(airsstack) do
  names[#names + 1] = name
end
table.sort(names)
print("modules: " .. table.concat(names, ", "))

-- `ext` exists only inside the `airsl` program, which is what fires extension events.
print("ext: " .. tostring(airsstack.ext))

-- The module is reachable through `require` and nothing else.
print("global airsstack: " .. tostring(rawget(_G, "airsstack")))
print("package.loaded holds the same table: " .. tostring(package.loaded.airsstack == airsstack))
print("a second require returns it again: " .. tostring(require("airsstack") == airsstack))

-- `{ 1, 2.5 }` has no integer-or-float question to answer, so both Lua versions print the same
-- line. The examples under `lua54/` and `luajit/` are about the numbers that do differ.
print(airsstack.json.encode({ name = "airsl", numbers = { 1, 2.5 } }))
print(airsstack.path.join("src", "main.lua"))
print(airsstack.hash.sha256("abc"))
