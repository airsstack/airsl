local skip = { _G = true, airsstack = true, arg = true }
local names = {}
for k in pairs(_G) do names[#names + 1] = k end
table.sort(names)
local base, libs = {}, {}
for _, k in ipairs(names) do
  local v = _G[k]
  if not skip[k] then
    if type(v) == "table" then
      local fns = {}
      for f in pairs(v) do fns[#fns + 1] = f end
      table.sort(fns)
      libs[#libs + 1] = k .. ": " .. table.concat(fns, " ")
    else
      base[#base + 1] = k
    end
  end
end
local out = "base: " .. table.concat(base, " ") .. "\n" .. table.concat(libs, "\n") .. "\n"
airsstack.stdio.write(out)
