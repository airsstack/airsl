# failure-policy

Whether a caller surfaces a script's failure or discards it — and the one case that overrides the
choice.

`FailurePolicy` is the only part of this crate that describes the **caller's** behaviour rather than
the script's permissions, and the two are read at different times: the engine reads a `Policy` while
building a state, this is read after evaluation has already returned. Giving it its own example is
what makes that distinction land, rather than leaving it as a footnote to the sandbox.

## Run

```bash
cargo run -p airsl --example failure-policy
```

Both scripts are safe to run directly, which is the point of `counts.lua` being an ordinary
terminating loop rather than a runaway:

```bash
airsl run crates/airsl/examples/failure-policy/counts.lua
airsl run --fail-open crates/airsl/examples/failure-policy/raises.lua
```

## Output

```
-- an ordinary failure: the script raised
Report: surfaced=true exit=1 breach=false
  lua error in raises.lua: runtime error: raises.lua:9: enforce: the tool call is not permitted here
stack traceback:
	[C]: in ?
	[C]: in function 'error'
	raises.lua:9: in main chunk
FailOpen: surfaced=false exit=0 breach=false

-- a resource breach: the same policies, a different kind of failure
Report: surfaced=true exit=1 breach=true
  script `counts.lua` exceeded its instruction limit of 100000
FailOpen: surfaced=true exit=0 breach=true
  script `counts.lua` exceeded its instruction limit of 100000

-- and with a budget that fits, there is no failure to react to
counts.lua returned 20000100000
```

## What it demonstrates

- **The two variants are `Report` and `FailOpen`** (`src/failure_policy.rs:30` and
  `src/failure_policy.rs:37`) — there is no `FailClosed`. `Report` is the default and is correct for
  anything a person invoked directly, because a failure that produces no output and no message is
  worse than a loud one.
- **`FailOpen` exists for a specific hazard.** A `PreToolUse` hook that exits non-zero blocks the
  tool call that triggered it, and such matchers commonly cover `Read` — so a propagated failure can
  block every file read in a session. The module doc at `src/failure_policy.rs:1` is the long form.
- **The breach test is `||`, not `&&`.** The reaction in `describe` is
  `!policy.swallows_errors() || breached`. A caller that discards ordinary script failures usually
  still wants to hear that the host's CPU or memory was consumed, because that is a fact about the
  machine rather than a diagnostic the script chose to emit. The fourth line of output is the whole
  example: `FailOpen`, `surfaced=true`, `exit=0`.
- **Surfacing and exiting are separate decisions.** `exit_code()` (`src/failure_policy.rs:49`) still
  returns `0` for `FailOpen` on that breach. Only the exit status can block a tool call; printing a
  diagnostic cannot.
- **`exhausted_limit()` is what makes the distinction available** (`src/error.rs:278`). Without it a
  caller would have to match on message text, and a script could then disguise its own failure as a
  resource breach.
- **The chunk name is chosen, not inherited.** `Script::from_file` takes the chunk name from the
  path as given (`src/script.rs:63`), and the path this example builds from `CARGO_MANIFEST_DIR` is
  absolute — so inheriting it would put the developer's home directory into every diagnostic above.
  For a hook whose stderr lands in someone else's log that is a real leak, so the example overrides
  the label with `Script::with_name` (`src/script.rs:118`). It changes nothing else: the source, the
  arguments and the root the file was read from all survive, so a renamed script still `require`s
  what it could before.

`counts.lua` is deliberately not a runaway. It is a correct, terminating script that becomes a
breach only because the host handed it a 100,000-instruction budget — which keeps the file safe to
run on its own. The genuinely malicious chunks live inline in
[`resource-limits`](../resource-limits/) for exactly that reason.

## See also

- [`resource-limits`](../resource-limits/) — where the breach in the middle of this output comes
  from.
- [how-to](../../docs/how-to.md) — "React to a failure without propagating it", the pattern
  `describe` is built from.
