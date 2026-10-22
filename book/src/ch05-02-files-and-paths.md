# Files and Paths

The example on this page finds every `TODO` comment in a project's Lua files, and prints where each
one is.

```lua
-- todos.lua
local a = airsstack
local root = arg[1] or "."

for _, file in ipairs(a.glob.walk(root, "**/*.lua")) do
  local lines = a.fs.read_lines(a.path.join(root, file))
  for number, line in ipairs(lines) do
    local note = a.regex.find([[TODO.*]], line)
    if note then
      print(file .. ":" .. number .. ": " .. note)
    end
  end
end
```

Point it at a project's `src` folder:

```console
$ airsl run --allow-read src todos.lua src
lib/parse.lua:12: TODO handle tabs
main.lua:3: TODO read the path from arg
```

The script lives outside `src`, which is just as well: run over its own folder, it would also find
the `TODO` in its own pattern.

## Three modules, three jobs

```
  path    builds and takes apart path strings        never needs a grant
  glob    matches path patterns                      walk needs a read grant
  fs      reads and writes files                     always needs a grant
```

They are separate because building a path touches nothing, while opening one does. A script that
only works out file names can do that with no permission at all.

## `path`

Every function in `path` works on strings. None of them looks at the disk.

```lua
a.path.join("lib", "parse.lua")          --> lib/parse.lua
a.path.dirname("lib/parse.lua")          --> lib
a.path.basename("lib/parse.lua")         --> parse.lua
a.path.stem("lib/parse.lua")             --> parse
a.path.ext("lib/parse.lua")              --> lua
a.path.ext("Makefile")                   --> (an empty string)
a.path.normalize("lib/./x/../parse.lua") --> lib/parse.lua
a.path.relative_to("/repo/lib/x.lua", "/repo")   --> lib/x.lua
```

`ext` returns an empty string, not `nil`, when there is no extension, so "no extension" is never
confused with "something went wrong".

Paths are always written with `/`, on every platform, including Windows. A script never has to care
which way the slashes go.

## `glob`

`glob.match(pattern, path)` tells you whether a path matches a pattern. `glob.walk(root, pattern)`
finds every file under `root` that matches, and returns their paths relative to `root`.

| Pattern | Matches |
|---|---|
| `*.lua` | `main.lua`, but not `lib/parse.lua`: `*` stops at a `/` |
| `**/*.lua` | `main.lua` and `lib/parse.lua`: `**/` crosses any number of folders, including none |
| `*.{lua,md}` | `main.lua` and `README.md` |

## `fs`

Reading:

```lua
local text  = a.fs.read("notes.md")          -- the whole file, as a string
local lines = a.fs.read_lines("notes.md")    -- a list of lines, without line endings
local info  = a.fs.stat("notes.md")          -- { kind = "file", size = 42, modified = …, readonly = false }
```

Writing:

```lua
a.fs.write("out.txt", text)      -- replace the file
a.fs.append("log.txt", "line\n") -- add to the end
```

Lists come back **sorted**. `fs.list`, `fs.walk` and `glob.walk` always return names in the same
order, whatever order the disk happens to store them in. Run the same script twice and you get the
same output.

### Two functions worth knowing

**`fs.atomic_write(path, text)`** replaces a file so that anyone reading it sees either the old
contents or the new contents, never half of each. Use it for any file another program might be
reading while you write it, such as an index or a cache.

**`fs.create_exclusive(path, text)`** creates a file only if it does not exist yet, as one step that
nobody can interrupt. It returns `true` if this call created it, and `false` if the file was already
there. That makes it a way to do something exactly once, even when several copies of a script start
at the same moment:

```lua
if a.fs.create_exclusive("build/.claimed", "") then
  -- only one copy of the script gets here
end
```

Checking `fs.exists` and then writing would not work. Two copies could both check, both see nothing,
and both write.
