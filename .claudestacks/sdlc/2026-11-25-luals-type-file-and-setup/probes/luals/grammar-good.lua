local s = airsstack.json.encode({ a = 1 })
local p = airsstack.path.join("a", "b", "c")
local st = airsstack.fs.stat("x")
local size = st.size + 1
local k = st.kind
local made = airsstack.fs.create_exclusive("x")
for _, n in ipairs(airsstack.fs.list(".")) do print(n:upper()) end
local v = airsstack.env.get("HOME")
for name, value in pairs(airsstack.env.all()) do print(name:upper(), value:upper()) end
local h = airsstack.hash.hash_file("x", "sha1")
local re = airsstack.regex.compile("a+")
local ok = re.is_match("aaa")
airsstack.ext.on("PreToolUse", function(payload) return payload end)
local hi = airsstack["end"].hello("me")
print(s, p, size, k, made, v, h, ok, hi:upper())
