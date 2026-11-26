-- Mirrors helpers/manifests.moon:68-71 (build_manifest filter) using the server's own ext/luarocks/deps.lua
local deps = require("ext.luarocks.deps")
local filters = {"5.1","5.2","5.3","5.4","5.5"}
local specs = {"lua == 5.4", "lua == 5.1", "lua >= 5.1", "lua >= 5.1, < 5.5", "lua ~> 5.4"}
for _, d in ipairs(specs) do
  local dep = deps.parse_dep(d)
  local row = {}
  for _, f in ipairs(filters) do
    local ok = dep and deps.match_constraints(deps.parse_version(f), dep.constraints)
    row[#row+1] = "manifest-"..f.."="..tostring(ok and true or false)
  end
  print(string.format("%-20s %s", d, table.concat(row, " ")))
end
