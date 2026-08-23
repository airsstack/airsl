# env-and-proc

Reads the environment through a name allowlist, then hands a variable to a child process it was
granted permission to spawn.

These two share an example because they are the pair that has to agree. `env.set` writes into a
per-process overlay rather than into the host's own environment, and the only way to show that the
overlay is real is to spawn a child and let it report what it sees.

## Run

```bash
cargo run -p airsl --example env-and-proc
```

Only `sh` is ever executed — it is the one program guaranteed present on both CI runners.

## Output

```
PATH: granted, set, non-empty
AIRSL_EXAMPLE_UNSET: granted, unset -> nil
AIRSL_EXAMPLE_SECRET: env.get denied: `AIRSL_EXAMPLE_SECRET` is not granted — the allowed names are AIRSL_EXAMPLE_PASSED, AIRSL_EXAMPLE_UNSET, PATH
AIRSL_EXAMPLE_PASSED: set in the overlay
env.all() sees: AIRSL_EXAMPLE_PASSED, PATH
child reported: from-lua (status 0)
deliberate failure: status 3, raised nothing
which sh resolved: true
curl: proc.run denied: `curl` is not granted — the allowed executables are sh
host environment still does not define AIRSL_EXAMPLE_PASSED
```

`PATH`'s value is never printed, only whether it is non-empty, and `which` reports only that it
resolved: `sh` is `/bin/sh` on macOS and `/usr/bin/sh` on a usrmerge Linux.

## What it demonstrates

- **An ungranted name raises; a granted-but-unset name returns `nil`.**
  `src/modules/env.rs:153` states the reason: a script that cannot tell those apart cannot tell
  "you may not ask" from "it is not set", and would report a missing configuration when it had
  actually been denied. The refusal itself is built at `src/modules/env.rs:111` and lists the
  allowed names.
- **`env.all()` returns only the granted names**, and only those actually set —
  `src/modules/env.rs:161`. Never everything the host process inherited. Note that `PATH` appears
  and `AIRSL_EXAMPLE_UNSET` does not, though both are granted.
- **`env.set` writes an overlay, not the host's environment.** `env.get`, `env.all` and `proc.run`
  all consult it, so the child sees `from-lua` — while the final line of the run asserts from Rust
  that the host process still has no such variable. Partly this is because `std::env::set_var` is
  `unsafe` in Edition 2024 and this crate forbids `unsafe`; mostly it is because a sandboxed script
  silently changing the host's environment is not a capability anyone meant to grant.
- **`proc.run` takes an argv array and has no string form** — `src/modules/proc.rs:67`. There is no
  shell, so there is no word splitting and no quoting bug available to write. `io.popen`, which
  takes a shell string, is withheld below the `full` surface for the same reason.
- **A non-zero exit status is a result, not an error.** `src/modules/proc.rs:131` keeps `status` a
  number even for a signalled process (reporting `-1`), so `result.status ~= 0` stays the one way
  to ask whether it worked. The `exit 3` above raises nothing.
- **The executable allowlist matches the program name as written.** `ProcGrant::allow`
  (`src/sandbox/grants.rs:193`) permitting `sh` does not permit `/bin/sh`, and the refusal
  (`src/modules/proc.rs:48`) lists what was allowed.

## See also

- [sandbox](../../docs/sandbox.md) — what a grant is and where it is enforced.
- [`filesystem-grants`](../filesystem-grants/) — the third axis, and the containment rule.
- [`denials-are-data`](../denials-are-data/) — what a module does when it has no grant at all.
