-- Text work: `regex` for what a Lua pattern cannot say, and `hash` for digests that match the
-- command-line tools.

local airsstack = require("airsstack")
local regex, hash = airsstack.regex, airsstack.hash

local log = table.concat({
  "2023-11-14T22:13:20Z INFO  engine builds the state in 4 ms",
  "2023-11-14T22:13:21Z WARN  fs denied: /etc/shadow is outside the granted read roots",
  "2023-11-14T22:13:22Z ERROR proc denied: curl is not on the executable allowlist",
}, "\n")

-- `split` takes an expression, so an empty field is kept rather than dropped.
local lines = regex.split([[\n]], log)
print("lines: " .. #lines)

-- Compiled once, then reused. `INFO|WARN|ERROR` is alternation and `(?<level>...)` a named
-- group, and a Lua pattern has neither. Groups arrive under their number and their name.
local entry = regex.compile([[^(?<stamp>\S+) (?<level>INFO|WARN|ERROR)\s+(?<module>\w+) (?<message>.*)$]])
for _, line in ipairs(lines) do
  local caught = entry.captures(line)
  print(string.format("%-5s %-6s %s", caught.level, caught.module, caught.stamp))
end

print("timestamps found: " .. #regex.find_all([[\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z]], log))
print(regex.replace_all([[(denied: )/\S+]], lines[2], "${1}<path>"))

-- The digests are lowercase hex, which is what `shasum` prints, so they can be compared with the
-- output of those tools.
print("sha256: " .. hash.sha256("abc"))
print("sha1:   " .. hash.sha1("abc"))
print("hex:    " .. hash.hex("airsl"))
