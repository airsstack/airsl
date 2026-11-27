local env = {}
local f = assert(loadfile(arg[1], "t", env)); f()
local n = 0
for _ in pairs(env.repository) do n = n + 1 end
print("repository entries:", n)
for _, k in ipairs({ "airsstack", "airsstack-luajit", "luafilesystem" }) do print(k .. ":", env.repository[k] ~= nil) end
