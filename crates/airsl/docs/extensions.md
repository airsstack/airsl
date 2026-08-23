# Extension system

**Status: partly implemented.** The dispatcher exists — `airsstack.ext` (`src/modules/ext.rs`) and
`Engine::dispatch` (`src/engine.rs`), see the status table at the end of this document. The
manifest parser exists (`src/extension/manifest.rs:189`), and so do the ceiling
(`src/extension/ceiling.rs:18`, `Ceiling::new`), negotiation (`src/extension/negotiate.rs:274`,
`negotiate`) and the approver (`src/extension/approver.rs:68`, the `Approver` trait plus
`ManifestApprover`/`DenyAll`). What is still missing is the loader that ties them together —
`ExtensionHost`, `ExtensionHost::load`, and the `Extension` handle the sketch below shows — so a
host cannot yet call one function and get a running, negotiated extension back.
See [architecture.md](architecture.md).

An extension is third-party code that runs inside a host program with capabilities it *requested* and
the host *granted*. That negotiation is the whole difference between an extension system and a
plugin directory.

## The shape of an extension

```
journal-indexer/
  extension.toml
  main.lua
  lib/
    index.lua
    frontmatter.lua
```

```toml
[extension]
name    = "journal-indexer"
version = "0.2.0"
entry   = "main.lua"
api     = 1                      # airsl extension API version

[capabilities]
fs.read   = ["$APP_HOME/journal"]
fs.write  = ["$APP_HOME/journal/.index"]
proc.run  = ["git"]
env.read  = ["APP_HOME", "HOME"]
regex     = true                 # no parameters — pure computation

[capabilities.optional]
proc.run  = ["tar"]              # absence is not fatal

[limits]
memory       = "64MB"
instructions = 50_000_000
```

Three properties of the manifest carry weight:

**A request is a maximum the host may grant, never an entitlement.** The host intersects the request
with its own ceiling. A manifest asking for `fs.read = ["/"]` is not an error; it simply will not be
granted that.

**Variables are expanded by the host, from the host's environment.** If an extension could expand
`$APP_HOME` itself, it could set that variable and widen its own grant. Expansion happens
before the intersection, on the host side, always.

**`api` is declared,** so the contract can evolve without breaking installed extensions.

## Negotiation

Three outcomes: granted, reduced, denied. The decision worth making deliberately is what a
*reduction* does.

**Fail closed by default.** If a required capability is not granted, the extension does not load.
Anything it can genuinely live without goes under `[capabilities.optional]`. Two reasons: an
extension then knows its full authority at startup and needs no defensive branching at every call
site; and a silently-degraded extension — one that looks like it is working and is quietly doing
half its job — is the worst available failure mode.

An extension can still introspect what it received:

```lua
local granted = airsstack.ext.granted()
if granted.proc and granted.proc.run["tar"] then
  -- the optional capability came through
end
```

## The host API

The pieces below the loader are implemented and tested today; `ExtensionHost` is the proposed shape
that will call them in sequence and is not yet built.

```rust
// Implemented: Ceiling::new (src/extension/ceiling.rs:27) refuses a policy that would not bound
// anything — a full language surface or unrestricted grants.
let ceiling = Ceiling::new(
    Policy::confined().with_grants(
        GrantSet::declared()
            .with_fs(|fs| fs.read(home).write(home_index))
            .with_proc(|proc| proc.allow(["git", "tar"])),
    ),
)?;

// Implemented: Manifest::from_dir (src/extension/manifest.rs:189) and negotiate
// (src/extension/negotiate.rs:274) intersect the request with the ceiling.
let manifest = Manifest::from_dir(extension_dir, &variables)?;
let negotiation = negotiate(&manifest, &ceiling, &stdlib()?);

// Implemented: the Approver trait (src/extension/approver.rs:68) with two shipped
// implementations — ManifestApprover honours a satisfied negotiation, DenyAll refuses everything.
let decision = ManifestApprover.decide(&ApprovalRequest::new(extension_dir, &manifest, &negotiation));

// Proposed, not yet built: the loader that turns an approved negotiation into a running engine.
let host = ExtensionHost::builder().ceiling(ceiling).approver(ManifestApprover).build()?;
let ext = host.load(extension_dir)?;
println!("{:?}", ext.granted());
ext.call("on_session_start", payload)?;
```

The **ceiling** is what makes manifest-driven requests safe to honour at all: it is the host
program's own statement of maximum authority, and nothing a manifest says can exceed it. The
`Approver` trait then decides policy within that bound — a host writes its own implementation for
anything `ManifestApprover` and `DenyAll` do not cover, such as prompting a person interactively.

The natural split follows provenance, and a host that already distributes extensions through a
registry has the distinction to hand: registry-installed extensions get `ManifestApprover`,
locally-developed ones get a host-written interactive `Approver`.

