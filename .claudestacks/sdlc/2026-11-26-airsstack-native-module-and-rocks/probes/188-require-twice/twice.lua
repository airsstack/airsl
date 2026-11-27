-- Probe 188: one lua5.4 state; the probe 181 toy (0.0.2) at ./airsstack.so.
package.cpath = arg[1] .. "/?.so;" .. package.cpath
local a = require("airsstack")
print("1st require:", a, "version=" .. a.version, "opens=" .. a.opens)
local b = require("airsstack")
print("2nd require:", b, "rawequal(a, b)=" .. tostring(rawequal(a, b)), "opens=" .. b.opens)
print("package.loaded.airsstack == a:", rawequal(package.loaded.airsstack, a))
print("global airsstack:", rawget(_G, "airsstack"))
package.loaded.airsstack = nil
local ok, c = pcall(require, "airsstack")
print("after package.loaded.airsstack = nil, require ok=" .. tostring(ok), c, "rawequal(a, c)=" .. tostring(rawequal(a, c)), "opens=" .. (ok and c.opens or "?"))
local f1, e1 = package.loadlib(arg[1] .. "/airsstack.so", "luaopen_airsstack")
local f2, e2 = package.loadlib(arg[1] .. "/airsstack.so", "luaopen_airsstack")
print("loadlib twice:", type(f1), e1, type(f2), e2, "rawequal(f1, f2)=" .. tostring(rawequal(f1, f2)))
local d, e = f1(), f2()
print("calling both:", d, e, "opens=" .. d.opens .. "," .. e.opens, "rawequal(d, e)=" .. tostring(rawequal(d, e)))
d.ok = false
print("mutating one table leaves the other:", d.ok, e.ok, a.ok)
