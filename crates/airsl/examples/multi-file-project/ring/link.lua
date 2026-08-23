-- The other half of the cycle. Nothing here is unusual, and that is the point: this file asks for
-- a module by name the way any file would, and the name happens to be the one still loading.

local ring = require("ring")

return { ring = ring }
