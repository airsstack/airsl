# Nothing Is Allowed by Default

Under the default preset, a script starts with no grants. This page explains the three rules that
follow from that.

## The tools are always there

Every module is present, even when the script has no permission to use it. You can always write
`airsstack.fs.read(...)`. Whether the call succeeds is a separate question, answered when it runs.

This is on purpose. If `airsstack.fs` disappeared when a script had no file grant, then
`if airsstack.fs then` would quietly change its meaning, from "does this runtime have files?" to
"am I allowed?", and a script that checked it would silently skip work instead of telling you why.

## A refusal is an error, never an answer

When a script asks about something it was not granted, it gets an error. It never gets a value that
could be mistaken for a fact.

Consider a script checking whether a config file exists:

```lua
if airsstack.fs.exists("/etc/myapp.conf") then
  -- ...
end
```

If `exists` returned `false` for a folder the script was not allowed to look in, the script would
report "no config file" when the truth was "not allowed to look". Those are very different problems
to debug. So `exists` raises an error instead, and so does every other function that asks about
something outside the script's grants.

## Refusals can be caught

A refusal is an ordinary Lua error, so `pcall` catches it like any other. `pcall` calls a function
and returns `true` plus its result, or `false` plus the error:

```lua
-- check.lua
local ok, err = pcall(airsstack.fs.read, "/etc/hosts")
if not ok then
  local message = tostring(err):match("^[^\n]*")
  print("could not read: " .. message)
end
```

```console
$ airsl run check.lua
could not read: fs.read denied: `/etc/hosts` is outside the granted read roots: none are granted
```

The error is an error object rather than a plain string. `tostring` turns it into text, which
includes a stack traceback after the first line, and `match("^[^\n]*")` keeps only that first line.

A script can use this to fall back gracefully. But usually it is better to let the error stop the
script, so that whoever runs it sees exactly what was missing.
