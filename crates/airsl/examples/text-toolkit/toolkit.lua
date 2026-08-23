-- Real text work under an empty grant set: parsing, matching, hashing, formatting, globbing.
--
-- Nothing below is *permitted* by a grant, because there is nothing here to permit. `regex`,
-- `path`, `hash`, `time` and `glob.match` compute over the arguments they are handed and reach
-- nothing outside them, which is why they are installed whole even under the pure preset.
--
-- Two functions that need no grant are still absent from this file on purpose. `time.now` and
-- `path.absolute` need no authority, but they read the wall clock and the process working
-- directory — so their results differ between two runs on one machine, and this example's output
-- has to be checkable against its own README.

--- The corpus, written into the script rather than read from a file: reading one would need the
--- read grant this example exists to do without.
local LOG = table.concat({
  "2023-11-14T22:13:20Z INFO  engine builds the state in 4 ms",
  "2023-11-14T22:13:21Z WARN  fs denied: /etc/shadow is outside the granted read roots",
  "2023-11-14T22:13:22Z ERROR proc denied: curl is not on the executable allowlist",
  "2023-11-14T22:13:23Z INFO  engine evaluates crates/airsl/examples/text-toolkit/toolkit.lua",
  "2023-11-14T22:13:24Z WARN  hash denied: /var/db/keys is outside the granted read roots",
}, "\n")

--- A path this machine does not have and this script never opens. `path` is string arithmetic, so
--- whether it exists is not a question any of the calls below can ask.
local TARGET = airsstack.path.join("crates", "airsl", "examples", "text-toolkit", "toolkit.lua")

-- --- regex: one compiled pattern, applied five times ---------------------------------------

print("regex")

-- `split` on a real expression is what replaces the `string.gmatch("[^\n]+")` dance, and unlike
-- that dance it does not silently drop an empty field.
local lines = airsstack.regex.split([[\n]], LOG)

-- Compiled once, above the loop. The one-shot forms recompile the pattern on every call, which a
-- script scanning a file pays for once per line; `compile` hands back the same operations bound to
-- one pattern, so the cost is paid here and not five times below.
--
-- The pattern also shows why this module exists at all: `INFO|WARN|ERROR` is alternation and
-- `(?<name>...)` is a named group, and a Lua pattern has neither.
local entry = airsstack.regex.compile(
  [[^(?<stamp>\S+) (?<level>INFO|WARN|ERROR)\s+(?<module>\w+) (?<message>.*)$]]
)

