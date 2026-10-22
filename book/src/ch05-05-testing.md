# Testing and Checking

You met `airsl test` in [chapter 2](ch02-00-first-project.md). This page covers the rest of what it
does, using a small module that formats file sizes.

```lua
-- size.lua
local M = {}

function M.human(bytes)
  if bytes < 1024 then
    return bytes .. " B"
  end
  return ("%.1f KiB"):format(bytes / 1024)
end

return M
```

```lua
-- size_test.lua
local size = require("size")

return {
  small_sizes_stay_in_bytes = function()
    assert(size.human(512) == "512 B")
  end,

  larger_sizes_use_kibibytes = function()
    local got = size.human(2048)
    assert(got == "2.0 KiB", "got " .. got)
  end,
}
```

```console
$ airsl test .
./size_test.lua
  ok    larger_sizes_use_kibibytes
  ok    small_sizes_stay_in_bytes

2 passed, 0 failed (1 files)
```

## How tests are found and run

- A test file is named `*_test.lua` or `test_*.lua`, anywhere under the folder you name.
- It returns a table. Each function in the table is one test, and its key is the test's name. Tests
  run in name order, so the output is the same on every run.
- A test passes by returning, and fails by raising an error. `assert(condition, message)` raises
  when `condition` is false, so it is usually all you need.
- **Each file gets a fresh engine.** One test file cannot leave global variables behind that change
  how the next one behaves.
- **Finding no tests is a failure.** If `airsl test` finds no test files, it exits non-zero. Otherwise
  a typo in a folder name would look exactly like a passing test suite in CI.

`airsl test` takes the same `--policy` and grant flags as `airsl run`. When your tests need to read
fixture files, grant the folder:

```console
$ airsl test --allow-read tests/fixtures tests/
```

Even better, write tests that need no grants at all. As chapter 2 showed, if you keep the I/O at the
edges of a script, the logic in the middle can be tested with no permission.

## Checking without running

`airsl check` compiles every `.lua` file under a folder, and runs none of them. It finds syntax
errors in files your tests never load, such as the entry script of a hook.

```console
$ airsl check .
2 file(s) compiled, 0 failed
```

A good CI job runs both:

```console
$ airsl check scripts/
$ airsl test scripts/
```

## Summary

- `json` and `regex` handle text, and JSON keys always come out sorted.
- `path` builds paths, `glob` matches them, and `fs` reads and writes them. Only `fs` and
  `glob.walk` need a grant.
- `time.format` always needs a moment and always uses UTC, so output is the same everywhere.
- `stdio` replaces Lua's `io` for the standard streams, and `hook` handles the agent hook exchange.
- `airsl test` runs test files with a fresh engine each, and `airsl check` catches syntax errors.

That is everything a script can do from the command line. The next chapter moves to Rust, and shows
how to run scripts from your own program.
