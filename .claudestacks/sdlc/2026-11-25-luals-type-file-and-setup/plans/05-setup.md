---
status: approved
created: 2026-11-25
depends-on: [04]
built-on: 044432888b01
---

# Setup Implementation Plan

**Goal:** `luals::setup` writes a project's type file and luarc, writing nothing when any check fails.

**Architecture:**
- `luals/setup.rs` is the only file in `luals` that does I/O (M-MOCKABLE-SYSCALLS, spec §2).
- It composes plan 02's `verify`, plan 03's `TypeFile::render` and plan 04's `luarc::plan`.
- It resolves each target through symlinks before writing. It writes with a temporary file in the resolved target's directory, then renames it over the target, using the existing `tempfile` dependency (`crates/airsl/Cargo.toml:36`).
- Two new `Error` variants carry its refusals. The existing `Io` variant (`crates/airsl/src/error.rs:251-257`, fields `operation: &'static str`, `path: String`, `source`) carries its I/O failures.

**Tech Stack:** Rust 1.94, crate `airsl`, `tempfile`, std `fs` / `os::unix`, cargo-make.

**Content authority:** spec §6 (paths in errors, target resolution, writing, flow steps 1–6, luarc selection, report), §6.1 (refusal display), §8 (`LuarcRefused`, `TypeFileRefused`, `Io` reuse and its doc sentence), §9 (the `setup.rs` rows), §10 (`luals/setup.rs` tests and D17 platforms), §11 (the `setup` doctest).

---

## Files that change

```
crates/airsl/src/luals/setup.rs   — create  setup(), SetupReport, FileChange, target resolution, atomic write
crates/airsl/src/luals/mod.rs     — modify  mod setup; pub use setup, SetupReport, FileChange
crates/airsl/src/error.rs         — modify  + LuarcRefused, + TypeFileRefused; Io doc sentence
```

## Order of work

### Task 1 — Run the setup flow and report what changed

**Files:**
- Create `crates/airsl/src/luals/setup.rs`: `setup(project: &Path, modules: &ModuleSet, root: &RootTable) -> Result<SetupReport>`; `SetupReport` with `type_file()`, `luarc()` and `Display`; `FileChange { Created, Updated, Unchanged }` with `Display` (`created` / `updated` / `unchanged`). All derive `Debug`, `Clone`, `PartialEq` and `Eq`, and `FileChange` also derives `Copy` (D25). `SetupReport`'s `Display` joins its two lines with `\n` and adds no final newline. The module doc follows the crate's shape.
- Modify `crates/airsl/src/luals/mod.rs`: add `mod setup;` and re-export `setup`, `SetupReport` and `FileChange`.
- Modify `crates/airsl/src/error.rs`:
  - add `LuarcRefused { path: String, reason: String }` and `TypeFileRefused { path: String, reason: String }`, each with its §8 display;
  - change `Io`'s doc sentence to "An operating-system call failed." Its display is unchanged.

**Change:**

The flow, §6 steps 1–6. Steps 1–4 write nothing, and the first error ends the call:
1. `read_dir(project)`, or `Io { operation: "open project directory", path: <project as given> }`.
2. `verify` every module in order.
3. Render. Its error is returned as is.
4. All reads happen in this step:
   - Pick the luarc: `.luarc.json` if `symlink_metadata` succeeds, else `.luarc.jsonc` if it does, else create `.luarc.json`.
   - Decide presence for both targets by the D18 first rule: `NotFound` or `NotADirectory` from `symlink_metadata` means absent, and any other error is `Io { operation: "read", path: project.join(<rel>) }`.
   - Read the existing type file's bytes if it is present, and the luarc's bytes if it is present. Either read can fail with `Io` "read".
   - Run `luarc::plan`. A refusal becomes `LuarcRefused { path: project.join(<rel>).display(), reason }`.
5. `create_dir_all(project/.airsl/types)` (`Io` "create directory"). Write the type file only if its bytes differ from those read in step 4.
6. Write the luarc only on `LuarcPlan::Write`.

