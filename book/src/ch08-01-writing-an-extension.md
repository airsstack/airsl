# Writing an Extension

An extension is a folder with two things in it: a manifest called `extension.toml`, and a Lua entry
script.

```
word-count/
├── extension.toml
└── main.lua
```

## The entry script

```lua
-- word-count/main.lua
airsstack.ext.on("count", function(payload)
  local words = airsstack.regex.find_all([[\w+]], payload.text)

  local longest = ""
  for _, word in ipairs(words) do
    if #word > #longest then
      longest = word
    end
  end

  return { words = #words, longest = longest }
end)
```

The entry script runs once, when the extension loads. It does not do the work itself. It calls
`ext.on` to say "when the `count` event happens, call this function". Each time the host fires
`count`, the function receives the event's payload as a table, and whatever it returns goes back to
the host as JSON.

## The manifest

```toml
# word-count/extension.toml
[extension]
name    = "word-count"
version = "0.1.0"
entry   = "main.lua"
api     = 1

[capabilities]
regex = true

[limits]
memory       = "8MB"
instructions = 1_000_000
```

| Section | Says |
|---|---|
| `[extension]` | who the extension is, which file to run, and which version of the extension API it was written for |
| `[capabilities]` | what it needs; this one needs only `regex`, which reaches nothing |
| `[limits]` | how much it expects to spend; a host will never give it *more* than its own limits |

## Asking for more

An extension that needs files, variables or programs asks for them the same way:

```toml
[capabilities]
fs.read  = ["$NOTES_HOME"]
fs.write = ["$NOTES_HOME/.index"]
env.read = ["NOTES_HOME"]
proc.run = ["git"]

[capabilities.optional]
proc.run = ["tar"]
```

Three rules apply to everything in `[capabilities]`:

**A request is a maximum, never a right.** The host compares what the extension asks for with what
it is willing to allow. Asking for `fs.read = ["/"]` is not an error, it just will not be granted.

**Required means required.** If anything under `[capabilities]` cannot be granted, the extension does
not load at all. It never starts in a half-working state, quietly failing at whatever it could not
do. Anything it can genuinely live without goes under `[capabilities.optional]`.

**Variables are filled in by the host.** `$NOTES_HOME` is replaced with a value the host supplies,
never one the extension supplies. Otherwise an extension could point the variable at `/` and widen
its own grant.

An unknown key is an error, not something to skip. A misspelled `fs.raed` that was silently ignored
would be a permission the author believed they had asked for.

## Checking what was granted

Inside the extension, `ext.granted()` returns what the host actually allowed. Use it to check for an
optional capability:

```lua
local granted = airsstack.ext.granted()
if granted.proc and granted.proc.run["tar"] then
  -- tar was allowed, so archives can be made
end
```
