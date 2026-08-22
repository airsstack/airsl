-- Reads the environment through an allowlist, then hands a variable to a child process.
--
-- Values are almost never printed here, only facts about them: `PATH` differs on every machine,
-- and an example whose output cannot be pasted into its own README is an example nobody can check.

local inherited, unset, passed, ungranted = arg[1], arg[2], arg[3], arg[4]

--- The first line of a refusal, without the Lua traceback that follows it.
local function first_line(message)
  return string.match(message, "^[^\n]*") or message
end

-- --- reading through the allowlist ----------------------------------------------------------

-- Granted and set. The length stands in for the value, which is machine-specific.
local path = airsstack.env.get(inherited)
print(inherited .. ": granted, set, " .. tostring(#path > 0 and "non-empty" or "empty"))

-- Granted and *not* set. This returns nil — it does not raise.
print(unset .. ": granted, unset -> " .. tostring(airsstack.env.get(unset)))

-- Not granted. This raises. The distinction matters: a script that could not tell these two
-- apart would report a missing configuration when it had actually been denied.
local ok, err = pcall(airsstack.env.get, ungranted)
assert(not ok, "an ungranted name must raise")
print(ungranted .. ": " .. first_line(tostring(err)))

-- --- the overlay ----------------------------------------------------------------------------

airsstack.env.set(passed, "from-lua")
print(passed .. ": set in the overlay")

-- `all` returns only the granted names, and only those actually set — never everything the host
-- process inherited. Sorted here because a Lua table has no order of its own.
local names = {}
for name in pairs(airsstack.env.all()) do
  names[#names + 1] = name
end
table.sort(names)
print("env.all() sees: " .. table.concat(names, ", "))

-- --- the child ------------------------------------------------------------------------------

-- An argv array, not a string: there is no shell here, so there is no word splitting and no
-- quoting bug available to write. The child inherits the overlay, which is how it can see a
-- variable that exists nowhere in the host's own environment.
local echo = airsstack.proc.run({ "sh", "-c", "printf %s \"$" .. passed .. "\"" })
print("child reported: " .. echo.stdout .. " (status " .. echo.status .. ")")

-- A non-zero status is a *result*, not an error. The script asked what happened, so it is told;
-- raising here would deny it the chance to decide whether the failure mattered.
local failed = airsstack.proc.run({ "sh", "-c", "exit 3" })
print("deliberate failure: status " .. failed.status .. ", raised nothing")

-- `which` resolves against PATH. Only whether it resolved is printed: the path it resolves to is
-- /bin/sh on macOS and /usr/bin/sh on a usrmerge Linux.
print("which sh resolved: " .. tostring(airsstack.proc.which("sh") ~= nil))

-- Ungranted executables refuse by name, listing what was allowed.
local ran, run_err = pcall(airsstack.proc.run, { "curl", "https://example.com" })
assert(not ran, "an ungranted executable must raise")
print("curl: " .. first_line(tostring(run_err)))
