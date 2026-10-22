# Text: JSON and Regular Expressions

The example on this page reads a JSON description of a package and checks that its version looks
like `1.2.3`.

```lua
-- version.lua
local a = airsstack

local package = a.json.decode([[{"name": "demo", "version": "1.4.0-beta"}]])

local parts = a.regex.captures([[^(\d+)\.(\d+)\.(\d+)(?:-(?<pre>.+))?$]], package.version)
if not parts then
  error("not a version: " .. package.version)
end

print("major " .. parts[1] .. ", minor " .. parts[2] .. ", patch " .. parts[3])
print("pre-release: " .. (parts.pre or "none"))

print(a.json.encode({ name = package.name, major = tonumber(parts[1]) }))
```

```console
$ airsl run version.lua
major 1, minor 4, patch 0
pre-release: beta
{"major":1,"name":"demo"}
```

## JSON

`json.decode` turns JSON text into Lua tables, and `json.encode` turns them back.
`json.encode_pretty` does the same with indentation, for files people will read.

Object keys always come out **sorted**. In the output above, `major` comes before `name` even though
the script wrote them the other way round. That is on purpose. Lua tables have no order of their
own, so without sorting the same table could encode differently on every run. With sorting, the
same data always produces the same bytes, which is what you want for any file you commit, diff or
cache.

Two things in JSON have no exact match in Lua:

- **`null`** cannot be `nil`, because a key set to `nil` is the same as a missing key. So `decode`
  turns `null` into a special marker value, and `encode` turns that marker back into `null`. Watch
  out: the marker is not `nil`, so `if value.a then` is true when `a` was `null`. Compare against a
  value you decoded from `"null"` if you need to test for it.
- **An empty list** cannot be told apart from an empty object, because both are an empty Lua table.
  An empty table you build yourself encodes as `{}`. If you need `[]`, `a.json.decode("[]")` gives you
  a table that encodes as one.

Both markers can only come from decoding JSON. There is not yet a way to write them directly in Lua.

## Regular expressions

Lua has its own patterns, but they are not regular expressions. They have no `|` for alternatives,
no `\b` for word boundaries, and no named groups. `airsstack.regex` gives you real regular
expressions instead, with the same syntax as Rust's `regex` crate.

| To… | Use |
|---|---|
| test for a match | `regex.is_match(pattern, text)` |
| get the first match, or every match | `regex.find`, `regex.find_all` |
| get the groups of the first match | `regex.captures` |
| replace the first match, or every match | `regex.replace`, `regex.replace_all` |
| split text on a pattern | `regex.split` |

`captures` returns a table where `[1]`, `[2]` and so on are the numbered groups, and named groups
are also there under their names, like `parts.pre` above. If the pattern does not match, it returns
`nil`.

In a replacement, `$1` and `$name` refer to groups:

```lua
print(a.regex.replace_all([[(\w+)@(\w+)]], "bob@home, amy@work", "$2:$1"))
--> home:bob, work:amy
```

If you use the same pattern many times, compile it once. The compiled pattern has the same
functions, without the pattern argument:

```lua
local number = a.regex.compile([[\d+]])
print(#number.find_all("3 apples and 12 pears"))   --> 2
```
