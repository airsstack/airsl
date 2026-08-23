# resource-limits

Arms both ceilings, trips each one, and shows that the resulting error says *which* was breached.

This is the second example that keeps its Lua inline, and the only one that does so for a reason
other than brevity: the chunks are deliberately malicious. A shipped `.lua` file containing
`while true do end` would be a footgun for anyone pointing `airsl run` — or even `airsl check`,
which is safe but invites the reader to try the other — at this directory. The two runaway chunks
live as `const`s that only this example can reach.

## Run

```bash
cargo run -p airsl --example resource-limits
```

## Output

```
instruction ceiling: script `runaway-loop` exceeded its instruction limit of 100000
  classified as: Some(Instructions)
memory ceiling: script `runaway-alloc` exceeded its memory limit of 8388608 bytes
  classified as: Some(Memory)
no ceilings: the bounded loop counted to 200000
tight ceiling: the same bounded loop is stopped: true
```

Both figures in that output are the ceilings this example set, not measurements, so the run is
byte-identical every time and on every platform.

## What it demonstrates

- **A resource breach is not a script failure.** `Error::MemoryLimit` and `Error::InstructionLimit`
  are variants of their own rather than an `Error::Lua` carrying a message
  (`src/error.rs:49` and `src/error.rs:64`), because a script stopped for consuming the host's
  memory is an operational event, not a defect in the script.
- **Classification is structural, never textual.** `Engine::classify` (`src/engine.rs:251`)
  consults the engine's own instruction counter and walks the VM error chain for
  `mlua::Error::MemoryError`. It never matches on message text — a script is free to raise a string
  that reads exactly like either report, and a text match would let it disguise its own failure as
  a resource breach, or the reverse.
- **`exhausted_limit()` is the caller's question** (`src/error.rs:224`). It returns
  `Option<ExhaustedLimit>` (`src/error.rs:202`), which is how the `failure-policy` example decides
  to surface a breach even while discarding ordinary script errors.
- **The ceiling is enforced to within a check interval.** The VM hook fires every 10,000
  instructions (`src/instruction_budget.rs:34`), so a ceiling is approximate by that much. Precision
  finer than this would be paid for by every script that runs, since the hook sits on the VM's hot
  path.
- **Order matters when both ceilings could trip.** `classify` tests the instruction budget first, so
  a script that would breach both is reported as an instruction breach. The memory case here arms
  only the memory ceiling, which is what makes its answer unambiguous.
- **`ResourceLimits::none()` lifts them** (`src/sandbox/resource_limits.rs:101`) — what
  `Policy::trusted()` carries. Right for first-party code, wrong for anything else: without the
  instruction ceiling nothing defends against a script that never finishes, and the crate offers no
  `sleep` and no timeout to substitute for one.

The last two lines run the *same* bounded loop twice, unbounded and then under the tight ceiling, to
make the point that a ceiling is a budget rather than a judgement. Nothing is wrong with that loop;
it was given less than it needed.

## See also

- [`failure-policy`](../failure-policy/) — what a caller does with the error this produces.
- [sandbox](../../docs/sandbox.md) — ceilings as one of the three independent policy questions.
