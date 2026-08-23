-- The directory module, reached as `require("lib")`.
--
-- There is no `lib.lua`, so the loader falls through to this file. That second candidate is what
-- lets a component keep its parts in a subdirectory and still be required by the name of the
-- directory, instead of forcing every project into a flat file per module.

BODIES_RUN = BODIES_RUN or {}
BODIES_RUN[#BODIES_RUN + 1] = "lib"

-- A dotted target is resolved against the script root, not against the directory this file sits
-- in. There is no relative `require` here: a name means the same file wherever it is written, so
-- moving a module changes the name once rather than in every file that reaches it.
local text = require("lib.text")

return {
  version = "1.0.0",
  text = text,
}
