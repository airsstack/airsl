-- A leaf module, reached as `require("lib.text")`.
--
-- Records that its body ran. That is the whole instrumentation of this example: `lib/init.lua`
-- requires this file and so does `app.lua`, and only the first of those two calls runs it. A
-- module that re-ran would append a second entry, so the list below is a detector and not a label.
--
-- The list is a global because it has to outlive the chunk that writes it, and globals a script
-- wrote survive on a reused engine exactly as the module cache does. Nothing else here needs one.

BODIES_RUN = BODIES_RUN or {}
BODIES_RUN[#BODIES_RUN + 1] = "lib.text"

local M = {}

--- A lowercase, hyphen-separated form of `s`.
---
--- `string.gsub` answers with a replacement count as well as the text, so the call is parenthesised
--- to return the one value a caller of `slug` expects.
function M.slug(s)
  return (string.gsub(string.lower(s), "%s+", "-"))
end

return M
