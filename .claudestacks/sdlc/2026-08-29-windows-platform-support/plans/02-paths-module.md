---
status: done
created: 2026-08-30
depends-on: [01]
---

# Paths Module Implementation Plan

**Goal:** Create `crates/airsl/src/paths/` as the single home for platform path rules, the
`ResolvedPath` newtype, and the shared containment predicate.

**Architecture:** Three files under a folder module whose `mod.rs` is a table of contents only.
Every lexical rule is a pure function taking an explicit `PathFlavor`, with a `native` submodule
supplying one-argument wrappers bound to the compile-time flavour. This parameterisation is what
lets the Windows rules be unit-tested on Linux and macOS. Nothing wires into callers yet — this
plan builds the module and its tests; plans 03–06 consume it.

**Tech Stack:** Rust 1.94, `std::path`, no new dependencies.

**Content authority:** spec §2, premise §1.5, and the `ResolvedPath` contract in §2.

---

## File structure

```
crates/airsl/src/paths/mod.rs          — create  TOC + module doc only
crates/airsl/src/paths/rules.rs        — create  pure lexical rules, flavour-parameterised
crates/airsl/src/paths/resolved.rs     — create  ResolvedPath newtype
crates/airsl/src/paths/containment.rs  — create  shared root-comparison predicate
crates/airsl/src/lib.rs                — modify  register `mod paths;`
```

Every file ships a colocated `#[cfg(test)] mod tests` per the unit-test mandate. Module docs
follow the fixed shape: what it is, why it exists as its own module, `Responsibilities:`,
`Non-responsibilities:`.

### Task 1 — `rules.rs`: the flavour-parameterised core

**Files:**
- Create `crates/airsl/src/paths/rules.rs`

**Steps:**

1. Write failing tests first, driving **both** flavours from this macOS host. This is the point of
   the parameterisation and the tests must prove it:
   - `reject_unrepresentable(r"\\?\C:\x", Windows)` → `Err(Verbatim)`
   - `reject_unrepresentable(r"\\?\C:\x", Posix)` → `Ok(())` (an ordinary unix filename)
   - `reject_unrepresentable("NUL", Windows)` → `Err(DeviceNamespace)`
   - `reject_unrepresentable("NUL", Posix)` → `Ok(())` — a real file on Linux; refusing it would
     regress unix
   - `reject_unrepresentable("a\0b", _)` → `Err(InteriorNul)` on both
   - `to_script_string(Path::new(r"a\b"), Windows)` → `"a/b"`
   - `to_script_string(Path::new(r"a\b"), Posix)` → `r"a\b"` — a legal unix filename, unchanged
   - `strip_verbatim(r"\\?\C:\x", Windows)` → `C:\x`; `\\?\UNC\srv\share` → `\\srv\share`
   - `executable_candidates("git", Windows)` → `["git.exe"]`; `("git.exe", Windows)` →
     `["git.exe"]`; `("GIT.EXE", Windows)` → `["GIT.EXE"]` (case-insensitive suffix test);
     `("git", Posix)` → `["git"]`
   - `has_separator("a/b", Posix)` → true; `has_separator(r"a\b", Posix)` → false;
     `has_separator(r"a\b", Windows)` → true
2. Implement `PathFlavor`, `Unrepresentable`, and the five functions to make them pass.
3. Add the `native` submodule with `FLAVOR` and one-argument wrappers.
4. Verify:
   ```
   $ cargo test -p airsl paths::rules
   ```
   Expected: every rule green for both flavours on macOS.

### Task 2 — `resolved.rs`: `ResolvedPath`

**Files:**
- Create `crates/airsl/src/paths/resolved.rs`

**Steps:**

1. Write the newtype over `PathBuf` with a `pub(crate)` constructor and exactly two accessors:
   `as_path(&self) -> &Path` and `to_script_string(&self) -> String`.
2. Do **not** derive or implement `Display`, `AsRef<str>`, `AsRef<Path>`, or `Deref`, and keep the
   inner field private. The doc comment states why: the absence is the mechanism, and adding any
   of them silently re-opens native spelling at every call site.
3. State the invariant in the doc comment exactly as spec §2 words it — absolute, non-verbatim,
   symlink-resolved to the deepest existing ancestor, no `..` below that point — and state
   explicitly that it does **not** mean "inside a granted root".
4. Add a test asserting `to_script_string` yields `/` separators for a constructed value.
5. Verify:
   ```
   $ cargo test -p airsl paths::resolved
   ```

### Task 3 — `containment.rs`: the shared predicate

**Files:**
- Create `crates/airsl/src/paths/containment.rs`

**Steps:**

1. Write failing tests: a path under a root is contained; a sibling whose name merely shares the
   root's prefix (`app-extra` vs `app`) is not; comparison is component-wise, not string-prefix.
2. Implement `is_within(path: &Path, root: &Path) -> bool` over `Path::starts_with`, and a
   `contains_any(roots: &[PathBuf], path: &Path) -> bool` to replace the private copy at
   `crates/airsl/src/sandbox/grants.rs:132-134`.
3. Document that comparison is **never** case-folded, citing spec §1.3 and §3.6: folding would be
   wider than a case-sensitive NTFS directory allows, which is an escape.
4. Verify:
   ```
   $ cargo test -p airsl paths::containment
   ```

### Task 4 — Register the module

**Files:**
- Create `crates/airsl/src/paths/mod.rs`
- Modify `crates/airsl/src/lib.rs`

**Steps:**

1. Write `mod.rs` as TOC only — module doc plus `mod` and `pub(crate) use`, no implementation.
2. Add `mod paths;` to `crates/airsl/src/lib.rs` alongside the existing private modules.
3. Verify the whole gate, since nothing consumes the module yet and it must not warn:
   ```
   $ cargo make dod-crate airsl
   ```
   Expected: green, zero warnings. Unused-item warnings here mean a `pub(crate)` item no plan
   consumes — check it against plans 03–06 before silencing it.

---

## Verification summary (plan-level)

- `cargo make dod` green on macOS and, via CI, on Linux and Windows.
- Every lexical rule has a test for **both** flavours, and those tests pass on a unix host — the
  premise §1.5 claim is demonstrated, not asserted.
- `ResolvedPath` has no `Display`, no `AsRef<str>`, no `Deref`, no public field. A test or a
  grep confirms it.
- No existing file's behaviour has changed; this plan is additive.
