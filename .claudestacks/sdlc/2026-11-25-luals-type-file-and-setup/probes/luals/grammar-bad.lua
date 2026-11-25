local a = airsstack.json.encode()
local b = airsstack.fs.stat("x").nope
local c = airsstack.hash.hash_file("x", "md5")
local d = airsstack.json.nope
local e = airsstack["end"].nope
local f = airsstack.fs.list(42)
local g = airsstack.env.get("HOME"):upper()
print(a, b, c, d, e, f, g)
