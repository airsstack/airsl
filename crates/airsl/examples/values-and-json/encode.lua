-- Builds one record and encodes it three ways, so the host can compare the bytes.
--
-- The arguments arrive in Lua's own `arg` table, which is why a shell script ported to airsl
-- reads `arg[1]` where it used to read `$1`.

local record = {
  name = arg[1],
  count = tonumber(arg[2]),

  -- Lua 5.4 distinguishes integers from floats, and airsl runs 5.4 for exactly this reason: a
  -- timestamp that silently became a float would encode as `1.7e9` and stop being a timestamp.
  observed_at = 1700000000,

  -- A sequence, so it encodes as a JSON array. Element order is the script's, not sorted --
  -- sorting applies to object keys, where Lua never had an order to preserve in the first place.
  tags = { "beta", "alpha" },

  -- Lua has no distinct empty-sequence value, so an empty table encodes as an empty object.
  meta = {},
}

local compact = airsstack.json.encode(record)
local again = airsstack.json.encode(record)
local pretty = airsstack.json.encode_pretty(record)

-- Decoding returns an ordinary Lua table, so the round trip is checkable from Lua as well as from
-- the host.
local decoded = airsstack.json.decode(compact)

-- Returning a table is how a script hands the host structured output. The host reads the fields it
-- knows about; anything else is ignored rather than mis-parsed.
return {
  compact = compact,
  again = again,
  pretty = pretty,
  decoded_name = decoded.name,
  decoded_observed_at = decoded.observed_at,
  decoded_tag_count = #decoded.tags,
}
