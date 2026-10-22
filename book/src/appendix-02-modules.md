# B. Module Reference

Every module lives under the root table, `airsstack` unless the host renamed it. Every module is
present under every preset. A call that needs a grant the script does not have raises an error that
names what was granted.

## `json`

No grant needed.

| Function | Returns |
|---|---|
| `encode(value)` | compact JSON text; object keys sorted |
| `encode_pretty(value)` | indented JSON text; object keys sorted |
| `decode(text)` | a Lua value; `null` becomes a marker that is not `nil` and encodes back as `null`; `[]` becomes a table that encodes back as `[]` |

## `regex`

No grant needed. Syntax is that of Rust's [`regex`](https://docs.rs/regex) crate.

| Function | Returns |
|---|---|
| `is_match(pattern, text)` | `true` or `false` |
| `find(pattern, text)` | the first match, or `nil` |
| `find_all(pattern, text)` | a list of every match |
| `captures(pattern, text)` | groups of the first match by number and by name, or `nil` |
| `replace(pattern, text, with)` | `text` with the first match replaced; `$1` and `$name` refer to groups |
| `replace_all(pattern, text, with)` | `text` with every match replaced |
| `split(pattern, text)` | a list of the pieces between matches |
| `compile(pattern)` | a table with all the functions above, bound to `pattern` |

## `path`

No grant needed. Works on strings only, and always writes `/`.

| Function | Returns |
|---|---|
| `join(a, b, …)` | the parts joined with `/` |
| `dirname(p)`, `basename(p)` | the folder part, the last part |
| `stem(p)`, `ext(p)` | the last part without, and only, its extension; `ext` gives `""` when there is none |
| `normalize(p)` | `p` with `.` and `..` resolved as text; not a containment check |
| `relative_to(p, base)` | `p` relative to `base`; an error if `p` is not under `base` |
| `is_absolute(p)` | `true` or `false` |
| `absolute(p)` | `p` made absolute against the current folder |

## `fs`

Needs a read or write grant for the path, as listed.

| Function | Grant | Returns |
|---|---|---|
| `read(p)` | read | the file's contents |
| `read_lines(p)` | read | a list of lines, without line endings |
| `write(p, text)` | write | nothing; replaces the file |
| `append(p, text)` | write | nothing; adds to the end |
| `atomic_write(p, text)` | write | nothing; readers never see a half-written file |
| `create_exclusive(p, text)` | write | `true` if this call created the file, `false` if it existed |
| `exists(p)`, `is_file(p)`, `is_dir(p)` | read | `true` or `false` |
| `stat(p)` | read | `{ kind, size, modified, readonly }` |
| `list(p)` | read | sorted names in the folder |
| `walk(p)` | read | sorted paths of everything below the folder, relative to it |
| `mkdir(p)` | write | nothing; creates missing parents too |
| `remove(p)`, `remove_dir(p)` | write | nothing |
| `copy(from, to)` | read `from`, write `to` | nothing |
| `rename(from, to)` | write both | nothing |
| `same_content(a, b)` | read both | `true` if the files hold the same bytes |
| `canonicalize(p)` | read | the real absolute path, symlinks followed |
| `tempfile()`, `tempdir()` | write, on the system temp folder | the new path |

## `glob`

| Function | Grant | Returns |
|---|---|---|
| `match(pattern, path)` | none | `true` or `false` |
| `walk(root, pattern)` | read `root` | sorted matching paths, relative to `root` |

`*` and `?` stop at `/`. `**/` matches any number of folders, including none. `{a,b}` matches either.

## `env`

Needs each variable's name to be granted.

| Function | Returns |
|---|---|
| `get(name)` | the value, or `nil` if granted but unset |
| `all()` | a table of every granted variable that is set |
| `set(name, value)` | nothing; seen by later `get` calls and by programs `proc` runs, never by the host |

## `proc`

Needs each program's name to be granted.

| Function | Returns |
|---|---|
| `run({program, arg, …})` | `{ status, stdout, stderr }`; a non-zero status is not an error |
| `which(program)` | the full path that would run, or `nil` |

## `time`

No grant needed.

| Function | Returns |
|---|---|
| `now()` | whole seconds since 1970 |
| `format(seconds [, pattern])` | text in UTC; the default is like `2023-11-14T22:13:20Z` |
| `parse(text [, pattern])` | seconds since 1970 |
| `monotonic()` | a reading in seconds, for measuring time between two readings |

## `hash`

| Function | Grant | Returns |
|---|---|---|
| `sha256(text)` | none | hex |
| `sha1(text)` | none | hex; for compatibility with existing names only |
| `hex(text)` | none | the bytes of `text` as hex |
| `hash_file(p)` | read | the SHA-256 of the file, as hex |

## `stdio`

No grant needed.

| Function | Returns |
|---|---|
| `read()` | all of standard input |
| `lines()` | all of standard input, as a list of lines |
| `write(text)` | nothing; writes to standard output with nothing added |
| `error(text)` | nothing; writes to standard error with nothing added |
| `isatty([stream])` | whether `"stdin"`, `"stdout"` (the default) or `"stderr"` is a terminal |

## `hook`

No grant needed.

| Function | Returns |
|---|---|
| `payload()` | standard input decoded as JSON; an empty table for empty input |
| `context(event, text)` | nothing; prints the reply that adds `text` to the agent's context |
| `emit(value)` | nothing; prints `value` as JSON |

## `ext`

No grant needed. Only useful inside a registered extension.

| Function | Returns |
|---|---|
| `on(event, handler)` | nothing; registers `handler` for an event the host declared |
| `granted()` | the grants and limits this script actually received |
