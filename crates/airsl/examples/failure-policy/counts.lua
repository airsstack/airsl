-- An ordinary, terminating script. Nothing is wrong with it.
--
-- It appears in an example about failure because of what the host does to it, not because of what
-- it does: given a large enough instruction budget it finishes and returns; given a tiny one it is
-- stopped part-way. That makes it safe to run directly — unlike a genuine runaway, which is why
-- the `resource-limits` example keeps its malicious chunks inline rather than in a file like this.

local total = 0

for i = 1, 200000 do
  total = total + i
end

return total
