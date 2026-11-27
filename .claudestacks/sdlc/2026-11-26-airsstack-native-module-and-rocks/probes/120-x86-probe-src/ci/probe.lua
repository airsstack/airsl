-- X probe: load the toy module from the directory given as arg[1] (file airsstack.so).
local dir = arg[1]
print("host: " .. _VERSION .. (jit and (" / " .. jit.version) or ""))
package.cpath = dir .. "/?.so;" .. package.cpath
local ok, m = pcall(require, "airsstack")
if ok then
  print("require(\"airsstack\").ok = " .. tostring(m.ok) .. "  json = " .. tostring(m.json))
else
  print("require failed: " .. tostring(m))
  os.exit(1)
end
