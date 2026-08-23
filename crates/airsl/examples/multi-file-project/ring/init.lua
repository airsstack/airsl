-- One half of a cycle: `ring` requires `ring.link`, which requires `ring` again.
--
-- Neither file can tell that it is the one closing the loop, which is why the detection lives in
-- the loader rather than in a convention these two could follow. Requiring this raises instead of
-- recursing; without the in-progress marker the recursion would end in a stack overflow, and that
-- aborts the process rather than raising something a script can catch.
--
-- Deliberately absent from `BODIES_RUN`: a failed `require` clears its cache entry, so both halves
-- of this cycle run their bodies again on every evaluation, and counting them there would make the
-- list stop measuring what it exists to measure.

local link = require("ring.link")

return { link = link }
