# E. Performance

These numbers come from `cargo bench -p airsl` on one machine. Treat them as a rough guide to the
relative cost of things, not as a guarantee, and measure on your own hardware before relying on
them.

## Inside a Rust program

| What | With limits | Without limits |
|---|---|---|
| Building an engine | 128 µs | 102 µs |
| Running a trivial script on an existing engine | 4.6 µs | 3.4 µs |
| Running a script that makes 1000 module calls | 1073 µs | 955 µs |
| Building an engine and running a trivial script | 136 µs | 100 µs |

Two things stand out.

**Reusing an engine is about thirty times faster than building one.** That is why
[chapter 6](ch06-04-reuse.md) recommends keeping an engine for scripts that trust each other.

**The limits cost about a quarter of a small script's running time.** The instruction limit has to be
checked as the script runs. That is the price of being able to stop a script that never ends. You can
turn the limits off, but make it a decision rather than a default.

## From the command line

| Command | Time per run |
|---|---|
| `airsl run` on a trivial script | 3.2 ms |
| `python3 -c 'print("hi")'` | 13.8 ms |
| `sh -c 'echo hi'` | 2.2 ms |

Starting a process takes far longer than anything in the first table, so for the command line none
of those costs matter. `airsl` starts about four times faster than Python, and within half again of
a bare shell.
