# Asking Permission

In the last chapter, a script was refused until it was given a folder to read. This chapter covers
the permissions a script can be given, and the rules behind them.

Four of them name a place, a variable or a program, and each one has its own flag:

| Flag | What it grants | Example |
|---|---|---|
| `--allow-read DIR` | reading files under `DIR` | `--allow-read ./docs` |
| `--allow-write DIR` | writing files under `DIR` | `--allow-write ./build` |
| `--allow-env NAME` | reading and setting the variable `NAME` | `--allow-env HOME` |
| `--allow-exec PROGRAM` | running the program `PROGRAM` | `--allow-exec git` |

Every flag can be repeated, so a script can be granted several folders, names or programs.

A permission in `airsl` is called a **grant**. Each grant names exactly what it covers: a folder, a
variable, a program. There is no "may read files" or "may run programs". There is only "may read
files *here*" and "may run *this* program".

A fifth, smaller kind of permission names no place at all: writing output, reading the clock, using
a relative path. It is called host access, and it has its own flag, `--allow-access`. It is covered
under [Host access](ch03-01-nothing-by-default.md#host-access).