local counts, stamps, named_and_numbered_agree = {}, {}, true
for _, line in ipairs(lines) do
  assert(entry.is_match(line), "the corpus and the pattern disagree: " .. line)

  local caught = entry.captures(line)
  -- Groups arrive under their number *and* their name, so a pattern can be read either way
  -- without the caller having to know which style it was written in.
  named_and_numbered_agree = named_and_numbered_agree and caught[3] == caught.module

  counts[caught.level] = (counts[caught.level] or 0) + 1
  stamps[#stamps + 1] = caught.stamp
end

print("  split gave " .. #lines .. " lines, all matched by one compiled pattern")

-- `pairs` walks a table in Lua's hash order, which differs between runs. Sorting the keys is what
-- makes this line printable at all — the same reason the host sorts JSON keys and directory
-- listings rather than offering it as an option.
local levels = {}
for level in pairs(counts) do
  levels[#levels + 1] = level
end
table.sort(levels)
for index, level in ipairs(levels) do
  levels[index] = level .. "=" .. counts[level]
end
print("  levels: " .. table.concat(levels, " "))
print("  group 3 and the name `module` are the same span: " .. tostring(named_and_numbered_agree))

local found = airsstack.regex.find_all([[\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z]], LOG)
print("  find_all timestamps: " .. #found .. ", first " .. found[1])

-- A replacement can refer to a capture, so the part of the match to keep does not have to be
-- rebuilt by hand. `${1}` rather than `$1`, because `$1<path>` would read as a group named `1`
-- followed by nothing this pattern defines.
print("  replace_all: " .. airsstack.regex.replace_all([[(denied: )/\S+]], lines[2], "${1}<path>"))

-- --- path: separators, and no filesystem at all ---------------------------------------------

print("")
print("path")
print("  join:        " .. TARGET)
print("  dirname:     " .. airsstack.path.dirname(TARGET))
print("  basename:    " .. airsstack.path.basename(TARGET))
print("  stem / ext:  " .. airsstack.path.stem(TARGET) .. " / " .. airsstack.path.ext(TARGET))

-- Lexical: `..` cancels the component before it textually, without asking whether either exists.
-- That also means normalising is not a containment check — confinement is decided against a
-- canonical path, inside `fs`.
local dotted = "crates/airsl/../airsl/./examples//toolkit.lua"
print("  normalize:   " .. airsstack.path.normalize(dotted))
print("  relative_to: " .. airsstack.path.relative_to(TARGET, "crates/airsl"))

-- Refuses rather than answering `../../elsewhere`: the caller asked where this is *under* that,
-- and a path that is not under it has no answer to that question.
local outside = pcall(airsstack.path.relative_to, TARGET, "crates/other")
print("  relative_to refuses a path outside its base: " .. tostring(not outside))
print(
  "  is_absolute: "
    .. tostring(airsstack.path.is_absolute(TARGET))
    .. " here, "
    .. tostring(airsstack.path.is_absolute("/etc/hosts"))
    .. " for /etc/hosts"
)

-- --- hash: content addressing over strings ---------------------------------------------------

print("")
print("hash")

local body = "the grant is checked inside the host function, before the operation.\n"
print("  sha256: " .. airsstack.hash.sha256(body))
print("  sha1:   " .. airsstack.hash.sha1(body))

-- The Lua spelling of `shasum | cut -c1-8`: a short key naming an artifact, where the hash was
-- never chosen for its strength. SHA-1 ships for that case and for no other.
print("  cache key from sha1(path): " .. string.sub(airsstack.hash.sha1(TARGET), 1, 8))
print("  hex: " .. airsstack.hash.hex("airsl"))

-- --- time: an instant the host supplies, never one the clock does ---------------------------

print("")
print("time")

-- The instant comes out of the corpus, which is why this line is the same on every run. `format`
-- takes an explicit instant for exactly this reason — there is no "now" default to fall into.
local seconds = airsstack.time.parse(stamps[1])
print("  parse " .. stamps[1] .. " -> " .. seconds)
print("  format:          " .. airsstack.time.format(seconds))
print("  format %Y-%m-%d: " .. airsstack.time.format(seconds, "%Y-%m-%d"))
local round_tripped = airsstack.time.parse(airsstack.time.format(seconds))
print("  parse(format(t)) == t: " .. tostring(round_tripped == seconds))

-- `monotonic` is a reading, not a fact: printing one would put a different number in the output on
-- every run. The property is what is printable, and it is the property a script measuring a
-- duration actually depends on.
local before = airsstack.time.monotonic()
local sum = 0
for i = 1, 10000 do
  sum = sum + i
end
assert(sum == 50005000, "the loop between the two readings did not run")
local after = airsstack.time.monotonic()
print("  monotonic is non-decreasing across two reads: " .. tostring(after >= before))

-- --- glob: matching a path that is only a string ---------------------------------------------

print("")
print("glob")

for _, pattern in ipairs({ "*.lua", "**/*.lua", "crates/**/text-toolkit/*.lua", "**/*.rs" }) do
  print(string.format("  %-30s %s", pattern, tostring(airsstack.glob.match(pattern, TARGET))))
end

-- `*` stops at a separator, so the first pattern above is false for the nested path and true here.
-- The looser reading would let a rule declaring `*.lua` apply to files its author never named.
local bare = airsstack.glob.match("*.lua", "toolkit.lua")
print(string.format("  %-30s %s", "*.lua, against toolkit.lua", tostring(bare)))

-- `glob.walk` is the other half of this module and is deliberately not called: it reads
-- directories, goes through the same guard `fs` does, and would need a read grant.
