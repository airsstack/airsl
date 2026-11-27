---
status: done
created: 2026-11-26
depends-on: [01]
built-on: af7c0dcc7368
---

# airsl Cargo Features Implementation Plan

**Goal:** `airsl` selects every mlua build option it depends on (the Lua version, vendored linking,
`send`) through its own Cargo features, with today's build as the default set.

**Architecture:** The root `mlua` entry keeps only `serialize`. `airsl` declares six features:
- `default = ["lua54", "vendored", "send"]`;
- `lua54`, `luajit`, `vendored`, `send` and `module`, each forwarding to the `mlua` feature of the
  same name.

`airsl-cli` keeps getting the same seven mlua features through airsl's defaults (§2.4 table). The
features are non-additive (D2), so `--all-features` can no longer build. It leaves every gate
command and docs-check's inner `cargo run` first (D1), in its own task, which is green on its own
because no feature exists yet at that point.

`LanguageSurface::libraries()` gains `cfg(feature = "luajit")` match arms (form A, D6, M5). A LuaJIT
build therefore compiles, though it never creates a `LanguageSurface` state. The crate docs gain a
"Cargo features" section, and `Engine`'s rustdoc says `Send + Sync` holds with the default `send`
feature.

This plan depends on plan 01 because both edit the root `Cargo.toml` and `crates/airsl/Cargo.toml`.

**Tech Stack:** Cargo features (cargo 1.94.1), `mlua` 0.12.0 / `mlua-sys` 0.11.0 / `mlua_derive` 0.12.0,
cargo-make (`Makefile.toml`), crate `docs-check`. Cargo runs with `CARGO_BUILD_JOBS=2`
(`CLAUDE.md`).

**Content authority:** spec §1.1 F1–F3, §2.1 (root `Cargo.toml` mlua entry, `Cargo.lock`,
`crates/airsl/Cargo.toml`, `src/lib.rs`, `src/engine.rs`, `src/sandbox/language_surface.rs`,
`Makefile.toml`, docs-check `src/examples.rs`), §2.4, §2.9 D1, D2, D6, §2.10 M5, §3.1, §3.4
(`src/examples.rs`), §9.1 (`examples.rs` test), §9.3 (the dropped `--all-features`), §11 (rows for
the `Cargo.toml` comments and `crates/airsl/src/lib.rs`).

---

## Files that change

```
Makefile.toml                              — modify  every --all-features removed; the :32-34 comment replaced by D1's reason
crates/docs-check/src/examples.rs          — modify  the inner cargo run drops --all-features; module doc and the pinning test follow
Cargo.toml                                 — modify  mlua entry features = ["serialize"]; its comment says airsl's features choose Lua, linking and send
crates/airsl/Cargo.toml                    — modify  + [features] default/lua54/luajit/vendored/send/module
Cargo.lock                                 — modify  whatever cargo writes once `module` is declared (spec: + mlua_derive 0.12.0 → syn 2.0.119)
crates/airsl/src/lib.rs                    — modify  crate docs gain a "Cargo features" section (still export-only)
crates/airsl/src/engine.rs                 — modify  Engine rustdoc: Send + Sync with the default `send` feature
crates/airsl/src/sandbox/language_surface.rs — modify  libraries(): cfg(feature = "luajit") arms, form A
```

## Order of work

### Task 1 — Drop `--all-features` from the gate and from docs-check's inner run

**Files:**
- Modify `Makefile.toml`:
  - remove `"--all-features"` from the `clippy`, `doc`, `test`, `test-doc` and `clippy-fix` args
    (`Makefile.toml:65`, `:77`, `:92`, `:98`, `:362`).
  - remove `--all-features` from the four cargo lines of the `dod-crate` script (`:400`, `:405`,
    `:407`, `:411`).
  - replace the comment at `:32-34` ("carried for forward-safety …") with D1's reason. `airsl`
    forwards mlua's mutually exclusive features, so an all-features build is a compile error by
    construction. Coverage is the default build plus the per-ABI runs of the native module, which
    plan 04 adds. Write it in the file's own comment style.
- Modify `crates/docs-check/src/examples.rs`:
  - remove `"--all-features"` from the inner `cargo run` argument list (`examples.rs:202`).
  - rewrite the module doc sentence "It also passes `--all-features` …" (`examples.rs:30-32`), so
    the doc still states that the inner build is the outer build: the same profile, triple and target
    directory.
  - in the test now named `the_inner_run_carries_the_outer_target_dir_profile_triple_and_all_features`
    (`examples.rs:453`), drop `--all-features` from the expected argument list (`:473`) and rename the
    test to `the_inner_run_carries_the_outer_target_dir_profile_and_triple`.

**Change:** No crate declares features yet, so each command builds exactly what it built before. The
gate keeps its five steps in their order.

