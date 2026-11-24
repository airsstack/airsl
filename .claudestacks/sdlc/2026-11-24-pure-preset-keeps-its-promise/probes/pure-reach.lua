print("print:", type(print), "stdio:", type(airsstack.stdio), "hook:", type(airsstack.hook), "time:", type(airsstack.time))
print("now:", pcall(airsstack.time.now))
print("absolute:", pcall(airsstack.path.absolute, "x"))
print("hash_file:", pcall(airsstack.hash.hash_file, "p.lua"))
print("walk:", pcall(airsstack.glob.walk, "."))
print("env:", pcall(airsstack.env.get, "HOME"))
print("stdio.write:", pcall(airsstack.stdio.write, "<via stdio>\n"))
print("random:", math.random(1, 1000000))
local t = {}
for _, k in ipairs({"alpha","beta","gamma","delta","eps","zeta","eta","theta"}) do t[k] = true end
local o = {}
for k in pairs(t) do o[#o + 1] = k end
print("pairs:", table.concat(o, ","))
print("tostring:", tostring({}))
