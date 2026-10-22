# Running Programs

This example asks `git` which branch you are on:

```lua
-- branch.lua
local result = airsstack.proc.run({"git", "branch", "--show-current"})

if result.status ~= 0 then
  airsstack.stdio.error("git failed: " .. result.stderr)
else
  airsstack.stdio.write("on branch " .. result.stdout)
end
```

```console
$ airsl run --allow-exec git branch.lua
on branch main
```

## No shell, ever

`proc.run` takes a list: the program, then each argument as its own string. There is no version that
takes a single command string.

That one rule removes a whole family of bugs. When a command is a string, something has to split it
into words, and a file name with a space or a quote in it gets split wrong. Worse, a file name like
`x; rm -rf ~` gets run. With a list, there is nothing to split. Each argument arrives exactly as you
wrote it.

## A failing program is not an error

`run` gives back a table with three fields:

| Field | Holds |
|---|---|
| `status` | the exit code; `0` means success |
| `stdout` | everything the program printed |
| `stderr` | everything it printed as errors |

If `git` exits with a non-zero status, that is not a Lua error. The script asked what happened, and
`run` tells it. Check `status` yourself, as the example does.

Running a program the script was *not* granted is an error, like every other refusal.

## The grant matches the name you wrote

`--allow-exec git` allows `{"git", ...}`. It does not allow `{"/usr/bin/git", ...}`, even if that is
the same file. The grant is compared with the program name exactly as the script spelled it, so use
the bare name, and let `airsl` find it on `PATH` the way your shell would.

`proc.which("git")` returns the full path `airsl` would run, or `nil` if it cannot find one.

On Windows, `git` finds `git.exe` the same way. One limit: `airsl` runs `.exe` programs only, so
tools that install as `.cmd` scripts, such as `npm` and `npx`, cannot be run through `proc` on
Windows.
