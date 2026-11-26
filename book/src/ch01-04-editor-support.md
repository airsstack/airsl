# Editor Support

Your editor can complete `airsstack.json.encode` and warn you when you misspell it. It needs a
description of what `airsl` provides, and `airsl` writes that description for you. This page uses the
[Lua language server](https://luals.github.io/), the engine behind the Lua support in VS Code,
Neovim, Zed and others.

Make a folder with a script in it. Save this as `hello.lua`:

```lua
local text = airsstack.json.encode({ name = "airsl" })
print(text)
```

The script runs fine with `airsl run hello.lua`, but the language server has never heard of
`airsstack`. This is what its checker says:

```console
$ lua-language-server --check . --checklevel=Information
Initializing ...
hello.lua:1:14 [Warning] Undefined global `airsstack`. (undefined-global)
    local text = airsstack.json.encode({ name = "airsl" })
                 ^^^^^^^^^
>>>>>>>>>>>>>>>>>>>> 1/1 [Found 1 problems in 1 files]
Diagnosis complete, 1 problems found
```

## Setting a project up

Run this in the project's folder:

```console
$ airsl lua setup
created .airsl/types/airsstack.lua
created .luarc.json
```

To set up another folder, give its path: `airsl lua setup path/to/project`.

Check the script again:

```console
$ lua-language-server --check . --checklevel=Information
Initializing ...
>>>>>>>>>>>>>>>>>>>> 2/2
Diagnosis completed, no problems found
```

The warning is gone, and your editor now completes every `airsstack` module and checks each call. A
misspelled function is reported. Save a copy of the script as `typo.lua` with `encode` spelled
`encod`, and the checker answers:

```console
$ lua-language-server --check . --checklevel=Information
Initializing ...
typo.lua:1:29 [Warning] Undefined field `encod`. (undefined-field)
    local text = airsstack.json.encod({ name = "airsl" })
                                ^^^^^
>>>>>>>>>>>>>>>>>>>> 3/3 [Found 1 problems in 1 files]
Diagnosis complete, 1 problems found
```

A script can also take the modules from `require("airsstack")`, which returns the same table as the
global. The language server follows it the same way. Change `typo.lua` to this, and it reports the
misspelled field on the local:

```lua
local airsstack = require("airsstack")
local text = airsstack.json.encod({ name = "airsl" })
print(text)
```

```console
$ lua-language-server --check . --checklevel=Information
Initializing ...
typo.lua:2:29 [Warning] Undefined field `encod`. (undefined-field)
    local text = airsstack.json.encod({ name = "airsl" })
                                ^^^^^
>>>>>>>>>>>>>>>>>>>> 3/3 [Found 1 problems in 1 files]
Diagnosis complete, 1 problems found
```

Completion works from the same type. For the form itself, see
[Requiring the modules by name](ch02-00-first-project.md#requiring-the-modules-by-name).

## What it writes

Setup writes two things:

- `.airsl/types/airsstack.lua`, a file that describes every built-in module. It holds only
  descriptions, and nothing in it runs.
- An entry in `.luarc.json`, the language server's settings file: `.airsl/types` is added to
  `workspace.library`. That entry is how the server finds the description.

If the project has no `.luarc.json` but has a `.luarc.jsonc`, setup edits that one instead.

Settings you already have are kept. Here is a `.luarc.json` that holds two settings of its own, after
setup:

```json
{
  "diagnostics.globals": [
    "vim"
  ],
  "runtime.version": "Lua 5.4",
  "workspace.library": [
    ".airsl/types"
  ]
}
```

When setup has to change the file, it writes the whole file again with its keys sorted.
Your settings stay, but the file's own layout does not. A file that already has the entry is not
touched.

Setup only edits a file of plain JSON. If yours has comments, setup refuses, writes nothing, and says
what to do:

```console
$ airsl lua setup
airsl: will not edit `./.luarc.json`: it is not plain JSON (key must be a string at line 2 column 3); add ".airsl/types" to `workspace.library` by hand
```

Add the entry yourself, to the `workspace.library` list, and the language server will find the file.

## Running it again

It is safe to run setup as often as you like. When nothing needs to change, it says so:

```console
$ airsl lua setup
unchanged .airsl/types/airsstack.lua
unchanged .luarc.json
```

Run it again after you upgrade `airsl`. The description comes from the version you have installed,
and a new version may add functions. Setup rewrites only what is out of date:

```console
$ airsl lua setup
updated .airsl/types/airsstack.lua
unchanged .luarc.json
```

## Should `.airsl/` be committed?

You can put `.airsl/` in `.gitignore`. It is generated, and anyone can recreate it with
`airsl lua setup`. The `.luarc.json` entry is the part that matters. It is what lets the language
server find the description, so commit that file, and teammates only need to run setup once.

## Summary

- `airsl lua setup` writes a description of the built-in modules and tells the language server where
  it is.
- Your existing settings are kept. A file with comments is refused, and you add the entry yourself.
- Run it again after upgrading `airsl`.
- To describe a module of your own, see [Writing Your Own Module](ch07-00-your-own-module.md).
