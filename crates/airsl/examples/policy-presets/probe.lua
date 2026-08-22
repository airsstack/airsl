-- Reports which globals this script can actually see, and returns the answer as one line.
--
-- The host runs this same file under all three presets. That it is the same file is the point:
-- nothing here asks what policy it is running under, so every difference in the output is the
-- sandbox and not the script.

-- Sorted, so the row reads the same on every run and between presets.
local NAMES = {
  "airsstack",
  "coroutine",
  "io",
  "load",
  "os",
  "package",
  "require",
  "string",
  "utf8",
}

local function mark(present)
  return present and "yes" or "no"
end

local seen = {}
for _, name in ipairs(NAMES) do
  seen[#seen + 1] = name .. "=" .. mark(_G[name] ~= nil)
end

-- `os` is not a single yes/no. Below `full` the table survives with the functions that reach
-- outside the process removed from it, so probing only the table would report `os` as intact while
-- `os.getenv` had in fact been taken away.
local has_os = type(os) == "table"

return table.concat(seen, " ")
  .. " | os.time="
  .. mark(has_os and os.time ~= nil)
  .. " os.getenv="
  .. mark(has_os and os.getenv ~= nil)
