# text-toolkit

Parses, matches, hashes, formats and globs — under a policy that grants nothing at all.

This is its own example because the other grant-axis examples all show a module *earning*
authority, and the larger half of the roster never asks for any. `regex`, `path`, `hash`, `time`
and `glob.match` compute over the arguments they are handed and reach nothing outside them, so they
arrive whole even under `pure`, the tightest preset there is. Reading only
[`filesystem-grants`](../filesystem-grants/) and [`env-and-proc`](../env-and-proc/) leaves the
impression that an ungranted script can do nothing; it can in fact do most text work, and this is
that script.

## Run

```bash
cargo run -p airsl --example text-toolkit
```

The twin, `toolkit_test.lua`, is run by the binary rather than by a Rust host:

```bash
cargo run -p airsl-cli -- test crates/airsl/examples/text-toolkit/
```

No `--allow-*` flag, and none is needed — which is the point. A test file that required a grant
would fail for whoever ran the obvious command.

## Output

```
policy: pure — surface=minimal grants=none

regex
  split gave 5 lines, all matched by one compiled pattern
  levels: ERROR=1 INFO=2 WARN=2
  group 3 and the name `module` are the same span: true
  find_all timestamps: 5, first 2023-11-14T22:13:20Z
  replace_all: 2023-11-14T22:13:21Z WARN  fs denied: <path> is outside the granted read roots

path
  join:        crates/airsl/examples/text-toolkit/toolkit.lua
  dirname:     crates/airsl/examples/text-toolkit
  basename:    toolkit.lua
  stem / ext:  toolkit / lua
  normalize:   crates/airsl/examples/toolkit.lua
  relative_to: examples/text-toolkit/toolkit.lua
  relative_to refuses a path outside its base: true
  is_absolute: false here, true for /etc/hosts

hash
  sha256: fa59177decf3530582300814daaadc7f3e14f386fde816c45659cf685d053fa4
  sha1:   5dea9e139d482231ea03585bfb0a0e953e842c38
  cache key from sha1(path): aa5d959e
  hex: 616972736c

time
  parse 2023-11-14T22:13:20Z -> 1700000000
  format:          2023-11-14T22:13:20Z
  format %Y-%m-%d: 2023-11-14
  parse(format(t)) == t: true
  monotonic is non-decreasing across two reads: true

glob
  *.lua                          false
  **/*.lua                       true
  crates/**/text-toolkit/*.lua   true
  **/*.rs                        false
  *.lua, against toolkit.lua     true
```

And the twin:

```
crates/airsl/examples/text-toolkit/toolkit_test.lua
  ok    a_compiled_pattern_carries_the_same_operations_as_the_one_shot_forms
  ok    a_compiled_pattern_is_reusable_across_calls
  ok    a_glob_double_star_matches_zero_segments_as_well_as_many
  ok    a_glob_star_stops_at_a_directory_boundary
  ok    a_missing_extension_is_an_empty_string_and_not_nil
  ok    a_replacement_can_refer_to_a_capture
  ok    alternation_and_word_boundaries_work_where_a_lua_pattern_has_neither
  ok    an_invalid_pattern_raises_rather_than_returning_a_sentinel
  ok    captures_arrive_under_their_number_and_their_name
  ok    hashing_reproduces_the_digests_the_command_line_tools_print
  ok    is_absolute_reads_the_leading_separator_and_nothing_else
  ok    monotonic_does_not_go_backwards
  ok    normalize_resolves_dots_without_consulting_the_filesystem
  ok    parse_reads_back_what_format_wrote
  ok    parsing_something_that_is_not_a_date_raises
  ok    path_splits_a_name_into_a_stem_and_an_extension
  ok    relative_to_refuses_a_path_that_is_not_under_its_base
  ok    the_same_input_hashes_to_the_same_digest
  ok    time_formats_in_utc_so_the_output_does_not_depend_on_the_machine

19 passed, 0 failed (1 files)
```

## What it demonstrates

- **`pure` is not a reduced runtime.** It selects the smallest language surface — `string`, `table`,
  `math`, `utf8` and nothing else (`src/sandbox/language_surface.rs:86`) — and grants nothing
  (`src/sandbox/policy.rs:87`), and every call in `toolkit.lua` still runs. The surface governs
  *Lua's own* libraries; what the host modules may touch is a separate question, and these five ask
  for none of it.
