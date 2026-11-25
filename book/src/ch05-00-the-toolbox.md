# The Toolbox

Lua is a small language, and its standard library is small too. It has no JSON, no real regular
expressions, no way to list a folder, and no way to run a program without going through a shell.
Scripts written in plain Lua tend to fill those gaps by calling out to Python or `sed`, which brings
back every problem `airsl` exists to solve.

So `airsl` adds twelve modules of its own, all under the `airsstack` table:

```
airsstack
├── text          json · regex
├── files         path · fs · glob
├── the system    env · proc
├── utilities     time · hash
└── the outside   stdio · hook · ext
```

`json`, `regex` and `ext` need nothing at all. The rest ask first, because they reach outside the
script: the disk, the environment, other programs, the clock, the current folder, or the standard
streams. Some ask for a grant of a place or a name. Others ask for a member of *host access*, a
short list of things a script may use that `confined` holds and `pure` does not. Host access is
described in [chapter 3](ch03-01-nothing-by-default.md).

| Module | Needs a grant? |
|---|---|
| `json`, `regex`, `ext` | never |
| `fs` | read or write grant, for every function; a relative path also needs `cwd` |
| `env` | the variable's name |
| `proc` | the program's name |
| `glob` | only `glob.walk`, which reads folders; a relative root also needs `cwd` |
| `hash` | only `hash.hash_file`, which reads a file; a relative path also needs `cwd` |
| `path` | only `path.absolute` of a relative path: `cwd` |
| `time` | `now` and `monotonic`: `clock`. `parse` with a pattern that names a time zone: `zones` |
| `stdio` | `read`, `lines`: `stdin`. `write`: `stdout`. `error`: `stderr`. `isatty`: the stream it asks about |
| `hook` | `payload`: `stdin`. `emit`, `context`: `stdout` |

This chapter is a tour, one small example per page. For the full list of functions, see the
[module appendix](appendix-02-modules.md). `env` and `proc` were covered in
[chapter 3](ch03-03-environment.md), and `ext` belongs to [chapter 8](ch08-00-extensions.md).

Throughout this chapter, `local a = airsstack` is used to keep lines short. It is an ordinary Lua
variable that refers to the same table.
