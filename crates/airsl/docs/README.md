# airsl documentation

`airsl` embeds Lua 5.4 into a Rust program and decides, from Rust, exactly what those scripts may
reach. It is not a Lua interpreter that happens to be written in Rust — the point is the boundary:
the host owns the capability surface, and a script gets only what the host granted it.

Two things drive the design. The near-term one is done: the airsstack plugin suite's mix of
Python, Node and POSIX sh is gone, replaced by Lua on this runtime, with 244 tests over it in CI. The longer-term one is serving as the **extension
system** for airsstack — third-party code, running with declared and negotiated capabilities. Redis
is the precedent: scripts are useful precisely because the server, not the script, decides what the
script can touch.

## Status: read this first

The runtime foundation ships, and so does the whole host standard library. The airsstack plugin
suite — the near-term driver named below — now runs entirely on it. What remains unbuilt is the
extension host — manifests, ceilings, approval and dispatch — and Tier 3. Every document marks
each piece, and the table below is the summary.

| Area | State |
|---|---|
| Embedded Lua 5.4 VM, statically linked | **implemented** |
| `Engine`, type-state builder, `Script`, `FailurePolicy` | **implemented** |
| `HostModule` extension seam — `Send + Sync`, `mlua` re-exported, per-engine root table, policy passed to `install` | **implemented** |
| Policy — language surface, three presets, memory and instruction ceilings | **implemented** |
| Confined `require` | **implemented** |
| `airsstack.json` | **implemented** (sorted keys; `null` and `[]` round-trip, but neither can be constructed from Lua — see [stdlib](stdlib.md)) |
| `airsl` CLI — `run`, `test`, `check`, `doctor`, grant flags | **implemented** |
| Parameterised capability grants — `FsGrant`, `EnvGrant`, `ProcGrant` | **implemented** |
| Host standard library — `path`, `fs`, `env`, `proc`, `regex`, `hash`, `time`, `glob`, `stdio`, `hook` | **implemented** |
| `airsl test` and `airsl check` | **implemented** |
| Extension host — manifests, ceilings, approval, dispatch | **proposed** |

Do not cite these documents as evidence that something works. Where a claim is about code that
exists, it carries a `file:line`. Where it is about code that does not, it says so.

## The documents

Four modes, kept separate on purpose, following [Diátaxis](https://diataxis.fr/): a tutorial that
stops to explain becomes a bad tutorial *and* a bad explanation, so each document commits to one job.

- **[Tutorial](tutorial.md)** — start here if you have never run a script. From installing the
  binary to a working agent hook, hitting the sandbox once on purpose along the way.
- **[How-to](how-to.md)** — recipes for a specific job, from Lua and from Rust: walking a tree,
  running a program, embedding the runtime, adding your own module.
- **[Architecture](architecture.md)** — the three layers, what each owns, what ships today, and the
  structural decisions worth understanding before changing anything.
- **[Sandbox](sandbox.md)** — the capability and policy model: what a grant is, where enforcement
  lives, what the resource ceilings can and cannot promise, and the honest comparison with WASM.
- **[Host standard library](stdlib.md)** — the module roster, the reasoning behind each, and the
  design principles every module follows.
- **[Extension system](extensions.md)** — manifests, capability negotiation, the host API, and the
  two extension shapes.

## Where the reference layer lives

Tutorial, how-to and explanation are the files above. The fourth Diátaxis mode — reference — is the
rustdoc rather than a document here, generated from source and gated by
`RUSTDOCFLAGS="-D warnings"`, so it cannot drift from what it describes:

```bash
cargo doc -p airsl --no-deps --open
```

## Measurements quoted in these documents

Taken on one machine, with the benchmark target in this crate — `cargo bench -p airsl`, median of
several runs at 1000 iterations. Reproduce them before relying on them; they are a snapshot, not a
guarantee, and an earlier snapshot taken elsewhere was roughly twice as slow across the board.

| What | Ceilings armed | No ceilings |
|---|---|---|
| `Engine` construction | 128 µs | 102 µs |
| In-process eval, engine reused, trivial chunk | 4.6 µs | 3.4 µs |
| In-process eval, engine reused, 1000 host-function calls | 1073 µs | 955 µs |
| In-process eval, fresh engine per call, trivial chunk | 136 µs | 100 µs |

| What | Result |
|---|---|
| CLI, release build, trivial script | 3.2 ms per run |
| `python3 -c 'print("hi")'`, same loop | 13.8 ms per run |
| `sh -c 'echo hi'`, same loop | 2.2 ms per run |

Two things follow, and [architecture](architecture.md) develops both.

**Reuse against rebuild is thirty-fold**, which makes engine lifetime an API-shape question rather
than an optimisation. It widened as the standard library grew: construction installs eleven modules
now rather than one, and went from 40 µs to 128 µs while the reused path barely moved. The argument
the number supports got stronger, not weaker.

**The ceilings are not free.** Arming the instruction hook costs roughly a quarter of the eval path,
because the hook fires on the VM's hot path. That is the price of being able to stop a script that
never terminates, and it is opt-out per policy — but it should be a decision rather than a surprise.
For the CLI none of it signifies: a 3.2 ms process spawn dwarfs every row above, and the extra
90 µs the larger standard library costs at construction is lost inside it. `airsl` stays within
half again of a bare `sh` spawn and is four times faster than starting `python3`.

## Related

- [`crates/airsl/README.md`](../README.md) — the crate's own short introduction.
- [`crates/airsl-cli/README.md`](../../airsl-cli/README.md) — the `airsl` binary, its flags, and the
  hook-launcher pattern.
- [`CLAUDE.md`](../../../CLAUDE.md) — repository conventions, including the evidence rules these
  documents follow.
