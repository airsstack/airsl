# agent-hook

An agent hook end to end: take the JSON payload on stdin, decide, and write the envelope the agent
host expects on stdout.

This gets its own example because the hook contract is the one place in this crate where the *bytes
on stdout* are the interface. Everywhere else a script's output is for a person to read; here it is
parsed by the process that invoked the hook, so a stray newline, a hand-rolled nesting or a
well-meant progress line is a bug rather than a blemish. That is a shape rather than a function
call, which is why it does not fit in a doc comment.

## Run

```bash
cargo run -p airsl --example agent-hook
```

And the way a real hook is actually invoked — a JSON document on stdin:

```bash
cargo run -q -p airsl --example agent-hook < crates/airsl/examples/agent-hook/payload.json
```

Both print the same bytes, and so does `< /dev/null`. That is arranged, not luck; see
[The stdin problem](#the-stdin-problem) below.

## Output

```
-- payload: event=PreToolUse tool=Write file=hooks/enforce.lua
-- hook.context: the envelope, nesting included
{"hookSpecificOutput":{"additionalContext":"This file is an airsl hook: it runs under a policy, and its authority comes from the invocation.","hookEventName":"PreToolUse"}}
-- hook.emit: any document, and exactly the bytes handed to it
{"reviewed":"hooks/enforce.lua","suppressOutput":false}
```

## The stdin problem

`hook.payload()` reads **this process's own stdin** (`src/modules/hook.rs:98`), and there is no seam
to inject anything else through. A naive example would therefore block forever when a developer runs
it in a terminal, waiting for a person to type a JSON document by hand. So the script asks first:

```lua
local payload = {}
if not airsstack.stdio.isatty("stdin") then
  payload = airsstack.hook.payload()
end
if next(payload) == nil then
  payload = airsstack.json.decode(airsstack.fs.read(airsstack.path.join(here, "payload.json")))
end
```

Three invocations, one result:

- **On a terminal**, stdin is a tty, so the script never reads it and never blocks.
- **Under CI**, where `cargo make examples` runs every example with inherited stdin, stdin is not a
  tty but is already at end of stream — so `hook.payload()` returns an empty table rather than
  raising.
- Both fall through to the bundled `payload.json`, so **the output is byte-identical in both**,
  which is what lets this example have a real, reproducible `## Output` block at all.
- **Piping that same file in** takes the `hook.payload()` path instead, and prints the same bytes a
  third time, because it is the same document. That is the path a real hook takes.

The fallback exists only so the example is self-running. A real hook calls `hook.payload()` and
nothing else, because the host always has a document to hand it — this is the one place the example
deviates from what you would ship.

## What it demonstrates

- **`hook.payload()` reads and decodes in one call** (`src/modules/hook.rs:98`), because every
  hook's first two lines were the same pair and the failure mode of getting them wrong is a hook
  that silently does nothing.
- **An empty stdin yields an empty table, not an error** (`src/modules/hook.rs:90`). No payload is
  not a parse failure: a hook invoked by hand, or one whose event carries nothing, should see `{}`
  rather than an error it has to guard every call site against. `next(payload) == nil` is how the
  script tells that apart from a real document.
- **`hook.context(event, additional)` writes the nesting once, in Rust**
  (`src/modules/hook.rs:119`, built by `context_envelope` at `src/modules/hook.rs:129`), so no
  script has to remember that `hookEventName` and `additionalContext` live *inside*
  `hookSpecificOutput`. `hookEventName` is an argument rather than a guess, because the contract
  requires it and there is no sensible default.
- **The envelope deliberately carries no `permissionDecision` field**
  (`src/modules/hook.rs:13`), and a test holds that line:
  `the_context_envelope_carries_no_permission_decision_field` (`src/modules/hook.rs:229`). The
  reason is behavioural, not stylistic, and it was watched directly against the agent CLI installed
  on the machine that built the module rather than read out of a specification — no version is
  claimed, and the exact conditions could shift release to release. What was watched: a hook
  returning `permissionDecision: defer` was watched
  having the tool call it fired on swallowed outright — no `tool_result` at all — when the session
  is non-interactive, the tool batch is solo, and the abort signal is not already set. That strands
  the caller with no record the call ever happened. `additionalContext` alone was confirmed to carry
  no such risk: the note reaches the model's turn and the tool call still returns normally. The test
  goes through the real builder rather than a literal, so a decision field added to the
  implementation turns it red instead of leaving it green against its own fixture.
- **`hook.emit(table)` writes an arbitrary JSON document** (`src/modules/hook.rs:106`, encoding at
  `src/modules/hook.rs:102`) with no envelope shape imposed — the escape hatch for anything
  `context` does not model. Keys come out sorted, so `reviewed` precedes `suppressOutput` whatever
  order the Lua table was built in.
- **No trailing newline is added, by either of them** (`src/modules/stdio.rs:98`), because a hook's
  output is a single JSON document read by a parser that cares what the bytes are. The newline that
  ends each of the two documents above is written by the script itself, so that this example can
  print two of them on separate lines; a real hook writes one and adds nothing to it.
- **`stdio.isatty("stdin")` is the only way a script can tell a reader from a parser**
  (`src/modules/stdio.rs:140`) — the difference between printing a progress line and corrupting a
  JSON document. This example is the only place in the ladder that uses it.
- **Only the exit status can block a tool call; printing a diagnostic cannot.** A `PreToolUse` hook
  that exits non-zero blocks the tool call that triggered it, and such matchers commonly cover
  `Read` — so a propagated failure can block every file read in a session. That is why
  `FailurePolicy::FailOpen` exists (`src/failure_policy.rs:37`) and why its `exit_code()` is `0`
  (`src/failure_policy.rs:49`). The script here never exits and never can: deciding the status is
  the host's job, not the script's (`src/modules/hook.rs:31`).
- **The chunk name is chosen, not inherited.** The host builds an absolute path from
  `CARGO_MANIFEST_DIR`, so `Script::with_name` (`src/script.rs:118`) relabels the chunk to
  `hook.lua` — a raised error would otherwise carry the developer's home directory into the
  traceback, and a hook's stderr routinely lands in someone else's log.
- **The read root travels as an argument.** The host grants a read root and passes its path to the
  script with `with_args`, the same way [`filesystem-grants`](../filesystem-grants/) does, so the
  Lua holds no path that is true only on one machine.

The script prints through `airsstack.stdio.write` and never through Lua's own `print`. Not because
the two would reorder — `print` flushes after every line and so does `stdio.write`
(`src/modules/stdio.rs:109`), so they interleave correctly even through a pipe — but because `print`
appends a newline and cannot be told not to. On the one stream where the bytes are the interface, a
writer that adds a byte of its own is the wrong tool no matter what it is asked to write.

`payload.json` is a synthetic `PreToolUse` payload: a placeholder session id, a repo-relative
`file_path`, and nothing else. Nothing in it is a real path or a real user's data.

## See also

- [tutorial](../../docs/tutorial.md) — step 6 builds a hook from the CLI side, with `airsl run` and
  a shell pipe instead of an embedded engine.
- [`failure-policy`](../failure-policy/) — the decision this example refuses to take: whether a
  failed hook exits non-zero.
- [`values-and-json`](../values-and-json/) — why the keys in both documents above come out sorted.
- [stdlib](../../docs/stdlib.md) — the full `airsstack.hook` and `airsstack.stdio` roster.
