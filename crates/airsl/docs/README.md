# airsl documentation

`airsl` embeds Lua 5.4 into a Rust program and decides, from Rust, exactly what those scripts may
reach. It is not a Lua interpreter that happens to be written in Rust — the point is the boundary:
the host owns the capability surface, and a script gets only what the host granted it.

Two things drive the design. The near-term one is replacing the airsstack plugin suite's mix of
Python, Node and POSIX sh with one runtime. The longer-term one is serving as the **extension
system** for airsstack — third-party code, running with declared and negotiated capabilities. Redis
is the precedent: scripts are useful precisely because the server, not the script, decides what the
script can touch.

## Status: read this first

The runtime foundation ships. The capability *surface* — the modules a script would actually call —
largely does not. Every document marks each piece, and the table below is the summary.

| Area | State |
|---|---|
| Embedded Lua 5.4 VM, statically linked | **implemented** |
| `Engine`, type-state builder, `Script`, `FailurePolicy` | **implemented** |
| `HostModule` extension seam — `Send + Sync`, `mlua` re-exported, per-engine root table | **implemented** |
| Policy — language surface, three presets, memory and instruction ceilings | **implemented** |
| Confined `require` | **implemented** |
| `airsstack.json` | **implemented** (two known gaps — see [stdlib](stdlib.md)) |
| `airsl` CLI — `run`, `doctor` | **implemented** |
| Parameterised capability grants | **proposed** — the axis is typed and empty until `fs` exists to constrain it |
| Host standard library — `path`, `fs`, `env`, `proc`, `regex`, `hash`, `time`, `glob`, `stdio` | **proposed** |
| Extension host — manifests, ceilings, approval, dispatch | **proposed** |
| `airsl test` | **proposed** |

Do not cite these documents as evidence that something works. Where a claim is about code that
exists, it carries a `file:line`. Where it is about code that does not, it says so.

## The documents

- **[Architecture](architecture.md)** — the three layers, what each owns, what ships today, and the
  structural decisions worth understanding before changing anything.
- **[Sandbox](sandbox.md)** — the capability and policy model: what a grant is, where enforcement
  lives, what the resource ceilings can and cannot promise, and the honest comparison with WASM.
- **[Host standard library](stdlib.md)** — the module roster, the reasoning behind each, and the
  design principles every module follows.
- **[Extension system](extensions.md)** — manifests, capability negotiation, the host API, and the
  two extension shapes.

## Why not the four Diátaxis modes yet

The sibling crate's docs (`crates/clauders/docs/`) split into tutorial, how-to, reference and
explanation. These four are all *explanation*, deliberately: a tutorial for `fs` would teach an API
nobody can call yet, and a how-to guide would be fiction. The tutorial and how-to layers land with
the standard library. The reference layer is the rustdoc, generated from source and gated by
`RUSTDOCFLAGS="-D warnings"`, so it cannot drift:

```bash
cargo doc -p airsl --no-deps --open
```

## Measurements quoted in these documents

Taken on one machine, with the benchmark target in this crate — `cargo bench -p airsl`, median of
several runs at 1000 iterations. Reproduce them before relying on them; they are a snapshot, not a
guarantee, and an earlier snapshot taken elsewhere was roughly twice as slow across the board.

| What | Ceilings armed | No ceilings |
|---|---|---|
| `Engine` construction | 40 µs | 33 µs |
| In-process eval, engine reused, trivial chunk | 4.2 µs | 3.4 µs |
| In-process eval, engine reused, 1000 host-function calls | 910 µs | 710 µs |
| In-process eval, fresh engine per call, trivial chunk | 48 µs | 42 µs |

| What | Result |
|---|---|
| CLI, release build, trivial script | 2.3 ms per run |
| `python3 -c 'print("hi")'`, same loop | 11.9 ms per run |
| `sh -c 'echo hi'`, same loop | 1.7 ms per run |

Two things follow, and [architecture](architecture.md) develops both.

**Reuse against rebuild is an order of magnitude**, which makes engine lifetime an API-shape question
rather than an optimisation.

**The ceilings are not free.** Arming the instruction hook costs roughly a quarter of the eval path,
because the hook fires on the VM's hot path. That is the price of being able to stop a script that
never terminates, and it is opt-out per policy — but it should be a decision rather than a surprise.
For the CLI none of it signifies: a 2.3 ms process spawn dwarfs every row above.

## Related

- [`crates/airsl/README.md`](../README.md) — the crate's own short introduction.
- [`crates/airsl-cli/README.md`](../../airsl-cli/README.md) — the `airsl` binary, its flags, and the
  hook-launcher pattern.
- [`CLAUDE.md`](../../../CLAUDE.md) — repository conventions, including the evidence rules these
  documents follow.
