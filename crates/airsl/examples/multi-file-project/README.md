# multi-file-project

A Lua project split across files, resolving against its own directory and nowhere else.

This gets its own example because `require` is the one global whose *identity* changes with the
language surface rather than merely its availability. `trusted` keeps Lua's own loader, which
searches `package.path` — the process's working directory and its installation prefixes — and can
open anything on the machine. `confined` replaces it with a Rust function that resolves under the
directory the script was read from, has no `package` table to configure, and refuses a name that
could describe a parent. To a script that only checks whether the global is there, both answer
"yes", which is where [policy-presets](../policy-presets/) has to stop. This is where the two are
pulled apart.

The project is a real one rather than a fixture. `app.lua` requires `lib` and `lib.text`,
`lib/init.lua` requires `lib.text` as well, and `ring` requires itself the long way round.

## Run

```bash
cargo run -p airsl --example multi-file-project
```

Every `.lua` file here also runs on its own. The host's two arguments are optional, and without
them the refusals name the real directory instead of `<project>`:

```bash
airsl run crates/airsl/examples/multi-file-project/app.lua
airsl run --policy trusted crates/airsl/examples/multi-file-project/which_require.lua
```

## Output

```
-- a fresh engine
evaluation 1 on this engine
lib.version: 1.0.0
lib.text.slug("Multi File Project"): multi-file-project
lib.text is the table lib itself required: true
module bodies run so far: lib, lib.text
require("../secrets"): invalid require target `../secrets`: must contain only ASCII letters, digits, underscores, hyphens and dots
require("missing"): module `missing` not found under `<project>`
require("ring"): lua error in ring: lua error in ring.link: module `ring` requires itself, directly or indirectly, under `<project>`

-- the same engine again
evaluation 2 on this engine
lib.version: 1.0.0
lib.text.slug("Multi File Project"): multi-file-project
lib.text is the table lib itself required: true
module bodies run so far: lib, lib.text
require("../secrets"): invalid require target `../secrets`: must contain only ASCII letters, digits, underscores, hyphens and dots
require("missing"): module `missing` not found under `<project>`
require("ring"): lua error in ring: lua error in ring.link: module `ring` requires itself, directly or indirectly, under `<project>`

-- a second engine
evaluation 1 on this engine
lib.version: 1.0.0
lib.text.slug("Multi File Project"): multi-file-project
lib.text is the table lib itself required: true
module bodies run so far: lib, lib.text
require("../secrets"): invalid require target `../secrets`: must contain only ASCII letters, digits, underscores, hyphens and dots
require("missing"): module `missing` not found under `<project>`
require("ring"): lua error in ring: lua error in ring.link: module `ring` requires itself, directly or indirectly, under `<project>`

-- one name, three functions
trusted  require=native   package=yes searchers=3  require("../secrets"): searched for it as a path
confined require=confined package=no  searchers=0  require("../secrets"): refused, the name is unrepresentable
pure     require=absent   package=no  searchers=0  require("../secrets"): there is no require to call
memory   require=absent   and no directory that could give it one
```

`<project>` is substituted by `app.lua` itself, from the directory the host hands it. Every line
above is printed from Lua and none of it from Rust — not because the two would reorder (they flush
per line and interleave correctly, as [`custom-module`](../custom-module/) demonstrates by doing
it), but because the substitution above has to happen on the Lua side, and a heading printed from
Rust would be the one line in the block that had not been through it.

## What it demonstrates

- **Two different functions behind one name.** `RequireDisposition::decide`
  (`src/require_loader.rs:67`) answers with three values rather than two: `Full` keeps the loader
  `package` installed (`src/require_loader.rs:69`), `Restricted` *with a root* gets the confined one
  (`src/require_loader.rs:70`), and everything else has the global cleared
  (`src/require_loader.rs:71`). The `trusted` row is the native loader — `package` is present, its
  searcher list is there to append to, and `require("../secrets")` is a path it goes off and looks
  for. The `confined` row has no `package` at all, so there is nothing to configure and no way to
  widen where it looks.