**Proof:**
- `the_inner_run_carries_the_outer_target_dir_profile_and_triple`: written first by editing the
  expected list, and seen failing while `examples.rs:202` still passes the flag. It asserts that the
  inner command is `run -q -p airsl --example hello --profile release --target-dir /w/target
  --target aarch64-apple-darwin`, with no `--all-features`.
- `grep -n "all-features" Makefile.toml crates/docs-check/src/examples.rs` finds no command use. Only
  D1's comment remains.

**Done when:** `cargo test -p docs-check examples` passes and `cargo make dod` is green. Commit
`build(workspace): stop passing --all-features to the gate`.

### Task 2 — Declare airsl's features with today's build as the default

**Files:**
- Modify `Cargo.toml`:
  - set the `mlua` workspace entry to `features = ["serialize"]`, the line in
    `probes/201-lockstep-manifests/Cargo.toml.diff`.
  - rewrite its comment (`Cargo.toml:15-20`): `serialize` is the only feature fixed here, and
    `airsl`'s own features choose the Lua version, vendoring and `send`. Keep the reason that Lua
    5.4 is airsl's engine (integers against floats for byte-stable JSON).
- Modify `crates/airsl/Cargo.toml`: add the `[features]` table exactly as in
  `probes/201-lockstep-manifests/crates_airsl_Cargo.toml.diff`, between the `[[example]]` table and
  `[dependencies]`.
- Modify `Cargo.lock`: commit what cargo writes. Spec §2.1 expects `mlua_derive 0.12.0` and
  `syn 2.0.119`.
