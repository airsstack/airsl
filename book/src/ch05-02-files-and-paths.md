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
  path    builds and takes apart path strings        a relative path needs `cwd`
  glob    matches path patterns                      walk needs a read grant
  fs      reads and writes files                     always needs a grant
```

They are separate because building a path touches nothing, while opening one does. A script that
only works out file names can do that with no permission at all. The one exception is `absolute`,
described below.

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

### `path.absolute`

`path.absolute(p)` makes a relative path absolute, by putting the folder the script was started in
before it. That folder is something the script could not otherwise learn, so a relative path needs
the `cwd` member of host access. A path that is already absolute needs nothing, and comes back
unchanged.

`confined` holds `cwd`, so under the default preset the call just works. Under `pure`, a script that
calls `airsstack.path.absolute("lib/parse.lua")` is refused:

```console
$ airsl run --policy pure absolute.lua
airsl: lua error in absolute.lua: path.absolute denied: the relative path `lib/parse.lua` needs `cwd`, which is not granted — no access is granted
```

`--allow-access cwd` gives it back.

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

### Two rules before any file is touched

`fs`, `glob.walk` and `hash.hash_file` all check the path before they look at the disk, and refuse
without revealing anything about it. Two rules apply, in this order.

**No roots granted.** If nothing is granted for the kind of access the call needs, the call is
refused at once, with the path exactly as the script wrote it. Here `read-hosts.lua` reads
`/etc/hosts`:

```console
$ airsl run --policy pure read-hosts.lua
airsl: lua error in read-hosts.lua: fs.read denied: `/etc/hosts` is outside the granted read roots: none are granted
```

**A relative path needs `cwd`.** A relative path means "from the folder the script was started in".
Unless the script holds the `cwd` member of host access, a relative path is refused even when it
would have landed inside a granted folder. Here `read-notes.lua` reads `notes.md`:

```console
$ airsl run --policy pure --allow-read /usr read-notes.lua
airsl: lua error in read-notes.lua: fs.read denied: the relative path `notes.md` needs `cwd`, which is not granted — no access is granted
```

The end of the message lists the host access the script does hold. With `--allow-access stderr` added
it reads `the granted access is stderr`. Passing `--allow-access cwd` lets the relative path through
to the usual check against `/usr`.

`confined` holds `cwd`, so only the first rule can refuse a script there, as in
[chapter 2](ch02-00-first-project.md).
