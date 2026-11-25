local out = airsstack.proc.run({ "echo", "hi" })
print(out.stdout:upper(), out.stderr:upper(), out.status + 1)
local caps = airsstack.regex.captures("(a)(?P<n>b)", "ab")
if caps then print(caps[0]:upper(), caps[1]:upper(), caps.n:upper()) end
local re = airsstack.regex.compile("(a)")
local c = re.captures("a")
if c then print(c[1]:upper()) end
print(re.is_match("a") == true, re.find_all("a")[1]:upper(), re.replace("a", "b"):upper(), re.replace_all("a", "b"):upper(), re.split("a")[1])
local f = re.find("a")
if f then print(f:upper()) end
local v = airsstack.json.decode('{"a":1}')
print(v.a, v[1], airsstack.json.decode("1") + 1, airsstack.json.decode('"s"'):upper())
local p = airsstack.hook.payload()
print(p.tool_name, p.tool_input.command)
local g = airsstack.ext.granted()
print(g.fs.read[1]:upper(), g.fs.write[1], g.env.HOME == true, g.proc.run.echo == true, g.unrestricted == false)
if g.limits.memory then print(g.limits.memory + 1, g.limits.instructions) end
