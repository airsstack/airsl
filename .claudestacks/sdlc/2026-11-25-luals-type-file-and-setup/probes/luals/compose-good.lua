local s, n = airsstack.t.f({ "a", 1 }, function(x) return x == "a" end, 1, "p", "q")
local s2 = airsstack.t.f({}, nil)
local caps = airsstack.t.g(function(...) return "x" end, { HOME = true })
print(s:upper(), n + 1, s2, caps)
