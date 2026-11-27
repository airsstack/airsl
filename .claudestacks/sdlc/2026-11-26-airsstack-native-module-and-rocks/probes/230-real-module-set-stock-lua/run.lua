-- probe 230: load the real eleven-module airsstack build into a stock interpreter.
-- usage: <lua> run.lua <dir holding airsstack.so> <empty temp dir>
local libdir, tmp = arg[1], arg[2]
package.cpath = libdir .. "/?.so;" .. package.cpath
print("interpreter:", _VERSION, rawget(_G, "jit") and jit.version or "no jit")
local ok, a = pcall(require, "airsstack")
print("require ok:", ok, type(a))
if not ok then print(a) os.exit(1) end
local keys = {}
for k, v in pairs(a) do keys[#keys + 1] = k .. ":" .. type(v) end
table.sort(keys)
print("keys (" .. #keys .. "):", table.concat(keys, " "))
print("ext:", a.ext)
print("global airsstack:", rawget(_G, "airsstack"))
print("package.loaded is the table:", rawequal(package.loaded.airsstack, a))
local function try(label, f)
  local function pack(...) return {n = select("#", ...), ...} end  -- table.pack is 5.2+
  local r = pack(pcall(f))
  local out = {}
  for i = 2, r.n do out[#out + 1] = tostring(r[i]) end
  print(string.format("%-8s %s  %s", label, r[1] and "ok " or "ERR", table.concat(out, " | ")))
end
local f = tmp .. "/probe.txt"
try("json", function() return a.json.encode({2, 2.0}), a.json.decode('{"n":1}').n end)
try("path", function() return a.path.join("a", "b"), a.path.basename(f) end)
try("fs", function() a.fs.write(f, "hello from fs") return a.fs.read(f), a.fs.read(arg[0]):sub(1, 10) end)
try("env", function() a.env.set("AIRSL_PROBE_230", "set-by-env") return a.env.get("AIRSL_PROBE_230"), type(a.env.get("HOME")), os.getenv("AIRSL_PROBE_230") end)
try("proc", function() local r = a.proc.run({"echo", "hi from proc"}) return r.status, (r.stdout:gsub("\n", "\\n")), r.stderr end)
try("regex", function() return a.regex.is_match("^h", "hello"), table.concat(a.regex.find_all("[a-z]+", "ab 12 cd"), ",") end)
try("hash", function() return a.hash.sha256("abc"), a.hash.hex("A") end)
try("time", function() local n = a.time.now() return math.type and math.type(n) or type(n), n > 1700000000, a.time.format(0) end)
try("glob", function() return a.glob.match("*.txt", "probe.txt"), table.concat(a.glob.walk(tmp, "*.txt"), ",") end)
try("stdio", function() a.stdio.write("[stdio.write reached stdout]\n") return a.stdio.isatty() end)
try("hook", function() a.hook.emit({emitted = true}) io.stdout:write("\n") return a.hook.context("PreToolUse", "probe") end)
