-- Never reached. `fs.read = ["$OUTSIDE"]` expands to this platform's filesystem root, which is
-- outside every ceiling this example builds, so negotiation denies the extension before this file
-- is even opened for evaluation — `airsl check` still compiles it on its own, which is the only
-- thing asked of it here, and it registers nothing.
return 1
