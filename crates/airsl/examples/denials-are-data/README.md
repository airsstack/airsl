# denials-are-data

Asks for six things the policy does not grant, and prints what it is told.

This is its own example because the alternative design is the tempting one. Omitting a module a
script may not use would make `if airsstack.fs then` mean *"am I allowed to read files"* instead of
*"does this runtime have a filesystem module"* — two questions needing different answers. A script
written against the first cannot be moved to a different policy without editing it.

## Run

```bash
cargo run -p airsl --example denials-are-data
```

The policy carries one narrow grant of each kind, so every refusal has something to name. A policy
granting nothing at all would print *"none are granted"* four times and show nothing about how a
refusal helps.

## Output

```
modules installed: json, path, fs, env, proc, regex, hash, time, glob, stdio, hook
path.join needs nothing: a/b/c.txt
json.encode needs nothing: {"ok":true}
granted read works: 17 bytes

fs.read, outside the granted root
  fs.read denied: `/` is outside the granted read roots: <granted>
fs.write, into the read-only root
  fs.write denied: `<granted>/new.txt` is outside the granted write roots: none are granted
hash.hash_file, outside the granted root
  hash.hash_file denied: `/` is outside the granted read roots: <granted>
glob.walk, outside the granted root
  glob.walk denied: `/` is outside the granted read roots: <granted>
env.get, a name that is not on the allowlist
  env.get denied: `AIRSL_EXAMPLE_SECRET` is not granted — the allowed names are PATH
proc.run, an executable that is not on the allowlist
  proc.run denied: `curl` is not granted — the allowed executables are sh

every refusal named the grant it was measured against
```

`/` is the denied path in three of the six because it exists on every unix and resolves to itself,
which keeps the message identical on Linux and macOS. `<granted>` is substituted by `denials.lua`.

## What it demonstrates

- **A module is always present, even ungranted.** All eleven are asserted to be tables under a
  policy that grants almost none of what they do. The roster comes from `modules::stdlib()`
  (`src/modules/stdlib.rs:20`), which is the single list the engine, `airsl doctor` and the tests
  all read.
- **Enforcement is per call, not per installation.** `HostModule::install` receives the policy
  through `InstallContext` (`src/modules/registry.rs:33`) and captures what it needs into the
  functions it creates — the check happens inside the host function, immediately before the
  operation. That is why `fs` can be present, refuse `/`, and still serve a granted read in the
  same run.
- **Some modules need no authority at all.** `path` and `json` behave identically under every
  policy. `path` needs nothing and `fs` does, so they are separate modules rather than one
  convenient namespace — every module is a capability.
- **Authority is borrowed, not invented.** `hash.hash_file` opens a file and `glob.walk` reads
  directories, so both answer to the *filesystem read* grant rather than getting an axis of their
  own. They share `PathGuard` with `fs` — `src/modules/hash.rs:73` and `src/modules/glob.rs:89`,
  against `src/modules/fs.rs:78`. `glob.match`, which is pure pattern arithmetic, needs nothing.
- **Every refusal is actionable.** Each message names the module, the operation, what was refused,
  and what *was* granted — `Error::Denied` (`src/error.rs:131`). "Permission denied" without those
  is indistinguishable from the operating system's own refusal, and sends whoever reads it looking
  at file modes instead of at the policy.

## See also

- [sandbox](../../docs/sandbox.md) — what a grant is and where it is enforced.
- [host standard library](../../docs/stdlib.md) — the full roster and the rules every module follows.
- [`filesystem-grants`](../filesystem-grants/) and [`env-and-proc`](../env-and-proc/) — what the
  grants permit, rather than what their edges refuse.
