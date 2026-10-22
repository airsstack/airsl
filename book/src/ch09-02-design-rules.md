# The Rules Behind the Design

A handful of rules explain almost every decision in `airsl`. If you are adding to it, these are the
ones to keep.

## Every module is a capability

A module is the unit of permission. `path` and `fs` are separate because building a path needs no
permission and opening one does. If they were one module, a script that only joins file names would
have to be granted file access to do it.

So adding a function that makes a module need a grant it did not need before is not a small change.
It changes what every user of that module has to grant.

## Checks happen in Rust, never in Lua

Every grant is checked inside the Rust function, right before the operation. A Lua script never holds
a file handle or a process handle. It holds a string, and asks Rust to act on it.

The alternative, a helper function in Lua that checks a path before opening it, does not work. A
script that forgets to call the helper is unprotected, and nothing makes it call the helper. A check
the script can skip is not a check.

## Checking a path properly

Deciding whether a path is inside a granted folder sounds simple, and the simple version is wrong.

The tempting approach is to clean the path up as text, by resolving `.` and `..`, and see whether the
result starts with the granted folder. Here is why that fails:

```
  granted:   /srv/app
  symlink:   /srv/app/link  →  /elsewhere

  path:      /srv/app/link/../secret

  as text:   /srv/app/secret      ✓ looks inside the folder
  on disk:   /elsewhere/../secret = /secret   ✗ is actually outside it
```

On disk, `..` is applied *after* the symlink is followed, so the two answers differ. A text check
would let the script read `/secret`.

What `airsl` does instead: it finds the deepest part of the path that really exists, and asks the
operating system for that part's true location, with every symlink followed. It compares that against
the grant. Below that point the path does not exist yet, so it cannot contain a symlink, and `airsl`
accepts only plain names there. A `..` below that point is refused rather than resolved.

## The same input gives the same output

Determinism is treated as part of being correct, not as a nice extra:

- JSON object keys are always sorted.
- Folder listings are always sorted, by name, byte by byte.
- `time.format` always uses UTC, and always needs an explicit moment.
- Paths are always written with `/`, on every platform.

The reason is that non-determinism does not fail loudly. It shows up as a diff that should not be
there, or a cache that never hits, and those are hard to trace back.

This rule is also why `os.setlocale` is withheld under `confined`, even though it is harmless in
itself. Lua compares strings using the current locale, so a script that changed the locale would
change the order of every `table.sort` in every script after it.

## Present but refusing

Every module is installed under every preset, even when the script has no grant for it. An ungranted
module refuses each call with a message that names what *was* granted. If modules disappeared instead,
`if airsstack.fs then` would mean "am I allowed?" rather than "does this runtime have files?", and a
script would skip its work silently instead of telling you why.

## Stopped is not the same as failed

When a script hits a memory or instruction limit, the error says so in its type, not just its text.
The engine works it out from its own instruction counter and from the VM's own error. A script can
raise an error whose message reads exactly like a limit being hit, and it is still reported as an
ordinary failure. That is why the command line can always report a limit being hit, even under
`--fail-open`, without being fooled.
