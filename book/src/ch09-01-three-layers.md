# Three Layers

`airsl` is built from three layers, and the most useful thing to know about them is that they are
separate. Each answers a different question, and each can be changed without the others.

```
  ┌─────────────────────────────────────────────────────────┐
  │ 3. Modules     json, fs, proc, …  and your own          │  what tools exist
  ├─────────────────────────────────────────────────────────┤
  │ 2. Policy      language · grants · limits               │  what is allowed
  ├─────────────────────────────────────────────────────────┤
  │ 1. The VM      Lua 5.4, compiled in                     │  what runs the code
  └─────────────────────────────────────────────────────────┘
```

## 1. The VM

`airsl` runs Lua 5.4 through the [`mlua`](https://crates.io/crates/mlua) crate. Lua's C source is
compiled during the build and linked straight into the binary, which is why the build needs a C
compiler and the result needs nothing at all.

It is Lua 5.4 rather than an older version or LuaJIT for one reason: from 5.3 onwards, Lua knows the
difference between the integer `3` and the float `3.0`. Older versions treat them as the same
number. Without that difference, JSON output could not be the same byte for byte from one run to the
next.

## 2. Policy

A policy is three separate answers:

| Part | Answers | Set with |
|---|---|---|
| language | which parts of Lua's own library the script sees | the preset |
| grants | what the `airsstack` modules may touch | `--allow-…`, `GrantSet` |
| limits | how much memory and time the script may spend | `--memory-limit`, `ResourceLimits` |

Keeping them apart is what lets you say things like "all of Lua, but only 8 MB of memory", or "no
limits, but no grants either". If they were one setting, most combinations could not be expressed.

The limits are armed before any module is installed, so there is no moment, however brief, when an
engine exists without them.

## 3. Modules

Each module is a set of Rust functions placed into a Lua table under the root table, `airsstack` by
default. The built-ins and your own modules are installed the same way, through the `HostModule`
trait.

Each module receives the engine's policy when it is installed. That single shared policy is why
`airsl doctor`, `engine.policy()` and what the modules actually enforce can never disagree: they are
all reading the same thing.

## Engines and evaluations

An engine runs one script at a time. It holds a lock for the whole of each evaluation, not just for
each step, so two threads sharing an engine can never mix up each other's arguments.

Each evaluation resets three things before the script runs: the instruction count, the `arg` table,
and the folder `require` looks in. Everything else in the Lua state carries over, as
[chapter 6](ch06-04-reuse.md) described.

## Extensions on top

The extension system is not a fourth layer. It sits on top of the three: it reads a manifest, works
out a policy from it, and then builds an ordinary engine with that policy. Once an extension is
running, it is just an engine like any other.