In this task a present target is treated as a regular file. Task 2 adds the symlink and special-file rules (D18's second rule, and §6 steps 2–3).

Writes and paths:
- A write goes to `tempfile::Builder::tempfile_in(<target's directory>)`, then `persist` over the target (§6).
- File modes are set on the temporary file before `persist` (D23):
  - a created file gets `0o644` on Unix, via `std::os::unix::fs::PermissionsExt` under `#[cfg(unix)]`;
  - an updated file gets the existing target's `Permissions`.
- An existing target whose `permissions().readonly()` is true is refused in step 4 with `it is read-only`: `LuarcRefused` for the luarc, `TypeFileRefused` for the type file. `TypeFileRefused` is therefore added in this task, not Task 2.
- A failed write is `Io { operation: "write", path: <target> }`, never naming the temporary file.
- Every error path is `project.join(<relative path>)` shown with `Path::display()`.

The report:
- Two lines, type file first. Paths are relative to the project with `/` separators: `created|updated|unchanged .airsl/types/<root>.lua`, then the luarc line.
- `Created` means the target was absent, `Updated` means its bytes changed, and `Unchanged` means nothing was written.

Add a rustdoc doctest on `setup` that runs it on a `tempfile::tempdir()` with `stdlib()` and prints the report.

**Proof:** all tests use `tempfile::tempdir()`.
- `first_run_creates_both_files`: the report is `created .airsl/types/airsstack.lua\ncreated .luarc.json`. The type file equals `TypeFile::render(...).as_str()`, and the luarc equals the L1 bytes.
- `second_run_reports_unchanged_and_writes_nothing`: a second call reports `unchanged` twice, and both files' modification times equal those after the first run.
- `a_changed_module_updates_only_the_type_file`: re-running with an extra stub module reports the type file `updated` and the luarc `unchanged`.
- `a_hand_edited_type_file_is_overwritten`: after the type file is overwritten with `x`, a re-run reports it `updated` and restores the rendered text (§9).
- `other_files_in_the_types_directory_are_untouched`: a file `.airsl/types/other.lua` keeps its bytes across a run (§9).
- `a_luarc_refusal_writes_nothing`: `.luarc.json` holds the probe's comment input, `{\n  // keep me\n  "workspace.library": []\n}`. The error displays exactly ``will not edit `<project.join(".luarc.json").display()>`: it is not plain JSON (key must be a string at line 2 column 3); add ".airsl/types" to `workspace.library` by hand``, and `.airsl` does not exist afterwards.
- `a_drifting_module_writes_nothing`: with a stub whose description misses a key, the error is `DescriptionDrift`, and neither `.airsl` nor `.luarc.json` exists afterwards.
- `a_missing_project_is_refused`: for a non-existent path, the error display starts with ``open project directory failed on `<path>`: `` (a prefix check, because the OS text differs by platform), and the path still does not exist.
- `a_file_as_project_is_refused`: a regular file as the project gives the same operation, and nothing is created beside it.
- `prefers_luarc_json_over_jsonc`: with both files present, only `.luarc.json` changes and the report names `.luarc.json`. `.luarc.jsonc` keeps its bytes.
- `edits_luarc_jsonc_when_it_is_the_only_one`: with only `.luarc.jsonc` (plain JSON, no entry), it is rewritten and the report names `.luarc.jsonc`.
- `io_errors_name_the_joined_path`: with `.airsl` created as a regular file, step 4 treats the type file as absent (`NotADirectory`). The error display then starts with ``create directory failed on `<project.join(".airsl/types").display()>`: ``. It is a prefix check, and the expected path is computed with `Path::join` rather than written as a literal, so the test holds on Windows.
- `an_unreadable_types_directory_is_an_io_read_error` (Unix only, `#[cfg(unix)]`, mode set with `std::os::unix::fs::PermissionsExt`): `.airsl/types` exists with mode 000. The error display starts with ``read failed on `<project.join(".airsl/types/airsstack.lua").display()>`: ``, and `.luarc.json` was not written. The test restores mode 755 before the tempdir is dropped.
- `a_created_file_has_mode_644` (Unix only): after a first run, both files' modes are `0o644`.
- `an_updated_file_keeps_its_mode` (Unix only): a `.luarc.json` with mode `0o640` that needs an edit still has mode `0o640` afterwards.
- `a_read_only_luarc_is_refused` (every platform): a `.luarc.json` set read-only with `Permissions::set_readonly(true)` gives ``will not edit `<project.join(".luarc.json").display()>`: it is read-only; add ".airsl/types" to `workspace.library` by hand``, and `.airsl` does not exist afterwards. The test clears the flag before the tempdir is dropped.
- `an_empty_module_set_sets_up_a_project`: with `ModuleSet::new()`, setup succeeds, and the type file holds only the header and root class (§9).
- `a_describe_error_stops_setup_before_writing`: a stub whose `describe` fails gives `DescribeFailed`, and neither `.airsl` nor `.luarc.json` exists afterwards (§9).

**Done when:** `cargo test -p airsl luals::setup` passes and `cargo make dod` is green. Commit `feat(airsl): write the LuaLS type file and luarc with luals::setup`.

### Task 2 — Resolve symlinked and special targets before writing

**Files:**
- Modify `crates/airsl/src/luals/setup.rs`: target resolution, wired into flow step 4 for both targets.
- No `error.rs` change: `TypeFileRefused` landed in Task 1.

**Change:**
- Resolution follows §6 "Resolving a target" and D18. The absent rule is already in place from Task 1. This task adds:
  - If the target is a symlink, `canonicalize` it. `NotFound` is refused with `it is a dangling symlink`, and any other error (such as `FilesystemLoop`) is `Io { operation: "read", path: project.join(<rel>) }`.
  - If the resolved target is not a regular file (checked with `std::fs::metadata` on the canonical path; a failure there is `Io` "read"), it is refused with `it is not a regular file`, and it is never opened. A symlink whose target is read-only is refused with `it is read-only`, as in Task 1.
- The luarc's refusals use `LuarcRefused` and the type file's use `TypeFileRefused`. Each names `project.join(<rel>)`, the path as the user sees it, not the resolved path.
- Reads and writes then go to the resolved path. The temporary file is created in the resolved target's directory, so a symlink survives the rename (D12).

**Proof:** tests marked Unix only are `#[cfg(unix)]` (D17). They make links with `std::os::unix::fs::symlink` and the socket with `std::os::unix::net::UnixListener::bind`, with no external tool and no new dependency.
- `a_symlinked_luarc_is_written_through` (Unix only): `.luarc.json` is a link to `elsewhere/luarc.json`, which holds `{}`. After setup, `.luarc.json` is still a symlink, and `elsewhere/luarc.json` holds the L1 bytes.
- `a_dangling_symlink_luarc_is_refused` (Unix only): a link to a missing file gives ``will not edit `<project>/.luarc.json`: it is a dangling symlink; add ".airsl/types" to `workspace.library` by hand``, and `.airsl` does not exist afterwards.
- `a_socket_luarc_is_refused_without_opening_it` (Unix only): a `UnixListener` bound at `<tempdir>/.luarc.json` gives `LuarcRefused` with reason `it is not a regular file`, not `Io`.
- `a_symlinked_type_file_is_written_through` (Unix only): `.airsl/types/airsstack.lua` is a link to `elsewhere/types.lua`. After setup the link survives, and `elsewhere/types.lua` holds the rendered text.
- `a_symlink_cycle_luarc_is_an_io_read_error` (Unix only): `.luarc.json` and `b.json` link to each other. The error display starts with ``read failed on `<project.join(".luarc.json").display()>`: ``, and `.airsl` does not exist afterwards.
- `a_dangling_symlink_type_file_is_refused` (Unix only): `.airsl/types/airsstack.lua` links to a missing file. The error displays ``will not write `<project.join(".airsl/types/airsstack.lua").display()>`: it is a dangling symlink``.
- `a_directory_at_the_type_file_path_is_refused` (every platform): a directory at `.airsl/types/airsstack.lua` gives ``will not write `<project.join(".airsl/types/airsstack.lua").display()>`: it is not a regular file``, and no luarc is written.

**Done when:**
- `cargo test -p airsl luals::setup` passes.
- `cargo make dod` is green. On this macOS machine, that runs the Unix-only tests too.

Commit `feat(airsl): resolve symlinked and special setup targets before writing`.

---

## Risks

- **Partial success is possible by design.** If step 6 fails after step 5, the type file is written and the luarc is not (§6). A re-run completes it. No test forces a step-6 failure, because it would need a write fault injected between two renames. The flow order is reviewed instead.
- **Windows has no coverage for symlinks or special files** (D17, spec §13). CI's `windows-latest` job runs only the platform-neutral tests.
- **mtime equality** in `second_run_reports_unchanged_and_writes_nothing` relies on "not written" meaning "not touched". The bytes-differ check must run before any temporary file is created.

## Proof

- `cargo make dod` is green.
- `cargo test -p airsl luals::` runs every `verify`, `render`, `luarc` and `setup` test together.