- Modify `crates/airsl/src/lib.rs`: add a "Cargo features" section to the crate `//!` docs. It
  states:
  - the default is `lua54`, `vendored` and `send`, which is today's build;
  - the two other supported sets are `lua54,module` and `luajit,module`, used only by the native
    module build;
  - `default-features = false` without `lua54` or `luajit` does not build ("No Lua feature
    enabled");
  - `Engine` is `Send + Sync` only with `send`.

  `lib.rs` stays export-only: docs, `mod`, `pub use`.
- Modify `crates/airsl/src/engine.rs`: in `Engine`'s rustdoc, state that the type is `Send + Sync`
  with the default `send` feature.

**Change:** The default build resolves mlua to `default, error-send, lua54, send, serde, serialize,
vendored` for `-p airsl`, `-p airsl-cli` and the workspace, as today (§2.4 table). `module` and
`luajit` are declared, but no shipped crate enables them yet.

**Proof:**
- `cargo tree -e features -i mlua -p airsl` and `cargo tree -e features -i mlua -p airsl-cli` each
  list the seven features of the §2.4 table and neither `module` nor `luajit`.
- `cargo check -p airsl --no-default-features --features lua54,module --lib` exits 0 (F2,
  `probes/02-airsl-lua54-module-mode.txt` "lib exit=0").
- `cargo check -p airsl --no-default-features --lib` fails with mlua's "No Lua feature enabled". This
  is the documented D2 side effect (`probes/05-feature-split-workspace.txt` (f)), and the crate docs
  must say it.
- Every existing test passes unchanged under the default features.

**Done when:** the checks above give those results, `cargo metadata --format-version 1 > /dev/null`
succeeds, and `cargo make dod` is green. Commit `feat(airsl): choose Lua, vendoring and send through
airsl's own features`.

### Task 3 — Gate the `utf8` and `coroutine` arms on `luajit`

**Files:**
- Modify `crates/airsl/src/sandbox/language_surface.rs`: apply form A from
  `probes/143-luajit-stdlib-cfg/formA.diff` in `libraries()`:
  - today's `Restricted` and `Minimal` arms take `#[cfg(not(feature = "luajit"))]`;
  - new `#[cfg(feature = "luajit")]` arms give `Restricted` = string, table, math and os, and
    `Minimal` = string, table and math;
  - the comment says LuaJIT's `coroutine` is in its base library and it has no `utf8`.

  Change no other arm, and leave the tests as they are. They run under the default features, where
  the arms are today's.

**Change:** A `luajit,module` build of `airsl` compiles. The default build is unchanged. No shipped
build creates a `LanguageSurface` state under LuaJIT, because `ModuleSet::open` loads no library
(§3.1).

**Proof:**
- `cargo check -p airsl --no-default-features --features luajit,module --lib` exits 0. Before this
  task it fails with "no associated item named `UTF8` found for struct `LuaStdLib`" (F3;
  `probes/03-airsl-luajit-compile.txt`). After it, `[exit 0]` (`probes/143-luajit-stdlib-cfg.txt`).
- The existing `language_surface.rs` tests (`restricted` / `minimal` / `utf8` assertions,
  `language_surface.rs:139-176`) still pass under the default build.

**Done when:** that check exits 0 and `cargo make dod` is green. Commit `feat(airsl): compile the
language surface under LuaJIT`.

---

## Risks

- Task 2 changes what a dependent with `default-features = false` gets. It now fails to build until
  it names a Lua feature (D2, R7). The crate docs say so, and plan 12 writes the CHANGELOG
  `### Breaking` item.
- Spec §2.1 attributes the `syn` 2 entry in `Cargo.lock` to `module`. Probe 144 (a) showed
  `cargo deny` refusing that duplicate with `airsl-lua` already a member
  (`probes/144-cargo-deny-features.txt` (a)); the state after this plan alone, before `airsl-lua`
  exists, was not probed, so `cargo make deny` (outside `dod`) may fail on it until plan 04 applies
  the `deny.toml` diff (D10). This plan does not run `deny`, and the plans land together.
- The LuaJIT arms are compiled only by a LuaJIT build. Until plan 04 adds `airsl-lua`, the only
  thing that exercises them is the Task 3 check command, and no gate step runs it.

## Proof

- `cargo make dod` is green.
- `cargo check -p airsl --no-default-features --features lua54,module --lib` and
  `cargo check -p airsl --no-default-features --features luajit,module --lib` both exit 0.
- `grep -rn "all-features" Makefile.toml crates/docs-check/src` finds no command use.

## Review findings

- Reviewer (28-reviewer-p03): SPEC COMPLIANT, no drift, blocking none; code 2🟡 2🔵, spec 0. DoD: `cargo make dod` → exit 0 (865/1/101/66 tests, 27 doctests); `cargo check -p airsl --no-default-features --features lua54,module --lib` and `… luajit,module --lib` → exit 0 each.
- 🔵 `crates/airsl/src/lib.rs`:36 — the "Cargo features" section said `Engine` is `Send + Sync` twice — fixed — duplicate removed; `cargo make dod` → exit 0.
- 🟡 `crates/airsl/src/sandbox/language_surface.rs`:91 — on LuaJIT the `Minimal` doc's "no `coroutine`" and "`utf8` is on every surface" are false (reviewer probe, mlua 0.12.0 `luajit,vendored`: "minimal: coroutine=table utf8=nil") — fixed — the `Minimal` doc (:72-74) and the comment (now :93) are scoped to Lua 5.4 and state what LuaJIT gives; `cargo make dod` → exit 0; `cargo check -p airsl --no-default-features --features luajit,module --lib` → exit 0.
- 🔵 `crates/airsl/src/sandbox/language_surface.rs`:63 — remainder of the :91 finding: the `Restricted` doc still lists `utf8`/`coroutine` without the LuaJIT caveat — open — latent; no shipped build creates a `LanguageSurface` state under LuaJIT.
- 🟡 `crates/airsl/src/sandbox/language_surface.rs`:105 — reverting the `cfg(feature = "luajit")` arms or the root mlua `features = ["serialize"]` reduction left `cargo make dod` green (plan Risks, bullet 3) — open at review; closed by plan 04 — `cargo make dod` now runs `cargo clippy -p airsl-lua --features luajit` and `cargo doc -p airsl-lua --features luajit` (`airsl-lua`'s `luajit` = `airsl/luajit`) and native-load → exit 0, ending "check.lua: ok (Lua 5.1, LuaJIT 2.1.1770848219)".
- 🔵 `Makefile.toml`:34 — the comment states per-ABI runs of the native module as present fact before any exists — open at review; closed by plan 04, which adds the per-ABI clippy/doc/native-load runs (the comment is now at :37) — `cargo make dod` → exit 0 with both `check.lua: ok` lines.

## Deviations

- 2026-11-26: Task 1 — rustfmt reflowed the `.args([...])` list in `crates/docs-check/src/examples.rs` onto one line after `--all-features` was removed (applied with rustfmt on that file only). `cargo test -p docs-check examples` → 17 passed.
- 2026-11-26: the review-fix coder (lib.rs, language_surface.rs) reported inline and wrote no handoff file. Its result: `cargo make dod` → exit 0; `cargo check -p airsl --no-default-features --features luajit,module --lib` → exit 0.
- 2026-11-26: no behaviour departs from the plan. Recorded results: `cargo check -p airsl --no-default-features --lib` → exit 101 "No Lua feature enabled..."; Task 3 before the change → E0599 on `UTF8`/`COROUTINE` (3 errors), after → exit 0; `Cargo.lock` gains only `mlua_derive 0.12.0` (→ existing `syn 2.0.119`).
- 2026-11-26: No commit was made: the execute skill commits only when the instruction to execute says so; the plan's per-task commit messages remain for the author.
