local out = airsstack.proc.run({ "echo" })
print(out.nope)
print(airsstack.ext.granted().fs.nope)
print(airsstack.ext.granted().limits.nope)
print(airsstack.ext.granted().proc.nope)
local s = airsstack.regex.compile("a").nope
local n = airsstack.proc.run({ "x" }).status:upper()
