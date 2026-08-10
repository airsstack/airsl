# Tutorial: your first airsl script

By the end of this you will have written a Lua script that reads a file, split it into a module,
tested it, and turned it into an agent hook — and you will have hit the sandbox once on purpose,
because that is the fastest way to understand what `airsl` is for.

Everything here is run from one directory. Nothing is installed except the binary.

## Install the binary

```bash
cargo install --path crates/airsl-cli --force
airsl doctor
```

```
airsl 0.1.0
  lua:          Lua 5.4
  language:     restricted
  root table:   airsstack
  grants:       none
  memory:       67108864 bytes
  instructions: 100000000 instructions
  modules:      json, path, fs, env, proc, regex, hash, time, glob, stdio, hook
```

`doctor` describes what a script would actually get. Read the last three lines now, because they are
the whole model: a script gets eleven host modules, two resource ceilings, and **no grants**.

## 1. Run something

```lua
-- greet.lua
local who = arg[1] or "world"
print("hello, " .. who)
```

```console
$ airsl run greet.lua airsl
hello, airsl
```

`arg` is Lua's own convention for a standalone script: `arg[1]` is the first argument, where a shell
script would read `$1`, and `arg[0]` is the script's own name.

## 2. Hit the sandbox

Now make it read a file. Save this as `notes.md`:

```markdown
# One

text

## Two
```

and this as `count.lua`:

```lua
local notes = airsstack.fs.read("notes.md")
print(#airsstack.regex.find_all([[(?m)^#+ .+$]], notes) .. " headings")
```

```console
$ airsl run count.lua
airsl: lua error in count.lua: fs.read denied: `/home/you/demo/notes.md` is outside them — no read roots are granted
```

**This is the point of the crate, so it is worth pausing on.** The script is not broken. `airsl` runs
it under a policy that grants nothing, and reading a file needs authority the script was not given.
The refusal names the file it wanted and the roots that were granted — here, none.

Two things are already true and worth noticing:

- `airsstack.regex` worked without any grant. Matching text reaches nothing, so it needs no
  authority. The same goes for `path`, `time`, `stdio` and `json`.
- The failure is a normal Lua error. `pcall` catches it like any other.

## 3. Grant what it needs

```console
$ airsl run --allow-read . count.lua
2 headings
```

One flag, and the authority is visible in the command rather than buried somewhere. That matters
more than it looks: whoever reads the invocation can see exactly what the script may touch.

Try `--allow-read /etc` instead and you will get the refusal back — the grant is a *place*, not a
switch.

## 4. Split it into a module

A script on disk may `require` its siblings. Put the interesting part in `headings.lua`:

```lua
-- headings.lua
local M = {}

function M.find(text)
  return airsstack.regex.find_all([[(?m)^#+ .+$]], text)
end

return M
```

and reduce `count.lua` to the part that does I/O:

```lua
-- count.lua
local headings = require("headings")
local notes = airsstack.fs.read(arg[1])
for _, h in ipairs(headings.find(notes)) do
  print(h)
end
```

```console
$ airsl run --allow-read . count.lua notes.md
# One
## Two
```

`require` here is not Lua's own — it resolves only under the script's own directory. A target cannot
contain a path separator or a `..`, so `require("../secrets")` is not something you can spell.

Note what the split bought: `headings.find` is now pure. It takes text and returns a list, and needs
no grant at all. That is worth doing deliberately, because a pure function is one you can test
without giving the test any authority.

## 5. Test it

A test file is named `*_test.lua` or `test_*.lua` and returns a table of named functions. A test
passes by returning and fails by raising, so Lua's own `assert` is the entire assertion surface.

```lua
-- headings_test.lua
local headings = require("headings")

return {
  finds_every_level = function()
    local found = headings.find("# One\ntext\n## Two\n")
    assert(#found == 2, "expected 2, got " .. #found)
  end,

  ignores_a_hash_mid_line = function()
    assert(#headings.find("not a # heading") == 0)
  end,
}
```

```console
$ airsl test .
./headings_test.lua
  ok    finds_every_level
  ok    ignores_a_hash_mid_line

2 passed, 0 failed (1 files)
```

No `--allow-read` needed: the tests exercise the pure function, so they run with no authority at all.
That is not a trick, it is the payoff from step 4.

## 6. Make it a hook

Agent hooks receive a JSON payload on stdin and may write one back. `airsstack.hook` is that
contract in one place:

```lua
-- hook.lua
local payload = airsstack.hook.payload()
local file = payload.tool_input and payload.tool_input.file_path

if file and airsstack.path.ext(file) == "lua" then
  airsstack.hook.context("PreToolUse", "Reminder: airsl scripts run under a policy.")
end
```

```console
$ echo '{"tool_input":{"file_path":"hooks/enforce.lua"}}' | airsl run hook.lua
{"hookSpecificOutput":{"additionalContext":"Reminder: airsl scripts run under a policy.","hookEventName":"PreToolUse"}}

$ echo '{"tool_input":{"file_path":"README.md"}}' | airsl run hook.lua
```

The second produces nothing, which is correct: a hook that has nothing to say says nothing.

## 7. Make it safe to fail

A hook that fails must not fail the thing that triggered it. A `PreToolUse` hook exiting non-zero
blocks the tool call, and those matchers commonly cover `Read` — so a script with a typo in it would
block every file read in the session.

```console
$ echo 'this is not lua' > broken.lua

$ echo '{}' | airsl run broken.lua ; echo "exit=$?"
exit=1

$ echo '{}' | airsl run --fail-open broken.lua ; echo "exit=$?"
exit=0
```

`--fail-open` is a command-line flag rather than something you write in the script, and that is
deliberate: a syntax error happens before any in-script setting could take effect, which is exactly
the case the flag exists for.

Set `AIRSL_DEBUG=1` to see the error on stderr while keeping the exit code at zero.

## What you now know

- A script gets host modules but no authority, and authority is granted per run, per place.
- Pure functions need no grants, which makes them cheap to test — so push logic out of the I/O.
- `require` is confined to the script's own directory.
- Hooks read a payload and write an envelope, and must fail open.

## Where to go next

- **[How-to](how-to.md)** — recipes for specific jobs: walking a tree, running a program, embedding
  the runtime in Rust, adding your own module.
- **[Host standard library](stdlib.md)** — the full roster and what each module owes its callers.
- **[Sandbox](sandbox.md)** — what a grant is, where it is enforced, and what the sandbox can and
  cannot promise.
