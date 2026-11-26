-- Probe 239 (b): luarocks 3.12.2's own reading of the same names (path.parse_name, path.lua:174-180).
local path = require("luarocks.path")
for _, n in ipairs(arg) do
  local name, version = path.parse_name(n)
  print(string.format("%-36s -> %s %s", n, tostring(name), tostring(version)))
end
