# Standard Streams and Hooks

Under `confined`, Lua's own `io` library is gone, because it can open any file. A script still needs
its standard input and output, though, and `airsstack.stdio` provides them. Reading and writing the
streams the script was started with needs no grant.

| Function | Does |
|---|---|
| `stdio.read()` | reads all of standard input, as one string |
| `stdio.lines()` | reads all of standard input, as a list of lines |
| `stdio.write(text)` | writes to standard output, exactly as given |
| `stdio.error(text)` | writes to standard error, exactly as given |
| `stdio.isatty(stream)` | whether `"stdin"`, `"stdout"` or `"stderr"` is a terminal |

`write` and `error` add nothing to what they are given, not even a newline. Lua's `print` always
adds one. When your output is going to be read by another program, every byte counts, so `write` is
the safer choice.

## Writing a hook

Many tools let you attach a script that runs when something happens. AI coding agents are a common
example: before the agent edits a file, it runs your hook, hands it a JSON description of what it is
about to do on standard input, and reads back any JSON the hook prints.

`airsstack.hook` handles that exchange. This hook adds a reminder whenever the agent is about to
touch a Lua file:

```lua
-- remind.lua
local a = airsstack

local payload = a.hook.payload()
local file = payload.tool_input and payload.tool_input.file_path

if file and a.path.ext(file) == "lua" then
  a.hook.context("PreToolUse", "Reminder: run `airsl test` after changing Lua files.")
end
```

```console
$ echo '{"tool_input":{"file_path":"lib/parse.lua"}}' | airsl run remind.lua
{"hookSpecificOutput":{"additionalContext":"Reminder: run `airsl test` after changing Lua files.","hookEventName":"PreToolUse"}}

$ echo '{"tool_input":{"file_path":"README.md"}}' | airsl run remind.lua
```

The second run prints nothing, and that is correct. A hook with nothing to say says nothing.

| Function | Does |
|---|---|
| `hook.payload()` | reads standard input as JSON; an empty input gives an empty table |
| `hook.context(event, text)` | prints the reply that adds `text` to the agent's context |
| `hook.emit(value)` | prints any Lua value as JSON, for replies `context` does not cover |

`context` builds the nested reply shape for you, which is easy to get subtly wrong by hand.

When you attach a hook like this, run it with `--fail-open`, as described in
[chapter 4](ch04-02-failing-safely.md), so that a bug in the hook never blocks the agent.

## Calling `airsl` from a hook

A hook is usually started by a small shell script. This one stays silent if `airsl` is not installed,
instead of failing:

```sh
#!/bin/sh
DIR=$(CDPATH= cd -- "$(dirname -- "$0")" 2>/dev/null && pwd) || exit 0
command -v airsl >/dev/null 2>&1 || exit 0
airsl run --fail-open "$DIR/remind.lua" || exit 0
exit 0
```

Every path through it exits `0`. If the hook never seems to run, check that the program running it
can find `airsl`. A hook runner may start a shell that never reads your profile, and so never sees
the `PATH` change the installer made. Writing the full path, `"$HOME/.local/bin/airsl"`, removes the
question.
