# Files

The example on this page is a tiny backup tool. It copies a file from one folder into another, which
needs one read grant and one write grant.

```lua
-- backup.lua
local from, to = arg[1], arg[2]

local text = airsstack.fs.read(from)
airsstack.fs.write(to, text)
print("copied " .. #text .. " bytes")
```

Set up two folders and run it:

```console
$ mkdir -p config backups
$ echo "port = 8080" > config/app.toml

$ airsl run --allow-read config --allow-write backups backup.lua config/app.toml backups/app.toml
copied 12 bytes
```

## Reading and writing are separate

A read grant does not let a script write, and a write grant does not let it read. Try writing back
into the folder the script may only read:

```console
$ airsl run --allow-read config --allow-write backups backup.lua config/app.toml config/copy.toml
airsl: lua error in backup.lua: fs.write denied: `/home/you/project/config/copy.toml` is outside the granted write roots: /home/you/project/backups
```

The common shape is a wide read grant and a narrow write grant: read the whole project, write only to
its `build` folder. Keeping the two lists separate is what lets you say that.

It also means a write grant is not readable. If a script writes a file and then wants to read it
back, grant the folder both ways:

```console
$ airsl run --allow-read backups --allow-write backups script.lua
```

## A grant covers everything below it

`--allow-read config` covers `config/app.toml`, `config/env/prod.toml`, and anything else beneath
`config`. It does not cover `config`'s parent, or a sibling folder.

Paths are checked after they are fully resolved, so a script cannot sneak out of a granted folder:

```
  granted:  /home/you/project/config

  config/app.toml                    ✓  inside
  config/../secrets.txt              ✗  ".." leaves the folder
  config/link-to-home/.ssh/id_rsa    ✗  a symlink that points outside is followed, then refused
```

The check happens inside `airsl`, in Rust, right before the file is opened. A Lua script never holds
an open file. It only ever holds a name, and asks `airsl` to act on it.

## Asking about files counts as reading

Listing a folder, checking whether a file exists, and reading a file's size all need a read grant,
because each one tells the script something about the folder. A write grant alone is not enough to
`list` the folder you are writing into.

## What `fs` can do

| To… | Use |
|---|---|
| read a whole file, or its lines | `fs.read`, `fs.read_lines` |
| write or add to a file | `fs.write`, `fs.append` |
| replace a file so no reader ever sees half of it | `fs.atomic_write` |
| create a file only if nobody else has | `fs.create_exclusive` |
| look at a path | `fs.exists`, `fs.is_file`, `fs.is_dir`, `fs.stat` |
| list a folder, or walk it recursively | `fs.list`, `fs.walk` |
| create and remove | `fs.mkdir`, `fs.remove`, `fs.remove_dir` |
| copy, move, compare | `fs.copy`, `fs.rename`, `fs.same_content` |

[Chapter 5](ch05-02-files-and-paths.md) shows these in use, and the
[module appendix](appendix-02-modules.md) lists every function.
