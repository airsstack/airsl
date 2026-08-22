-- A script that reaches a module this crate does not ship, through a root table of the host's
-- choosing.
--
-- Nothing here names `airsstack`. The host built this engine with `RootTable::new("myapp")`, so
-- that is the only global the script sees — and the contributed module sits in it alongside the
-- built-ins, indistinguishable from them at the call site.

local recorded = {}

for _, name in ipairs({ "requests", "cache.hits", "cache.misses" }) do
  recorded[#recorded + 1] = myapp.metrics.record(name)
end

-- A built-in, reached through the same custom root. `json` was never told the table was renamed;
-- the engine installs every module into whichever root it was built with.
print(myapp.json.encode(recorded))

-- The grant check lives in Rust, inside the host function, before the operation. A script cannot
-- reach around it — `pcall` observes the refusal, it does not bypass it.
local ok, refusal = pcall(myapp.metrics.reset)
print("reset allowed: " .. tostring(ok))

-- Trimmed to the first line. The traceback that follows names chunk positions that move whenever
-- this file is edited, and the refusal itself is the part worth reading.
refusal = tostring(refusal)
print("refusal: " .. (string.match(refusal, "^[^\n]*") or refusal))