- **The root is inferred, not configured.** `Script::from_file` takes it from the file's parent
  directory (`src/script.rs:63`), and a bare filename roots at `.` rather than at nothing
  (`src/script.rs:169`) — otherwise `app.lua` and `./app.lua` would name the same file and only one
  of them would get a `require`. `Script::from_source` records no root at all
  (`src/script.rs:37`), which is why the last line of the output reads `require=absent` under the
  very policy the row above it calls `confined`: there was no directory to confine a loader to, and
  falling back to the working directory would have made the same script resolve differently
  depending on where the host was started.

- **`require("../secrets")` is refused at the name, before any path exists.** `RequireTarget::new`
  accepts only ASCII letters, digits, `_`, `-` and `.` (`src/types/require_target.rs:52`) and
  rejects any empty dot-separated component (`src/types/require_target.rs:58`), so `..` has no
  spelling as a target. That is a stronger guarantee than a path check rather than a tidier one: no
  candidate path is built, so there is nothing to canonicalise, nothing to compare, and no window
  between the check and the open. The `trusted` row is the contrast — the same string there is a
  relative path, and the loader goes to the filesystem to ask about it.

- **A dot is a path separator, and a directory answers through `init.lua`.**
  `RequireTarget::candidates` returns `<stem>.lua` and then `<stem>/init.lua`
  (`src/types/require_target.rs:77`), so `require("lib")` finds `lib/init.lua` where there is no
  `lib.lua` — a component keeps its parts in a subdirectory and is still required by the name of
  the directory. Every target resolves against the script root and never against the requiring
  file, so `require("lib.text")` means the same file whether `app.lua` writes it or `lib/init.lua`
  does.

- **A module body runs once, and the cache outlives the evaluation.** The cache lives in the
  registry under a single key (`src/require_loader.rs:29`), is created once and kept rather than
  rebuilt per evaluation (`src/require_loader.rs:99`), and is keyed by the canonical path the
  target resolved to (`src/require_loader.rs:147`). `lib/text.lua` is required twice — by
  `lib/init.lua`, then by `app.lua` — and appends its name to `BODIES_RUN` whenever its body runs.
  The list reads `lib, lib.text` on `evaluation 1` and still reads `lib, lib.text` on
  `evaluation 2` of the same engine. `lib.text is the table lib itself required: true` is the other
  half of it: the second call returns the value the first produced, not an equal copy.

- **A second engine starts over.** The third block is the control for the second. Same script, same
  policy, `evaluation 1` again — so the unchanged list in the middle block is the cache surviving,
  not `app.lua` printing a constant.

- **A cycle is answered rather than recursed into.** The loader writes an in-progress marker before
  running a module (`src/require_loader.rs:164`) and reads finding one as a cycle
  (`src/require_loader.rs:154`), raising `Error::RequireCycle` (`src/require_loader.rs:155`).
  Without it, `ring` → `ring.link` → `ring` would end in a stack overflow, which aborts the process
  rather than raising something `pcall` can catch. The marker does not survive a failure
  (`src/require_loader.rs:183`), and that is why the same message appears in all three blocks
  instead of the diagnosis changing after the first: a module that raised once must not afterwards
  be reported as a cycle for as long as the engine lives.

- **A missing module names the one directory that was searched** (`src/require_loader.rs:226`).
  There is no second place to look — no `package.path`, no working directory, no environment
  variable — so the message is the whole search rather than a summary of it.

### The one escape a validated name cannot rule out

Not in the output above, deliberately: showing it would mean committing a symlink into the
repository. A target cannot spell a parent, but a symlink *inside* the root can point out of it,
and no amount of name validation sees that. So `resolve` canonicalises both the root and the
candidate and compares them as paths (`src/require_loader.rs:217`), raising `Error::RequireEscape`
when the candidate is not underneath. `Path::starts_with` matches whole components rather than
characters, so a sibling directory whose name merely begins with the root's — `app-extra` beside
`app` — is not a match. The crate's own tests cover both
(`src/require_loader.rs:328`, `src/require_loader.rs:341`).

## See also

- [policy-presets](../policy-presets/) — where `require` reports present on two rows and the
  difference between them cannot be shown.
- [architecture](../../docs/architecture.md) — why the confined `require` is built rather than
  narrowed, and what an engine keeps between evaluations.
- [sandbox](../../docs/sandbox.md) — the three questions a policy answers, at length.
- [hello-eval](../hello-eval/) — `Script::from_source`, the constructor that leaves a script with
  no root and therefore no `require`.
