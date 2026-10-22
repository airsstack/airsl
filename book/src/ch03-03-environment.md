# Environment Variables

Your shell's environment holds more than it seems: API tokens, cloud credentials, the path to your
SSH agent. A script that needs `HOME` should not see all of that along with it. So environment
grants are by name, one variable at a time.

```lua
-- whoami.lua
local user = airsstack.env.get("USER")
local home = airsstack.env.get("HOME")
print(user .. " lives in " .. home)
```

```console
$ airsl run --allow-env USER --allow-env HOME whoami.lua
you lives in /home/you
```

## Denied and unset are different

Asking for a variable that was not granted raises an error:

```console
$ airsl run --allow-env HOME whoami.lua
airsl: lua error in whoami.lua: env.get denied: `USER` is not granted — the allowed names are HOME
```

Asking for a variable that *was* granted but is not set returns `nil`. That difference is what lets a
script tell "you did not give me `API_URL`" apart from "`API_URL` is not set", which are two very
different things to fix.

`env.all()` returns a table of every granted variable that is set. It never includes variables the
script was not granted.

## Setting a variable

`env.set` changes a variable for the rest of the script, and for any program the script runs:

```lua
airsstack.env.set("GREETING", "hello")
print(airsstack.env.get("GREETING"))   --> hello
```

The variable needs a grant, the same as reading it. And the change stays inside `airsl`. It never
changes the environment of the program that started the script. A script should not be able to
quietly reconfigure its host.
