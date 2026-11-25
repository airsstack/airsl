local s, xs = airsstack.p.f(nil, nil, { [1] = "a", k = "b" }, "x", { n = "n" })
if s then print(s:upper(), xs[1] + 1) end
if airsstack.p.g then print(airsstack.p.g():upper()) end
if airsstack.p.m then print(airsstack.p.m + 1) end
local ou = airsstack.p.ou()
print(ou)
local fu = airsstack.p.fu()
print(fu)
airsstack.p.lits({ "a", 1, nil }, "a|b", "x?", "s")
print(airsstack["end"].get().v + 1)
print(airsstack.p.join("a", "b"):upper())
