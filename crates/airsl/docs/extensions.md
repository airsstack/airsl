# Extension system

**Status: implemented.** The dispatcher exists — `airsstack.ext` (`src/modules/ext.rs`) and
`Engine::dispatch` (`src/engine.rs`), see the status table at the end of this document. The manifest
parser exists (`src/extension/manifest.rs:189`), and so do the ceiling (`src/extension/ceiling.rs:27`,
`Ceiling::new`), negotiation (`src/extension/negotiate.rs:274`, `negotiate`) and the approver
(`src/extension/approver.rs:68`, the `Approver` trait plus `ManifestApprover`/`DenyAll`). The loader
that ties them together is built too: `ExtensionHost::load` (`src/extension/host.rs:251`) drives
`Extension::approve` (`src/extension/loaded.rs:175`) followed by `Approved::start`
(`src/extension/loaded.rs:96`), so a host calls one function and gets a running, negotiated
extension back. The CLI is built on the same loader: `airsl ext doctor`
(`crates/airsl-cli/src/ext_doctor.rs:198`, `run`) reports a negotiation without running the entry
script, and `airsl ext fire` (`crates/airsl-cli/src/ext_fire.rs:91`, `run`) loads and dispatches one
event through `ExtensionHost` exactly as a host program would.
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

Every piece below is implemented and tested today, `ExtensionHost` included
(`src/extension/host.rs:208`).

```rust
// Ceiling::new (src/extension/ceiling.rs:27) refuses a policy that would not bound anything — a
// full language surface or unrestricted grants.
let ceiling = Ceiling::new(
    Policy::confined().with_grants(
        GrantSet::declared()
            .with_fs(|fs| fs.read(home).write(home_index))
            .with_proc(|proc| proc.allow(["git", "tar"])),
    ),
)?;

// ExtensionHost::builder (src/extension/host.rs:222) is a type-state builder — there is no
// `build()` until `ceiling()` has been called. `.modules(factory)` replaces the module set each
// load starts from; it takes a `Fn() -> Result<ModuleSet>` because ModuleSet holds
// `Box<dyn HostModule>` and cannot be cloned, so a factory is drawn from fresh per load rather
// than passed as one owned value. Left unset it defaults to `crate::modules::stdlib`.
let mut host = ExtensionHost::builder()
    .ceiling(ceiling)
    .events(["session_start"])?
    .approver(ManifestApprover)
    .build()?;

// ExtensionHost::load (src/extension/host.rs:251) runs Manifest::from_dir
// (src/extension/manifest.rs:189), negotiate (src/extension/negotiate.rs:274), the approver, and
// Approved::start (src/extension/loaded.rs:96), which builds the engine and evaluates the entry
// script — in that order, and returns the running Extension.
let ext = host.load(extension_dir)?;
println!("{:?}", ext.granted());
ext.call(&EventName::new("session_start")?, &payload)?;
```

`ManifestApprover` and `DenyAll` are the two shipped `Approver` implementations
(`src/extension/approver.rs:68,75,95`) — the library ships no `interactive()`. Prompting a person is
a host's own `Approver` impl: the library never owns a terminal.

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
runs on are implemented — registration (`airsstack.ext.on`, `src/modules/ext.rs:89`), dispatch
(`Engine::dispatch`, `src/engine.rs:353`), and a **persistent engine across calls** (below) — and
`ExtensionHost` turns a manifest into one: `Extension` owns the engine `Approved::start`
(`src/extension/loaded.rs:96`) builds, and `ExtensionHost::broadcast`
(`src/extension/host.rs:312`) calls every loaded extension's handler in load order without
short-circuiting on a failure.

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
| `ext.on` registration and host dispatcher | implemented — `src/modules/ext.rs:89` (`on`), `src/engine.rs:353` (`dispatch`) |
| Capability introspection (`ext.granted`) | implemented — `src/modules/ext.rs:111` (`granted`) |
| `ExtensionHost`, `Extension` — the loader tying the above together | implemented — `src/extension/host.rs:251` (`ExtensionHost::load`), `src/extension/loaded.rs:175` (`Extension::approve`), `src/extension/loaded.rs:96` (`Approved::start`) |
| CLI — `airsl ext doctor` / `airsl ext fire` | implemented — `crates/airsl-cli/src/ext_doctor.rs:198` (`run`), `crates/airsl-cli/src/ext_fire.rs:91` (`run`) |

## Sequencing, versioning, and fail-closed

**The extension host should come after the standard library, not before.** The grant vocabulary *is*
the module list — a manifest cannot say `fs.read = [...]` before `fs` exists — so building the host
first would have meant designing grants for capabilities with no implementation to constrain them.
That ordering is now satisfied: every capability a manifest can name exists, and the grant types a
manifest would parse into are the ones the modules already enforce.

**Versioning rule.** Within `api = 1`, a module or event may only gain functions or names — an
existing signature never narrows, and an existing name never changes meaning. A change that is not
additive bumps `SUPPORTED_API` (`src/extension/api_version.rs:20`); an extension whose manifest
names an `api` outside that set is refused before any code runs — `ApiVersion::supported`
(`src/extension/api_version.rs:34`) returns `Error::UnsupportedApi` naming the supported set. This
is the guarantee an installed extension gets: nothing it already relies on stops working under the
api number it pinned.

**The fail-closed invariant.** A required capability outside the ceiling is refused before the
approver is ever asked, and the approver is asked before any engine exists —
`Extension::approve` (`src/extension/loaded.rs:175`) runs the negotiation check first and returns
`Error::ExtensionDenied` on the first denial it finds, so an approver can only narrow what a
ceiling already bounds, never widen it, and no entry script executes until both checks have passed.

## Resolved questions

**Grants are fixed for an engine's lifetime, not revocable at runtime.** Every module `Arc`-clones
the `GrantSet` at install; changing what an extension may reach means reloading it.

**Auto-disabling a repeatedly-failing extension is the host's decision, not the library's.**
`ExtensionHost::broadcast` (`src/extension/host.rs:312`) isolates one extension's failure into its
own `Dispatch` rather than stopping the rest, and hands every outcome to the caller — deciding
whether three failures in a row means "stop calling this one" is a policy `ExtensionHost` does not
impose.

Nothing about the negotiation-and-load design is left open, and the CLI built on top of it
(`airsl ext doctor`/`airsl ext fire`) closes the loop: every piece the spec named now has a caller.

## See also

- [architecture.md](architecture.md) — the seam this is built on and the gaps in it.
- [sandbox.md](sandbox.md) — the policy model a manifest negotiates against.
- [extension-host example](../examples/extension-host/) — `ExtensionHost::load_dir` and
  `broadcast` end to end, loading a directory of extensions under one ceiling.
- [stdlib.md](stdlib.md) — the capabilities a manifest can name.
