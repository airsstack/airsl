# Nothing Is Allowed by Default

Under the default preset, a script starts with no permission to touch files, environment variables
or programs. This page explains the three rules that follow from that. It then introduces a second,
smaller kind of permission, called host access.

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

## Host access

A folder, a variable and a program are permissions you name. Some things a script does have nothing
to name: writing its output, reading the clock, resolving a relative path. These are called **host
access**, and there are seven members:

| Member | What it lets a script do |
|---|---|
| `stdin` | read standard input |
| `stdout` | write standard output |
| `stderr` | write standard error |
| `clock` | read the clock |
| `cwd` | use a relative path, which resolves against the working folder |
| `require` | load other files from the script's own folder with `require` |
| `zones` | parse a time that names a time zone, such as `America/New_York` |

[Chapter 5](ch05-00-the-toolbox.md) says which member each function needs.

`trusted` and `confined` hold all seven, so nothing in this book so far has been refused for want of
one. `pure` holds none. A `pure` script that calls `airsstack.time.now` is refused, and the message
names the member it lacks. `--allow-access` gives members back, one at a time. Repeat the flag for
more than one. Here is a script that tries the clock and writes what happened:

```lua
-- clock.lua
local ok, err = pcall(airsstack.time.now)
local out = airsstack.stdio
out.write((ok and "the clock works" or tostring(err):match("^[^\n]*")) .. "\n")
```

```console
$ airsl run --policy pure clock.lua
airsl: lua error in clock.lua: stdio.write denied: `stdout` is not granted — no access is granted
stack traceback:
	[C]: in field 'write'
	clock.lua:4: in main chunk

$ airsl run --policy pure --allow-access stdout clock.lua
time.now denied: `clock` is not granted — the granted access is stdout
$ airsl run --policy pure --allow-access stdout --allow-access clock clock.lua
the clock works
```

The first run is refused before it can say anything, because the script cannot write without
`stdout`. The message ends with what the script does hold, which here is nothing.

`airsl doctor` shows the members a policy holds on its `grants:` line. See
[Grants on the command line](ch01-03-airsl-doctor.md#grants-on-the-command-line).

`--allow-access` changes nothing under `confined`, which already holds every member, and it is
ignored under `trusted`, like every other grant flag.

Host access does not remove parts of Lua itself; those belong to the preset. See
[Host access does not reach Lua's own functions](ch06-03-granting.md#host-access-does-not-reach-luas-own-functions).
