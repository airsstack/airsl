-- Asks for six things the policy does not grant, and prints what it is told.
--
-- The point is not that the calls fail. It is that they are all *there* to fail: every module is
-- installed under every policy, so `airsstack.fs` is a table whether or not this script may read a
-- file, and the refusal explains itself rather than just saying no.

local granted = arg[1]

--- The first line of a refusal, with the granted root replaced by its role.
--- `string.find` runs in plain mode because a temporary directory's name contains `-` and `.`,
--- which Lua patterns would otherwise read as syntax.
local function stable(message)
  message = string.match(tostring(message), "^[^\n]*") or tostring(message)
  local parts, from = {}, 1
  while true do
    local start, stop = string.find(message, granted, from, true)
    if not start then
      break
    end
    parts[#parts + 1] = string.sub(message, from, start - 1)
    parts[#parts + 1] = "<granted>"
    from = stop + 1
  end
  parts[#parts + 1] = string.sub(message, from)
  return table.concat(parts)
end

--- Runs `body`, asserts it was refused, and prints the refusal.
local function refused(label, body)
  local ok, err = pcall(body)
  assert(not ok, label .. " was permitted, and should not have been")
  print(label .. "\n  " .. stable(err))
end

-- --- presence is not permission ---------------------------------------------------------------

-- Every module the runtime ships, under a policy that grants almost none of what they do. A script
-- can therefore ask "does this runtime have `hook`?" and get an answer about the runtime, not
-- about its own authority.
local installed = {
  "json", "path", "fs", "env", "proc", "regex",
  "hash", "time", "glob", "stdio", "hook", "ext",
}
for _, name in ipairs(installed) do
  assert(type(airsstack[name]) == "table", name .. " is missing from the root table")
end
print("modules installed: " .. table.concat(installed, ", "))

-- Two of them need no authority at all, and work identically under every policy.
print("path.join needs nothing: " .. airsstack.path.join("a", "b", "c.txt"))
print("json.encode needs nothing: " .. airsstack.json.encode({ ok = true }))

-- And the one narrow grant this policy does carry works, so the refusals below are about the edge
-- of the grant rather than about a broken engine.
print("granted read works: " .. #airsstack.fs.read(airsstack.path.join(granted, "visible.txt")) .. " bytes")

-- --- six refusals ------------------------------------------------------------------------------

print("")

-- `/` exists on every unix and resolves to itself, so this refusal reads the same everywhere.
refused("fs.read, outside the granted root", function()
  airsstack.fs.read("/")
end)

-- The root *is* granted — for reading. Write is a separate grant, and this policy has none at all.
refused("fs.write, into the read-only root", function()
  airsstack.fs.write(airsstack.path.join(granted, "new.txt"), "x")
end)

-- `hash.hash_file` opens a file, so it answers to the same read grant `fs.read` does. A module
-- needing authority does not get its own axis; it borrows the one that matches what it touches.
refused("hash.hash_file, outside the granted root", function()
  airsstack.hash.hash_file("/")
end)

-- `glob.walk` reads directories, so it does too. `glob.match` is pure pattern arithmetic and needs
-- nothing — the split between them is the convention that every module is a capability.
refused("glob.walk, outside the granted root", function()
  airsstack.glob.walk("/", "*")
end)

refused("env.get, a name that is not on the allowlist", function()
  airsstack.env.get("AIRSL_EXAMPLE_SECRET")
end)

refused("proc.run, an executable that is not on the allowlist", function()
  airsstack.proc.run({ "curl", "https://example.com" })
end)

print("")
print("every refusal named the grant it was measured against")
