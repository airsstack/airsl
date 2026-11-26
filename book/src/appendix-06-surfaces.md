# F. Lua Functions by Surface

Which of Lua's own libraries a script sees depends on its preset. The preset picks one of three
*surfaces*, and `airsl doctor` shows the choice on its `language:` line:

| Preset | `doctor` language | Surface |
|---|---|---|
| `trusted` | `full` | every safe Lua library, and `io` and `package` |
| `confined` | `restricted` | `string`, `table`, `math`, `utf8`, `coroutine` and the safe parts of `os` |
| `pure` | `minimal` | `string`, `table`, `math` and `utf8`, with no `print`, `os` or `coroutine` |

The listings below show every global and every library function each surface loads. They do not
list `airsstack`, which holds the host modules; see [B. Module Reference](appendix-02-modules.md).
For what each preset otherwise allows, see [C. Presets at a Glance](appendix-03-presets.md).

## How these listings were made

This script prints the name of every global it finds, and for each library, the names of its
functions. It skips `_G`, `arg` and `airsstack`. It is the file [`surfaces.lua`](surfaces.lua),
shown here as it is:

```lua
{{#include surfaces.lua}}
```

It writes through `airsstack.stdio.write` because that works on every surface. Run it under each
preset. The `pure` run needs `--allow-access stdout`, because `pure` starts with no access to
standard output:

```console
$ airsl run --policy trusted surfaces.lua
$ airsl run --policy confined surfaces.lua
$ airsl run --policy pure --allow-access stdout surfaces.lua
```

The listings are that output, unedited. The first line, `base:`, holds the globals that are not
libraries. Every other line is a library name followed by the names found in it.

## `full` (`trusted`)

```text
base: _VERSION assert collectgarbage dofile error getmetatable ipairs load loadfile next pairs pcall print rawequal rawget rawlen rawset require select setmetatable tonumber tostring type warn xpcall
coroutine: close create isyieldable resume running status wrap yield
io: close flush input lines open output popen read stderr stdin stdout tmpfile type write
math: abs acos asin atan atan2 ceil cos cosh deg exp floor fmod frexp huge ldexp log log10 max maxinteger min mininteger modf pi pow rad random randomseed sin sinh sqrt tan tanh tointeger type ult
os: clock date difftime execute exit getenv remove rename setlocale time tmpname
package: config cpath loaded loadlib path preload searchers searchpath
string: byte char dump find format gmatch gsub len lower match pack packsize rep reverse sub unpack upper
table: concat insert move pack remove sort unpack
utf8: char charpattern codepoint codes len offset
```

## `restricted` (`confined`)

```text
base: _VERSION assert collectgarbage error getmetatable ipairs next pairs pcall print rawequal rawget rawlen rawset require select setmetatable tonumber tostring type warn xpcall
coroutine: close create isyieldable resume running status wrap yield
math: abs acos asin atan atan2 ceil cos cosh deg exp floor fmod frexp huge ldexp log log10 max maxinteger min mininteger modf pi pow rad random randomseed sin sinh sqrt tan tanh tointeger type ult
os: clock date difftime time
string: byte char dump find format gmatch gsub len lower match pack packsize rep reverse sub unpack upper
table: concat insert move pack remove sort unpack
utf8: char charpattern codepoint codes len offset
```

## `minimal` (`pure`)

```text
base: _VERSION assert collectgarbage error getmetatable ipairs next pairs pcall rawequal rawget rawlen rawset require select setmetatable tonumber tostring type warn xpcall
math: abs acos asin atan atan2 ceil cos cosh deg exp floor fmod frexp huge ldexp log log10 max maxinteger min mininteger modf pi pow rad random randomseed sin sinh sqrt tan tanh tointeger type ult
string: byte char dump find format gmatch gsub len lower match pack packsize rep reverse sub unpack upper
table: concat insert move pack remove sort unpack
utf8: char charpattern codepoint codes len offset
```

Two entries in the `minimal` listing are not Lua's own:

- `require` is present, but only as a stub. Unless you grant the `require` kind of host access,
  `pure` puts a function there that refuses every call except `require("airsstack")` with
  `require denied`. Grant it, and `require` loads files from the script's folder, as it does under
  `confined`. See
  [Requiring the modules by name](ch02-00-first-project.md#requiring-the-modules-by-name).
- `math.randomseed` is airsl's own wrapper around Lua's. `pure` starts every engine with the
  generator seeded, so `math.random` repeats from run to run. The wrapper refuses a call with no
  argument, because that form seeds from the clock; `math.randomseed(42)` works as usual.
