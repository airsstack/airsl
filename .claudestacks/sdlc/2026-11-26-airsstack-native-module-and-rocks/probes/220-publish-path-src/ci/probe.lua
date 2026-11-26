-- Y probe: require the toy module through whatever package.cpath the caller set (luarocks path).
print("host: " .. _VERSION .. (jit and (" / " .. jit.version) or ""))
local ok, m = pcall(require, "airsstack")
if ok then
  print("require(\"airsstack\").ok = " .. tostring(m.ok) .. "  json = " .. tostring(m.json))
else
  print("require failed: " .. tostring(m))
  os.exit(1)
end