## The Lua side

```lua
-- main.lua
local ext = airsstack.ext

ext.on("session_start", function(payload)
  local root  = airsstack.path.join(airsstack.env.get("APP_HOME"), "journal")
  local notes = airsstack.glob.walk(root, "**/*.md")
  local index = require("lib.index").build(notes)

  airsstack.fs.atomic_write(
    airsstack.path.join(root, ".index", "index.json"),
    airsstack.json.encode_pretty(index)   -- keys always sort
  )

  return { additionalContext = require("lib.index").card(index) }
end)

return ext
```

`require("lib.index")` resolves only under the extension root. That confinement ships: a target
cannot spell a path separator or a `..` component, the resolved path is canonicalised and checked
for containment, and a cycle raises rather than recursing. Multi-file extensions are no longer
blocked on it.

## Two extension shapes

**Script extensions** run to completion and produce output. This is what `airsl run` does today, and
what every script running on this runtime is.

**Registered extensions** load once, register handlers, and are called repeatedly by the host as
events occur. This is the Redis model and what "extension system" normally means. The pieces it
runs on are implemented — registration (`airsstack.ext.on`, `src/modules/ext.rs:104`), dispatch
(`Engine::dispatch`, `src/engine.rs:353`), and a **persistent engine across calls** (below) — but
nothing yet turns a manifest into one: that is the loader, `ExtensionHost`, still proposed.

The persistent engine is where the measurements matter: 4.6 µs per call on a reused engine against
136 µs constructing a fresh one. The gap widened as the standard library grew — a fresh engine now
installs twelve modules — so the case for a persistent engine is stronger than when it was first
made. A registered extension pays setup
once and then dispatches in microseconds. Engine reuse is now correct in the places it would
otherwise have been wrong — the instruction counter and the `arg` table are per evaluation,
`require` re-points at the current script's root, its module cache persists as `package.loaded`
does, and evaluations are serialised so threads sharing an engine cannot set each other's arguments.
A long-lived engine still accumulates global state between invocations, and `mlua`'s one-call
environment restore is Luau-only (`mlua-0.12.0/src/state.rs:673`), so isolation between successive
dispatches has to be built.

## What exists versus what is new

| Piece | State |
|---|---|
| `HostModule`, `ModuleSet`, `InstallContext`, `Engine` | implemented — verified from a downstream crate |
| Per-engine root table, so an extension does not land in someone else's namespace | implemented |
| Confined `require` | implemented |
| Resource ceilings, the `[limits]` block's counterpart | implemented |
| `Policy` composing all three axes | implemented |
| Parameterised grants — `FsGrant`, `EnvGrant`, `ProcGrant` | implemented |
| The host standard library a manifest names capabilities from | implemented |
| Manifest format and parser | implemented — `src/extension/manifest.rs:189` (`Manifest::from_dir`) |
| `Ceiling` | implemented — `src/extension/ceiling.rs:18` |
| Negotiation (`negotiate`, `Negotiation`, `Reduction`, `Denial`) | implemented — `src/extension/negotiate.rs:274` |
| `Approver`, `ManifestApprover`, `DenyAll` | implemented — `src/extension/approver.rs:68,75,95` |
| `ext.on` registration and host dispatcher | implemented — `src/modules/ext.rs:104` (`on`), `src/engine.rs:353` (`dispatch`) |
| Capability introspection (`ext.granted`) | implemented — `src/modules/ext.rs:116` (`granted`) |
| `ExtensionHost` — the loader tying the above together | proposed |

## Sequencing, and one caution

**The extension host should come after the standard library, not before.** The grant vocabulary *is*
the module list — a manifest cannot say `fs.read = [...]` before `fs` exists — so building the host
first would have meant designing grants for capabilities with no implementation to constrain them.
That ordering is now satisfied: every capability a manifest can name exists, and the grant types a
manifest would parse into are the ones the modules already enforce.

**Versioning needs a decision before the first third-party extension ships.** An extension pins
`api = 1`; modules will grow functions and occasionally change semantics. Whether the guarantee is
"additive only within an api version" or something looser constrains every module signature from
here onward, and it is very hard to tighten afterwards.

## Open questions

- Whether grants are revocable at runtime, or fixed for an engine's lifetime. Fixed is far simpler
  and probably right.
- How a registered extension reports failure without taking down the dispatcher, and whether a
  repeatedly-failing extension gets disabled automatically.

## See also

- [architecture.md](architecture.md) — the seam this is built on and the gaps in it.
- [sandbox.md](sandbox.md) — the policy model a manifest negotiates against.
- [stdlib.md](stdlib.md) — the capabilities a manifest can name.
