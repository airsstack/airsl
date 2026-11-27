-- Prepends the LuaJIT toy build dir (given as arg[1]) to package.cpath and loads the module.
package.cpath = arg[1] .. "/?.so;" .. package.cpath
local m = require("airsstack")
print("ok=" .. tostring(m.ok), "jit=" .. tostring(jit and jit.version), "_VERSION=" .. _VERSION)
