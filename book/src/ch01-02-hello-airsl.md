# Hello, airsl

Make a new folder for the examples in this chapter, and save this as `greet.lua`:

```lua
-- greet.lua
local who = arg[1] or "world"
print("hello, " .. who)
```

Run it:

```console
$ airsl run greet.lua
hello, world

$ airsl run greet.lua airsl
hello, airsl
```

If you have not written Lua before, three things in this script are worth knowing:

- `--` starts a comment.
- `local` declares a variable. Without it, a variable is global.
- `..` joins two strings.

`arg` is how a script reads its command-line arguments. `arg[1]` is the first argument after the
script's name, the same as `$1` in a shell script. Lua counts from 1, not 0. When no argument is
given, `arg[1]` is `nil`, and `nil or "world"` gives `"world"`.

Everything after the script's path is passed to the script untouched, even arguments that start with
`-`:

```console
$ airsl run greet.lua --loud
hello, --loud
```

So far this looks like any other script runner. The next page shows where it is different.

> The output in this book was recorded on Linux, in a folder under `/home/you`. On your machine the
> paths in messages will name your own folders. On macOS, some system paths appear under their real
> location: `/tmp` shows as `/private/tmp`, and `/etc` as `/private/etc`.
