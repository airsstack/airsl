# A First Project: Counting Headings

In this chapter you will build a small tool that lists the headings in a Markdown file. It is a
deliberately ordinary task, because what makes it interesting is not the code. It is the moment the
script is refused, and what that refusal tells you.

Along the way you will:

1. read a file, and get refused
2. grant the permission the script needs
3. split the script into a module
4. test that module without granting anything at all

Make a new folder and save this as `notes.md`:

```markdown
# Shopping

milk, eggs

## Hardware

a hammer
```

## Reading the file

Save this as `headings.lua`:

```lua
-- headings.lua
local text = airsstack.fs.read("notes.md")
local found = airsstack.regex.find_all([[(?m)^#+ .+$]], text)

for _, heading in ipairs(found) do
  print(heading)
end
```

Everything `airsl` adds to Lua lives under one global table, `airsstack`. `airsstack.fs` works with
files and `airsstack.regex` matches regular expressions. The `[[...]]` is Lua's raw string, so the
backslashes and brackets in a pattern need no escaping. `ipairs` walks a list in order.

Run it:

```console
$ airsl run headings.lua
airsl: lua error in headings.lua: fs.read denied: `/home/you/notes/notes.md` is outside the granted read roots: none are granted
stack traceback:
	[C]: in field 'read'
	headings.lua:2: in main chunk
```

The lines after the first are a stack traceback. Read them from the bottom up: line 2 of
`headings.lua` called `read`. Later examples in this book leave the traceback out to keep the output
short.

## Why it was refused

The script is not broken. It asked to read a file, and nobody had said it may.

Look closely at the message, because every refusal in `airsl` has the same shape:

```
fs.read denied: `/home/you/notes/notes.md` is outside the granted read roots: none are granted
└──────┬─────┘  └────────────┬───────────┘                                    └──────┬──────┘
   what was           what it wanted                                         what it had
   attempted
```

It tells you the operation, the exact file, and what the script *was* allowed to read. Here, nothing.

Notice also what was **not** refused. The `airsstack.regex` call would have worked fine. Matching
text reaches nothing outside the script, so it needs no permission. The same is true of building
paths, encoding JSON and formatting times. Only the tools that reach out into your machine ask
first.

## Granting what it needs

Give the script permission to read the current folder:

```console
$ airsl run --allow-read . headings.lua
# Shopping
## Hardware
```

One flag, and anyone reading the command can see exactly what this script may touch. That matters
more than it might seem. When this command lives in a hook or a CI job, the permission is written
down right next to it, instead of hidden inside the script.

A grant is a *place*, not a switch. If you grant a different folder, the script is refused again:

```console
$ airsl run --allow-read /tmp headings.lua
airsl: lua error in headings.lua: fs.read denied: `/home/you/notes/notes.md` is outside the granted read roots: /tmp
```

This time the message lists `/tmp`, so you can see at once that the grant points at the wrong
folder.

## Splitting it into a module

The script does two jobs: it reads a file, and it finds headings. Only the first needs permission.
Pull the second out into its own file, `markdown.lua`:

```lua
-- markdown.lua
local M = {}

function M.headings(text)
  return airsstack.regex.find_all([[(?m)^#+ .+$]], text)
end

return M
```

A Lua module is just a file that returns a table. Now `headings.lua` loads it with `require` and
keeps only the part that does I/O. It also takes the file name as an argument instead of hard-coding
it:

```lua
-- headings.lua
local markdown = require("markdown")

local text = airsstack.fs.read(arg[1])
for _, heading in ipairs(markdown.headings(text)) do
  print(heading)
end
```

```console
$ airsl run --allow-read . headings.lua notes.md
# Shopping
## Hardware
```

`require` only looks in the script's own folder, and in folders beneath it. A module name cannot
contain `..` or a path separator, so a script cannot reach for `require("../../secrets")`. You can
still organise modules into subfolders: `require("lib.markdown")` loads `lib/markdown.lua`.

## Testing it

Here is the payoff from the split. `markdown.headings` takes text and returns a list. It reads
nothing, so it can be tested without granting anything.

Save this as `markdown_test.lua`:

```lua
-- markdown_test.lua
local markdown = require("markdown")

return {
  finds_every_level = function()
    local found = markdown.headings("# One\ntext\n## Two\n")
    assert(#found == 2, "expected 2 headings, got " .. #found)
  end,

  ignores_a_hash_in_the_middle_of_a_line = function()
    assert(#markdown.headings("this is not a # heading") == 0)
  end,
}
```

A test file is any file named `*_test.lua` or `test_*.lua`. It returns a table of functions, one per
test. A test passes if its function returns, and fails if it raises an error, so Lua's own `assert`
is all you need. `#found` is the length of the list.

```console
$ airsl test .
./markdown_test.lua
  ok    finds_every_level
  ok    ignores_a_hash_in_the_middle_of_a_line

2 passed, 0 failed (1 files)
```

There is no `--allow-read` on that command. The tests only exercise the pure part of the code, so
they run with no permission at all.

## Summary

- Everything `airsl` adds lives under the `airsstack` global table.
- A script is refused when it reaches for something it was not granted, and the refusal names what
  it wanted and what it had.
- `--allow-read DIR` grants a place, not a general permission.
- `require` loads modules from the script's own folder and nowhere else.
- Code that does no I/O needs no permission, which makes it easy to test. Keep the I/O at the edges.

The next chapter looks at every kind of permission a script can be given.
