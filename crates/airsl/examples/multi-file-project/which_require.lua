-- Which `require` this surface handed the script, said without naming anything machine-specific.
--
-- The host runs this once per row: three presets, and `pure` a second time with `require` granted.
-- `package.path` differs with the installation and with the working directory, so this reports
-- whether the table exists rather than what it holds.
--
-- Printed from here rather than from the host for the reason `app.lua` gives: the whole block is
-- put through this script's own redaction, and a line printed from Rust would skip it.

-- Both arguments are optional, so `airsl run which_require.lua` works on its own and reports
-- whichever surface that invocation chose.
local label, heading = arg[1] or "-", arg[2] or ""

--- Whether calling `require` is refused for want of the `require` host access, as opposed to
--- failing for any reason of its own. The refusal is raised before the name is looked at, so any
--- target shows it.
local function refused_outright()
  local ok, err = pcall(require, "../secrets")
  return not ok and string.find(tostring(err), "require denied:", 1, true) ~= nil
end

--- Which of the four possible globals this surface provided: no global at all, one that refuses
--- every call, the confined loader, or Lua's own.
local function flavour()
  if require == nil then
    return "absent"
  elseif package ~= nil then
    -- Lua's own loader is the only one that comes with a `package` table. Testing for it first
    -- keeps the probe call below from running a real `package.path` search on that row.
    return "native"
  elseif refused_outright() then
    return "refused"
  end
  -- No `package` means no `package.path` and no `package.searchers`. The confined loader has no
  -- configuration surface at all, so a script cannot widen where it is allowed to look.
  return "confined"
end

--- How this surface answers a target that names a parent directory.
---
--- The three outcomes are not degrees of the same refusal. One refuses the call itself, whatever
--- the name; one rejects the name, so no path is ever built; the other builds a path and goes
--- looking.
local function parent_target()
  if require == nil then
    return "there is no require to call"
  end
  local ok, err = pcall(require, "../secrets")
  if ok then
    return "loaded it"
  elseif string.find(tostring(err), "require denied:", 1, true) then
    return "refused, require is not granted"
  elseif string.find(tostring(err), "invalid require target", 1, true) then
    return "refused, the name is unrepresentable"
  end
  return "searched for it as a path"
end

local configurable = package ~= nil

-- `print` is not in every surface this runs under, so each line goes through `stdio`, which the
-- host grants to every row. It adds no newline of its own.
local function say(line)
  airsstack.stdio.write(line .. "\n")
end

if heading ~= "" then
  say(heading)
end

say(string.format(
  '%-12s require=%-8s package=%-3s searchers=%d  require("../secrets"): %s',
  label,
  flavour(),
  configurable and "yes" or "no",
  configurable and #package.searchers or 0,
  parent_target()
))
