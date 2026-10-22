# Time and Hashing

The example on this page writes a small build stamp: when the build ran, and a short fingerprint of
its input.

```lua
-- stamp.lua
local a = airsstack

local input = a.fs.read(arg[1])

a.stdio.write(a.json.encode_pretty({
  built_at = a.time.format(a.time.now()),
  input    = a.hash.sha256(input):sub(1, 12),
}))
```

Run it on the `notes.md` from [chapter 2](ch02-00-first-project.md):

```console
$ airsl run --allow-read . stamp.lua notes.md
{
  "built_at": "2026-10-22T05:47:36Z",
  "input": "709eaf88c4ad"
}
```

`encode_pretty` already ends its output with a newline, so the script writes it with `stdio.write`,
which adds nothing, instead of `print`, which would add a second one.

## `time`

| Function | Returns |
|---|---|
| `time.now()` | the current time, as whole seconds since 1970 |
| `time.format(seconds)` | that moment as text, like `2026-10-22T05:30:00Z` |
| `time.format(seconds, pattern)` | that moment in your own format, like `"%Y-%m-%d"` |
| `time.parse(text)` | text back into seconds; it also takes an optional pattern |
| `time.monotonic()` | a reading for measuring how long something took |

`format` always needs a moment to format. It never quietly uses "now". That keeps it predictable:
the same number always formats to the same text. It also always uses UTC, so the output does not
change with the time zone of the machine it runs on.

```lua
a.time.format(1700000000)              --> 2023-11-14T22:13:20Z
a.time.format(1700000000, "%Y-%m-%d")  --> 2023-11-14
```

`monotonic` is for timing, not for dates. It is not affected when the system clock is changed, but
the number on its own means nothing. Subtract one reading from another to get the seconds in
between:

```lua
local start = a.time.monotonic()
-- ... some work ...
print(("took %.3f seconds"):format(a.time.monotonic() - start))
```

There is no `sleep`. A script that could pause for as long as it liked would get around the
instruction limit from [chapter 4](ch04-01-limits.md).

## `hash`

| Function | Returns |
|---|---|
| `hash.sha256(text)` | the SHA-256 of `text`, as hex |
| `hash.sha1(text)` | the SHA-1 of `text`, as hex |
| `hash.hash_file(path)` | the SHA-256 of a file; needs a read grant |
| `hash.hex(text)` | the bytes of `text`, as hex |

Use SHA-256 for anything new. SHA-1 is there for compatibility, for when a script has to produce the
same names an older tool produced with something like `shasum | cut -c1-8`.
