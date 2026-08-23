-- Reads through one root, writes through another, and asks for three things it was not granted.
--
-- The host passes the three directories as arguments so this script can rewrite them out of the
-- messages it prints: a temporary directory has a different name on every run, and an example
-- whose output cannot be pasted into its own README is an example nobody can check.

local source, output, outside = arg[1], arg[2], arg[3]

--- Replaces every literal occurrence of `needle` — `string.find` in plain mode, because a path
--- contains `-` and `.`, which Lua patterns would read as syntax.
local function replace_literal(text, needle, with)
  local parts, from = {}, 1
  while true do
    local start, stop = string.find(text, needle, from, true)
    if not start then
      break
    end
    parts[#parts + 1] = string.sub(text, from, start - 1)
    parts[#parts + 1] = with
    from = stop + 1
  end
  parts[#parts + 1] = string.sub(text, from)
  return table.concat(parts)
end

--- A message with the machine-specific directories replaced by their roles, and the Lua traceback
--- that follows the first line dropped — it names absolute paths and line numbers, neither of
--- which belongs in output meant to be diffed.
local function stable(message)
  message = string.match(message, "^[^\n]*") or message
  message = replace_literal(message, source, "<source>")
  message = replace_literal(message, output, "<output>")
  return replace_literal(message, outside, "<outside>")
end

--- Runs `body`, printing the refusal instead of propagating it.
local function expect_refusal(label, body)
  local ok, err = pcall(body)
  assert(not ok, label .. " was permitted, and should not have been")
  print(label .. ": " .. stable(tostring(err)))
end

-- --- reading, through the read root -------------------------------------------------------

local notes = airsstack.path.join(source, "notes.txt")
local body = airsstack.fs.read(notes)
print("read notes.txt: " .. #body .. " bytes")

-- `modified` is deliberately not printed: it is the one field of `stat` that changes per run.
local meta = airsstack.fs.stat(notes)
print("stat notes.txt: kind=" .. meta.kind .. " size=" .. meta.size)

-- --- writing, through the write root ------------------------------------------------------

airsstack.fs.write(airsstack.path.join(output, "notes.txt"), body)
print("copied to <output>/notes.txt")

-- The staging file is created in the target's own directory and renamed over it, so a concurrent
-- reader sees the old bytes or the new ones and never half of each.
airsstack.fs.atomic_write(
  airsstack.path.join(output, "index.json"),
  airsstack.json.encode_pretty({ source = "notes.txt", bytes = #body })
)
print("atomic_write wrote <output>/index.json")

-- Losing this race is the expected *other outcome*, not a failure, so it answers `false` rather
-- than raising. A read-then-write would let several concurrent callers all believe they won.
local claim = airsstack.path.join(output, "lock")
print("create_exclusive, first call:  " .. tostring(airsstack.fs.create_exclusive(claim, "held")))
print("create_exclusive, second call: " .. tostring(airsstack.fs.create_exclusive(claim, "held")))

-- Sorted by the host, so this line does not depend on the filesystem's enumeration order.
print("<source> holds: " .. table.concat(airsstack.fs.list(source), ", "))

-- --- the four refusals ---------------------------------------------------------------------

-- A read root is not a write root. This is the same directory the script has just read from.
expect_refusal("writing into the read root", function()
  airsstack.fs.write(notes, "overwritten")
end)

-- A write root is not implicitly readable either. The script wrote this file moments ago.
expect_refusal("reading back from the write root", function()
  airsstack.fs.read(airsstack.path.join(output, "notes.txt"))
end)

-- Interrogation counts as reading. `list`, `stat` and `exists` all refuse an ungranted path
-- rather than answering `false` for it, because answering would conflate "you may not ask" with
-- "there is nothing there".
expect_refusal("listing the write root", function()
  airsstack.fs.list(output)
end)

-- Outside both roots. The message names the roots that *were* granted, which is what turns
-- "denied" into something actionable.
expect_refusal("reading outside every root", function()
  airsstack.fs.read(airsstack.path.join(outside, "secret.txt"))
end)
