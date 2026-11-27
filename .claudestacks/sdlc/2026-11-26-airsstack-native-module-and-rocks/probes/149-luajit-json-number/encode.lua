-- The record of crates/airsl/examples/values-and-json/encode.lua with arg = {"widget", "3"},
-- encoded through the toy module's airsstack.json (airsl's Json host module, convert::to_json).
local airsstack = require("airsstack")
local arg = { "widget", "3" }
local record = {
  name = arg[1],
  count = tonumber(arg[2]),
  observed_at = 1700000000,
  tags = { "beta", "alpha" },
  meta = {},
}
print(_VERSION, jit and jit.version or "")
print("compact: " .. airsstack.json.encode(record))
print("pretty:\n" .. airsstack.json.encode_pretty(record))
print("observed_at via tostring: " .. tostring(record.observed_at))
print("2^53 + 0.0 => " .. airsstack.json.encode({ v = 2^53 }))
print("1.5 => " .. airsstack.json.encode({ v = 1.5 }))
print("1e300 => " .. airsstack.json.encode({ v = 1e300 }))
local d = airsstack.json.decode('{"observed_at":1700000000}')
print("decode back, math.type: " .. tostring(math.type and math.type(d.observed_at) or "n/a (no math.type)") .. " value " .. tostring(d.observed_at))
