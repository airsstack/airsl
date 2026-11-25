local function dump(label, v, indent)
  indent = indent or ""
  if type(v) ~= "table" then print(indent .. label .. " = " .. (math.type(v) or type(v)) .. " " .. tostring(v)); return end
  print(indent .. label .. " : table")
  local keys = {}
  for k in pairs(v) do keys[#keys + 1] = k end
  table.sort(keys, function(a, b) return tostring(a) < tostring(b) end)
  for _, k in ipairs(keys) do dump("[" .. (math.type(k) or type(k)) .. " " .. tostring(k) .. "]", v[k], indent .. "  ") end
end
local a = airsstack
dump("proc.run({'echo','hi'})", a.proc.run({ "echo", "hi" }))
dump("regex.captures", a.regex.captures("(a)(?P<n>b)(c)?", "ab"))
dump("ext.granted()", a.ext.granted())
dump("fs.stat('.')", a.fs.stat("."))
dump("json.decode('1')", a.json.decode("1"))
dump("json.decode('\"s\"')", a.json.decode('"s"'))
dump("json.decode('null')", a.json.decode("null"))
dump("hook.payload() with stdin \"x\"", a.hook.payload())
