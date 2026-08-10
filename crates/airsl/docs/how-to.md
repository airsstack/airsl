# How-to

Recipes for specific jobs. Each one assumes you already know what a grant is; if not, the
[tutorial](tutorial.md) gets you there in ten minutes.

Every snippet here was run before it was written down. Where output is shown, it is the output.

## From Lua

### Read and write files

```lua
local text = airsstack.fs.read(path)
airsstack.fs.write(path, text)
```

```bash
airsl run --allow-read . --allow-write ./out script.lua
```

A write root is **not** implicitly readable. Grant both if the script reads back what it wrote.

### Replace a file without a torn read

```lua
airsstack.fs.atomic_write(
  airsstack.path.join(root, "index.json"),
  airsstack.json.encode_pretty(index)
)
```

The staging file is created in the target's own directory and renamed over it, so a concurrent
reader sees either the old contents or the new ones and never half of each. `/tmp` is not used,
because a rename across filesystems is not atomic.

JSON object keys always sort, so the same table produces the same bytes on every run — which is what
makes an index file diffable.

### Claim something exactly once

```lua
if airsstack.fs.create_exclusive(path_to_claim) then
  -- you won; do the work
end
```

```
true
false
```

`create_exclusive` returns `false` rather than raising when the file already exists, because losing
that race is the expected *other outcome*, not a failure. This is `O_CREAT|O_EXCL`: a read-then-write
would let several concurrent callers all believe they won.

### Find files by pattern

```lua
for _, relative in ipairs(airsstack.glob.walk(root, "**/*.lua")) do
  print(relative)
end
```

```
a.lua
sub/b.lua
```

Paths come back relative to `root` and in sorted order, so output does not depend on the filesystem's
enumeration order. `**/` matches zero or more segments, so `**/Cargo.toml` finds a root-level one.

`glob.walk` reads directories, so it needs the same read grant `fs` would. `glob.match` is pure
pattern arithmetic and needs nothing.

### Run a program

```lua
local r = airsstack.proc.run({"git", "rev-parse", "--abbrev-ref", "HEAD"})
if r.status ~= 0 then
  airsstack.stdio.error("git failed: " .. r.stderr)
else
  print(airsstack.regex.replace([[\s+$]], r.stdout, ""))
end
```

```bash
airsl run --allow-exec git script.lua
```

`run` takes an argv array and there is no string form, so there is no shell, no word splitting and no
quoting bug available to you. A non-zero status is a *result*, not an error — the script asked what
happened, so it gets told.

The grant matches the program name as written: `--allow-exec git` does not permit `/usr/bin/git`.

### Read environment variables

```lua
local home = airsstack.env.get("HOME")
```

```bash
airsl run --allow-env HOME script.lua
```

An ungranted name **raises**; a granted-but-unset name returns `nil`. The distinction is deliberate —
otherwise a script reports a missing configuration when it was actually denied:

```
ok=false
denied: `SECRET_TOKEN` is not granted — the allowed names are HOME, MY_VAR
```

`env.all()` returns only the granted names, never everything the host process inherited.

### Pass a variable to a child process

```lua
airsstack.env.set("MY_VAR", "from-lua")
local r = airsstack.proc.run({"sh", "-c", "echo $MY_VAR"})
```

```
child sees: from-lua
```

`env.set` writes a per-process overlay that `env.get`, `env.all` and `proc.run` all consult. It does
**not** change the host process's own environment — partly because `std::env::set_var` is `unsafe` in
Edition 2024 and this crate forbids `unsafe`, and partly because a sandboxed script silently changing
the host's environment is not a capability anyone meant to grant.

### Derive the plugin suite's project key

```lua
local key = airsstack.hash.sha1(repo_path):sub(1, 8)
```

```
d50c1217
```

SHA-1 is here for compatibility only: `enforce.py` uses `hashlib.sha1(...).hexdigest()[:8]` and
`ensure-layout.sh` uses `shasum | cut -c1-8`. A port that used SHA-256 would silently re-key every
project and orphan its stored specs and plans. Use `hash.sha256` for anything new.

`hash.hash_file` reads a file, so it needs the read grant that reading it would.

### Format a timestamp reproducibly

```lua
airsstack.time.format(1700000000)              --> 2023-11-14T22:13:20Z
airsstack.time.format(1700000000, "%Y-%m-%d")  --> 2023-11-14
```

`format` takes an explicit instant rather than defaulting to now, and renders UTC rather than local
time, so a script that writes a timestamp into a file produces the same bytes on every machine.

There is no `sleep`: an arbitrary pause would defeat the instruction ceiling, which is the only
defence against a script that never finishes.

### Handle text as characters rather than bytes

```lua
#"café"            --> 5   (bytes)
utf8.len("café")   --> 4   (characters)
```

`utf8` is on every preset. Truncating on the byte count can cut through the middle of a character.

### Debug a refusal

Read the whole message — it names the roots that *were* granted:

```
airsl: fs.read denied: `/etc/hostname` is outside them — granted read roots are /home/me/journal
```

