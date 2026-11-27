-- Lua 5.4 keeps an integer and a float apart, and `json` keeps them apart too.
--
-- Run it with Lua 5.4 only: `math.type` does not exist in LuaJIT, whose numbers are all doubles.
-- The package that matches is `airsstack`; LuaJIT's counterpart is `airsstack-luajit`.

local airsstack = require("airsstack")
local json = airsstack.json

print("Lua version: " .. _VERSION)

-- `1` is an integer and `1.0` is a float, and the encoder writes each the way it was written.
local doc = { count = 1, ratio = 1.0, half = 0.5, big = 9007199254740993 }
print(json.encode(doc))

-- Decoding goes the other way: a JSON `1` comes back an integer, a JSON `1.0` a float.
local back = json.decode('{"whole":1,"fraction":1.0}')
print("whole:    " .. math.type(back.whole) .. " " .. tostring(back.whole))
print("fraction: " .. math.type(back.fraction) .. " " .. tostring(back.fraction))

-- 2^53 + 1 is the first integer a double cannot hold. As an integer it survives the round trip.
print("integer survives: " .. tostring(json.decode(json.encode(doc)).big == 9007199254740993))
print("integer division: " .. json.encode({ 7 // 2, 7 / 2 }))
