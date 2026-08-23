# filesystem-grants

Reads through one root, writes through another, and asks for four things it was not granted.

This is its own example because the read root and the write root are separate grants, and that is
the fact about `FsGrant` most likely to be assumed away. A script handed a write root cannot read
what it just wrote — the host has to say so twice.

## Run

```bash
cargo run -p airsl --example filesystem-grants
```

The three directories are `tempfile::TempDir`s created at startup and removed on exit. Nothing is
written inside the repository.

## Output

```
read notes.txt: 69 bytes
stat notes.txt: kind=file size=69
copied to <output>/notes.txt
atomic_write wrote <output>/index.json
create_exclusive, first call:  true
create_exclusive, second call: false
<source> holds: notes.txt
writing into the read root: fs.write denied: `<source>/notes.txt` is outside the granted write roots: <output>
reading back from the write root: fs.read denied: `<output>/notes.txt` is outside the granted read roots: <source>
listing the write root: fs.list denied: `<output>` is outside the granted read roots: <source>
reading outside every root: fs.read denied: `<outside>/secret.txt` is outside the granted read roots: <source>
```

`<source>`, `<output>` and `<outside>` are substituted by `copy.lua` itself. A temporary directory
has a different name on every run, and an example whose output cannot be pasted into its own README
is one nobody can check.

## What it demonstrates

- **Read and write are separate roots.** `FsGrant::read` (`src/sandbox/grants.rs:44`) and
  `FsGrant::write` (`src/sandbox/grants.rs:54`) build independent allowlists, checked by
  `allows_read` (`:61`) and `allows_write` (`:67`). The second and third refusals above are the
  script failing to read files it had just written.
- **Containment is decided in exactly one place.** Every `fs` call — plus `hash.hash_file` and
  `glob.walk` — funnels through `PathGuard` (`src/modules/guard.rs:57`), whose `read` (`:74`) and
  `write` (`:87`) resolve the path before checking it. `resolve` (`:145`) canonicalises the deepest
  existing part of the path, so a symlink out of a granted root is caught, and refuses a `..` below
  that point rather than resolving it lexically.
- **A refusal names the roots that *were* granted** — `PathGuard::deny` (`src/modules/guard.rs:105`).
  The usual cause of a denial is a grant one directory too deep, which is invisible without the
  list. Which allowlist a refusal was measured against is carried as an `Access` value
  (`src/modules/guard.rs:31`) rather than a string, so the message and the check cannot name
  different directions.
- **Interrogation counts as reading.** `list`, `stat` and `exists` refuse an ungranted path rather
  than answering `false` for it (`src/modules/fs.rs:183`); answering would conflate "you may not
  ask" with "there is nothing there".
- **`create_exclusive` returns `false` rather than raising** when the file already exists
  (`src/modules/fs.rs:171`). Losing that race is the expected *other outcome*, not a failure —
  this is `O_CREAT|O_EXCL`, and a read-then-write would let several concurrent callers all believe
  they won.
- **`atomic_write` stages in the target's own directory and renames over it**
  (`src/modules/fs.rs:419`), so a concurrent reader sees the old bytes or the new ones, never half
  of each. `/tmp` is not used, because a rename across filesystems is not atomic.
- **`fs.list` sorts** (`src/modules/fs.rs:271`). Directory order is whatever the filesystem returns
  and differs between machines.

### `Script::from_file` is not governed by the grants

Loading the script is the *host's* own `std::fs` call. `FsGrant` governs what `airsstack.fs` does
once the script is running — nothing else. The script file does not have to live under a granted
root, and in this example it does not: it is read out of the repository while the script's own read
grant points at a temporary directory. Handing a script a path it could not itself open is normal.

## See also

- [sandbox](../../docs/sandbox.md) — what a grant is and where it is enforced.
- [`env-and-proc`](../env-and-proc/) — the other two grant axes.
- [`denials-are-data`](../denials-are-data/) — what a module does when it has no grant at all.
