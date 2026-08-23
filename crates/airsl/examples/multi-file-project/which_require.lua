-- Which `require` this surface handed the script, said without naming anything machine-specific.
--
-- The host runs this once per preset. `package.path` differs with the installation and with the
-- working directory, so this reports whether the table exists rather than what it holds.
--
-- Printed from here rather than from the host for the reason `app.lua` gives: the whole block is
-- put through this script's own redaction, and a line printed from Rust would skip it.

-- Both arguments are optional, so `airsl run which_require.lua` works on its own and reports
-- whichever surface that invocation chose.
local label, heading = arg[1] or "-", arg[2] or ""

--- Which of the three possible globals this surface provided.
local function flavour()
  if require == nil then
    return "absent"
  elseif package == nil then
    -- No `package` means no `package.path` and no `package.searchers`. The confined loader has no
    -- configuration surface at all, so a script cannot widen where it is allowed to look.
    return "confined"
  end
  return "native"
end

--- How this surface answers a target that names a parent directory.
---
--- The two outcomes are not degrees of the same refusal. One rejects the name, so no path is ever
--- built; the other builds one and goes looking.
local function parent_target()
  if require == nil then
    return "there is no require to call"
  end
  local ok, err = pcall(require, "../secrets")
  if ok then
    return "loaded it"
  elseif string.find(tostring(err), "invalid require target", 1, true) then
    return "refused, the name is unrepresentable"
  end
  return "searched for it as a path"
end

local configurable = package ~= nil

if heading ~= "" then
  print(heading)
end

print(string.format(
  '%-8s require=%-8s package=%-3s searchers=%d  require("../secrets"): %s',
  label,
  flavour(),
  configurable and "yes" or "no",
  configurable and #package.searchers or 0,
  parent_target()
))
