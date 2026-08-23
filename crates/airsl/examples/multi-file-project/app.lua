-- The entry script of a project split across files. Everything it needs it requires, and
-- everything it can require lives under its own directory.
--
-- Every line of this example's output is written from here rather than from the host, and the
-- heading arrives as an argument for that reason. Not an ordering constraint: Lua's `print` and
-- Rust's `println!` both flush per line and interleave correctly. It is that every line below is
-- put through `stable` to redact the project root, and a heading printed from Rust would be the
-- one line in the block that had not been.
--
-- The project directory arrives the same way: a `require` refusal names the root it searched, and
-- an absolute path differs on every machine.

-- Both arguments are optional, so `airsl run app.lua` works on its own. Without them the refusals
-- below name the real directory, which is the right answer for someone running this by hand and
-- the wrong one for output that has to be diffed.
local root, heading = arg[1] or "", arg[2] or ""

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

--- The first line of `message`, with the project directory replaced by its role. The traceback
--- after that line names absolute paths, which belong nowhere near output meant to be diffed.
local function stable(message)
  message = string.match(message, "^[^\n]*") or message
  if root == "" then
    return message
  end
  return replace_literal(message, root, "<project>")
end

--- Requires `name`, printing the refusal instead of propagating it.
local function expect_refusal(name)
  local ok, err = pcall(require, name)
  assert(not ok, 'require("' .. name .. '") was permitted, and should not have been')
  print('require("' .. name .. '"): ' .. stable(tostring(err)))
end

if heading ~= "" then
  print(heading)
end

-- Counts this script's own evaluations, so the two facts can be read side by side: the number goes
-- up on a reused engine while the list of module bodies below it does not.
EVALUATIONS = (EVALUATIONS or 0) + 1

-- --- the module tree --------------------------------------------------------------------------

-- A directory module: there is no `lib.lua`, so this resolves through `lib/init.lua`.
local lib = require("lib")

-- A dot separates path components, so this is `lib/text.lua`. `lib/init.lua` has already required
-- it, and this second call gets the table that one produced rather than running the file again.
local text = require("lib.text")

print("evaluation " .. EVALUATIONS .. " on this engine")
print("lib.version: " .. lib.version)
print('lib.text.slug("Multi File Project"): ' .. text.slug("Multi File Project"))
print("lib.text is the table lib itself required: " .. tostring(lib.text == text))
print("module bodies run so far: " .. table.concat(BODIES_RUN, ", "))

-- --- what a target may not name ---------------------------------------------------------------

-- Not a path check that failed. `..` has no spelling as a target at all, so the name is refused
-- before any path is built — there is nothing for the filesystem to be asked about.
expect_refusal("../secrets")

-- A well-formed name for a file that is not there. The message names the directory searched, which
-- is the whole search: there is no second place to look.
expect_refusal("missing")

-- A chain that closes on itself, answered rather than recursed into.
expect_refusal("ring")
