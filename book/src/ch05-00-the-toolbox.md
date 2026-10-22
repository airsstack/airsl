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

Most of them need no grant at all. Only the parts that reach outside the script ask first:

| Module | Needs a grant? |
|---|---|
| `json`, `regex`, `path`, `time`, `stdio`, `hook`, `ext` | never |
| `fs` | read or write grant, for every function |
| `env` | the variable's name |
| `proc` | the program's name |
| `glob` | only `glob.walk`, which reads folders |
| `hash` | only `hash.hash_file`, which reads a file |

This chapter is a tour, one small example per page. For the full list of functions, see the
[module appendix](appendix-02-modules.md). `env` and `proc` were covered in
[chapter 3](ch03-03-environment.md), and `ext` belongs to [chapter 8](ch08-00-extensions.md).

Throughout this chapter, `local a = airsstack` is used to keep lines short. It is an ordinary Lua
variable that refers to the same table.
