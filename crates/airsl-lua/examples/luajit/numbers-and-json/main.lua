-- LuaJIT has one number type, and `json` can only encode what it is given.
--
-- Run it with LuaJIT only. There is no `math.type` and no integer subtype here, so `1.0` is the
-- same value as `1` and is written as `1`. The package that matches is `airsstack-luajit`;
-- Lua 5.4's counterpart is `airsstack`.

local airsstack = require("airsstack")
local json = airsstack.json

print("Lua version: " .. _VERSION)
print("jit: " .. tostring(type(rawget(_G, "jit")) == "table"))
print("math.type: " .. tostring(math.type))

-- The same table that Lua 5.4 encodes as `"ratio":1.0` comes out with a plain `1`.
print(json.encode({ count = 1, ratio = 1.0, half = 0.5 }))

-- A whole number that was a float is a whole number afterwards, and so is `tostring` of it.
local back = json.decode('{"whole":1,"fraction":1.0}')
print("whole:    " .. tostring(back.whole))
print("fraction: " .. tostring(back.fraction))
print("same value: " .. tostring(back.whole == back.fraction))

-- A double holds every integer up to 2^53 exactly. Beyond that it rounds, which is why an id
-- that large belongs in a string.
print("2^53 + 1 is the same number as 2^53: " .. tostring(2 ^ 53 + 1 == 2 ^ 53))
print(json.encode({ "9007199254740993" }))