The usual cause is a grant one directory too deep. If the roots list looks right but nothing matches,
check `airsl doctor --policy <preset>` to see the resolved policy.

Relative and symlinked grant roots are resolved when the grant is built, so `--allow-read .` and a
root reached through a symlink both work.

## From Rust

### Embed the runtime

```rust
let engine = Engine::builder().policy(Policy::confined()).build()?;
let script = Script::from_source("return airsstack.json.encode({ok = true})", "demo")?;
println!("{}", engine.eval_to::<String>(&script)?);   // {"ok":true}
```

There is no `build()` until `policy()` has been called, so "did I remember to sandbox it" is not a
question any call site has to ask.

### Grant authority from Rust

```rust
let policy = Policy::confined()
    .with_grants(GrantSet::declared().with_fs(|fs| fs.read(&dir)));
let engine = Engine::builder().policy(policy).build()?;

let script = Script::from_source("return airsstack.fs.read(arg[1])", "read")?
    .with_args([path.to_string_lossy().into_owned()]);
engine.eval_to::<String>(&script)?;                   // body
```

Each axis has a wither: `with_fs`, `with_env`, `with_proc`. Nothing is granted until you say so.

### Reuse one engine

```rust
let add = Script::from_source("return tonumber(arg[1]) * 2", "add")?;
for n in 1..=3 {
    total += engine.eval_to::<i64>(&add.clone().with_args([n.to_string()]))?;
}
```

Reuse is worth about thirty-fold — 4.6 µs against 136 µs to build a fresh state — so for anything
that evaluates repeatedly this is the shape to use, not an optimisation to add later. Each
evaluation gets its own `arg` table, its own `require` root and the whole instruction budget.

What a reused engine does **not** reset is the Lua globals a script wrote. Give one engine scripts
that trust each other.

### Share an engine between threads

```rust
let shared = Arc::new(Engine::builder().policy(Policy::confined()).build()?);
// ... spawn threads, each calling shared.eval_to(...)
```

```
0,1,2,3
```

`Engine` is `Send + Sync` and evaluations are serialised, so each thread gets its own arguments.
This is a way to avoid rebuilding a state, **not** a way to get parallelism — one Lua state cannot
execute in parallel however you hold it.

### Add your own host module

```rust
struct Metrics(ModuleName);

impl HostModule for Metrics {
    fn name(&self) -> &ModuleName { &self.0 }

    fn install(
        &self,
        lua: &airsl::mlua::Lua,
        table: &airsl::mlua::Table,
        cx: &InstallContext<'_>,
    ) -> Result<()> {
        let root = cx.root_table().as_str().to_owned();
        let f = lua
            .create_function(move |_, name: String| Ok(format!("{root}.metrics recorded {name}")))
            .map_err(|e| airsl::Error::lua("metrics", e))?;
        table.set("record", f).map_err(|e| airsl::Error::lua("metrics", e))
    }
}

let mut set = airsl::modules::stdlib()?;
set.insert(Box::new(Metrics(ModuleName::new("metrics")?)))?;

let engine = Engine::builder()
    .policy(Policy::confined())
    .root_table(RootTable::new("myapp")?)
    .stdlib(set)
    .build()?;
```

```
myapp.metrics recorded x
```

Three things to know:

- **Depend on `airsl::mlua`,** not on `mlua` directly. A separately declared version produces type
  errors that never mention the real cause.
- **`install` receives the policy** through `InstallContext`. A module that guards an operation reads
  its authority from there, so what it enforces and what `airsl doctor` reports cannot disagree.
- **Name your own root table.** A module contributed by a third party should not land in a namespace
  called `airsstack`.

### React to a failure without propagating it

```rust
if let Err(error) = engine.eval(&script) {
    let breached = error.exhausted_limit().is_some();
    if !FailurePolicy::FailOpen.swallows_errors() || breached {
        eprintln!("{error}");
    }
}
```

`exhausted_limit` separates "this script is broken" from "this script ate the host's memory or never
terminated". A caller that discards ordinary failures usually still wants to hear the second, because
it is a fact about the machine rather than a diagnostic the script chose to emit.

## Testing

### Run a suite

```bash
airsl test .
airsl test --allow-read . plugins/
```

Test files are named `*_test.lua` or `test_*.lua` and return a table of named functions. A test
passes by returning and fails by raising.

Each file gets a fresh engine, so one file cannot leave globals behind for the next. Finding no test
files at all exits non-zero — "no tests" and "all tests passed" must not read the same to CI.

### Keep tests grant-free

Push logic out of the I/O and into pure functions, then test those. A suite that needs no grants is
one nobody has to think about:

```lua
return {
  finds_every_level = function()
    assert(#headings.find("# One\ntext\n## Two\n") == 2)
  end,
}
```

## See also

- **[Tutorial](tutorial.md)** — the guided version, from nothing to a working hook.
- **[Host standard library](stdlib.md)** — the full roster and the rules every module follows.
- **[Sandbox](sandbox.md)** — what a grant is and where it is enforced.
- **[`airsl-cli` README](../../airsl-cli/README.md)** — every flag the binary takes.