- **A compiled pattern is compiled once.** `regex.compile` exists because the one-shot forms rebuild
  the pattern on every call (`src/modules/regex.rs:162`), which a script scanning a file pays once
  per line. The handle it returns carries the same operations bound to one pattern
  (`src/modules/regex.rs:79`), so the example's loop over five lines compiles once, above the loop —
  and `toolkit_test.lua`'s `a_compiled_pattern_is_reusable_across_calls` holds the handle across
  three separate matches.
- **Captures arrive under their number and their name at once**, rather than one instead of the
  other (`src/modules/regex.rs:68`), so a pattern can be read either way without the caller knowing
  which style it was written in. The `group 3 and the name ...` line above is that asserted rather
  than described.
- **Alternation and `\b` are why this module exists.** The log pattern uses `INFO|WARN|ERROR` and
  `(?<name>...)`; a Lua pattern has neither, and a script needing them under raw Lua ends up
  shelling out.
- **`path` is separate from `fs` because it needs no authority** (`src/modules/path.rs:3`). None of
  the paths printed above exists on the machine that printed them, and none was opened: `normalize`
  cancels `..` textually rather than asking the operating system (`src/modules/path.rs:168`). The
  corollary is worth keeping: normalising is *not* a containment check. That decision belongs to
  `PathGuard` (`src/modules/guard.rs:57`), against a canonical path, inside `fs`.
- **`relative_to` refuses a path outside its base** (`src/modules/path.rs:207`) instead of answering
  `../../elsewhere`. The caller asked where this is *under* that; a path that is not under it has no
  answer to that question, and inventing one hides the mistake.
- **`ext` returns an empty string, never `nil`** (`src/modules/path.rs:158`), so absence and failure
  are never the same value and the result concatenates without a check.
- **SHA-1 ships for compatibility, not for strength** (`src/modules/hash.rs:82`). `cache key from
  sha1(path)` is the Lua spelling of `shasum | cut -c1-8` — a script naming artifacts that already
  exist on disk, where offering only SHA-256 would silently re-key every one of them. `hex` is
  lowercase for the same reason: it is what the command-line tools print
  (`src/modules/hash.rs:48`).
- **`time.format` takes an explicit instant and renders UTC** (`src/modules/time.rs:8`,
  `src/modules/time.rs:99`). There is no "now" default to fall into, which is what lets a script
  write a timestamp into a file and get the same bytes on every machine. The instant in the output
  above is parsed back out of the log corpus, which is why the line is reproducible.
- **A reading is not a fact, so no reading is printed.** `time.monotonic` returns something
  different on every run (`src/modules/time.rs:80`), so the example prints the *property* a script
  measuring a duration actually depends on: a second reading is never below the first.
  `time.now` and `path.absolute` are absent from the script for the same reason — both need no
  grant, and both would put a different value in this README's output block on every run.
- **`glob`'s `*` stops at a directory separator** (`src/modules/glob.rs:48`), which is why `*.lua`
  is `false` for the nested path and `true` for the bare filename, while `**` stays recursive. The
  looser reading would let a rule declaring `*.lua` apply to files its author never named — and
  widening authority is the one direction a matcher must not be wrong in.
- **A test file needs no framework.** `airsl test` discovers `*_test.lua` and `test_*.lua`, expects
  a table of named functions, and reads a return as a pass and a raise as a failure
  (`crates/airsl-cli/src/test_runner.rs:11`) — so Lua's own `assert` is the whole assertion surface.
  Both the discovery order and the order of cases within a file are sorted, so two machines report
  the same failures in the same order.

### The two halves of `hash` and `glob`

Both modules straddle the line this example is drawn along, which is why only half of each appears
above. `hash.hash_file` reads a file (`src/modules/hash.rs:97`) and `glob.walk` reads directories
(`src/modules/glob.rs:102`); both go through the same `PathGuard` every `fs` call does, and both
need the read grant. `hash.sha256` and `glob.match` are handed their input and need nothing. That
split is the rule about modules being capabilities applied *inside* a module rather than between
two — see [`denials-are-data`](../denials-are-data/) for what either one says when it is called
without the grant.

## See also

- [`policy-presets`](../policy-presets/) — what each preset's language surface withholds.
- [`filesystem-grants`](../filesystem-grants/) — the other half of the roster, and what it costs.
- [host standard library](../../docs/stdlib.md) — the full module roster and the grant each carries.
