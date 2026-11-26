-- Probe 188: a second process is a second state; the opens counter restarts (static is per process image).
package.cpath = arg[1] .. "/?.so;" .. package.cpath
local a = require("airsstack")
print("process " .. arg[2] .. ":", "version=" .. a.version, "opens=" .. a.opens)
