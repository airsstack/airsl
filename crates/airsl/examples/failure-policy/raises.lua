-- A script that fails the ordinary way: it raises.
--
-- Stands in for the realistic case — a hook that hit a condition it cannot handle and said so.
-- Nothing here is a resource breach, which is the whole point: the host has to be able to tell
-- this apart from a script that ate the machine.

local reason = arg[1] or "no reason given"

error("enforce: " .. reason)
