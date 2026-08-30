//! `airsl ext doctor`: what a ceiling would grant one extension, without running it.
//!
//! Separate from `doctor` because that command describes a *policy* and this one describes a
//! *negotiation* — a manifest held against a ceiling. It deliberately stops before an engine
//! exists: an author inspecting a third-party manifest must be able to do so without executing
//! its entry script.
//!
//! Responsibilities: [`render`] (pure), [`run`] (read the manifest, negotiate, ask
//! [`ManifestApprover`], print, exit code).
//!
//! Non-responsibilities: running anything (`ext_fire`); the negotiation rules themselves
//! (`airsl::extension`).
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use core::fmt::Write as _;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use airsl::Policy;
use airsl::extension::{
    ApprovalRequest, Approver as _, Capability, CapabilityRequest, Ceiling, Decision, Manifest,
    ManifestApprover, Negotiation, Variables, negotiate,
};

use crate::cli::{ExtFlags, resolve_ceiling};
use crate::doctor::describe;

/// Column width a request's kind (`fs.read`, `proc.run`, …) is padded to.
const KIND_WIDTH: usize = 12;
/// Column width a request's value is padded to.
const VALUE_WIDTH: usize = 24;
/// Column width the leading label (`extension:`, `language:`, …) is padded to.
const LABEL_WIDTH: usize = 14;

/// Renders what `ceiling` would grant, reduce and deny for `manifest`, ending with `decision`.
///
/// Two policy blocks are printed because the report answers two different questions. `ceiling:`
/// is the host's real bound — `ceiling.policy()`, unmodified by anything this manifest asked for —
/// and `negotiated:` is the policy this extension would actually run under, which differs from the
/// ceiling wherever the manifest requested a tighter memory or instruction limit, or requested less
/// than the ceiling grants. Printing only one would hide either the host's bound or the manifest's
/// own request, and an author needs both to tell which number came from where.
///
/// `events` is `--event`'s declared set, sorted and deduplicated. `doctor` never dispatches
/// anything, so the flag has no other effect here; the line exists purely so the flag is
/// observable rather than silently accepted and discarded.
#[must_use]
pub(crate) fn render(
    manifest: &Manifest,
    ceiling: &Ceiling,
    negotiation: &Negotiation,
    decision: &Decision,
    events: &[String],
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<LABEL_WIDTH$}{} {} (api {}, entry {})",
        "extension:",
        manifest.name(),
        manifest.version(),
        manifest.api().get(),
        manifest.entry().display(),
    );
    let _ = writeln!(out, "{:<LABEL_WIDTH$}{}", "events:", events_summary(events));

    policy_block(&mut out, "ceiling:", ceiling.policy());
    policy_block(&mut out, "negotiated:", negotiation.policy());

    let _ = writeln!(out, "requested:");
    for (kind, value, capability) in requested_lines(manifest) {
        let tag = tag_for(&capability, negotiation);
        let _ = writeln!(out, "  {kind:<KIND_WIDTH$} {value:<VALUE_WIDTH$} {tag}");
    }

    match decision {
        Decision::Approve => {
            let _ = writeln!(out, "{:<LABEL_WIDTH$}approve", "decision:");
        }
        Decision::Deny(reason) => {
            let _ = writeln!(out, "{:<LABEL_WIDTH$}deny — {reason}", "decision:");
        }
    }
    out
}

/// Appends one `label:` block (`ceiling:` or `negotiated:`) describing `policy`'s language,
/// grants, memory and instruction ceiling.
fn policy_block(out: &mut String, label: &str, policy: &Policy) {
    let _ = writeln!(out, "{label}");
    let _ = writeln!(out, "  {:<LABEL_WIDTH$}{}", "language:", policy.language());
    let _ = writeln!(out, "  {:<LABEL_WIDTH$}{}", "grants:", policy.grants());
    let _ = writeln!(
        out,
        "  {:<LABEL_WIDTH$}{}",
        "memory:",
        describe(policy.limits().memory())
    );
    let _ = writeln!(
        out,
        "  {:<LABEL_WIDTH$}{}",
        "instructions:",
        describe(policy.limits().instructions())
    );
}

/// The `events:` line's value: `events`, sorted and deduplicated, joined with `, `, or `none`.
fn events_summary(events: &[String]) -> String {
    let unique: BTreeSet<&str> = events.iter().map(String::as_str).collect();
    if unique.is_empty() {
        "none".to_owned()
    } else {
        unique.into_iter().collect::<Vec<_>>().join(", ")
    }
}

/// Every requested capability, required then optional, in the order `fs.read`, `fs.write`,
/// `proc.run`, `env.read`, `module` — the order [`negotiate`] records denials in.
fn requested_lines(manifest: &Manifest) -> Vec<(&'static str, String, Capability)> {
    let mut lines = Vec::new();
    push_block(&mut lines, manifest.required());
    push_block(&mut lines, manifest.optional());
    lines
}

/// Appends one capability block's requests, resolving filesystem roots the way [`negotiate`]
/// does so the [`Capability`] built here compares equal to the one a denial or reduction carries.
fn push_block(lines: &mut Vec<(&'static str, String, Capability)>, block: &CapabilityRequest) {
    for root in block.fs_read() {
        let resolved = airsl::FsGrant::resolve_root(root);
        lines.push((
            "fs.read",
            resolved.display().to_string(),
            Capability::FsRead(resolved),
        ));
    }
    for root in block.fs_write() {
        let resolved = airsl::FsGrant::resolve_root(root);
        lines.push((
            "fs.write",
            resolved.display().to_string(),
            Capability::FsWrite(resolved),
        ));
    }
    for program in block.proc_run() {
        lines.push((
            "proc.run",
            program.to_owned(),
            Capability::ProcRun(program.to_owned()),
        ));
    }
    for name in block.env_read() {
        lines.push((
            "env.read",
            name.to_owned(),
            Capability::EnvRead(name.to_owned()),
        ));
    }
    for module in block.modules() {
        lines.push((
            "module",
            module.to_string(),
            Capability::Module(module.clone()),
        ));
    }
}

/// The status word (and, for a miss, its detail) for one requested capability.
fn tag_for(capability: &Capability, negotiation: &Negotiation) -> String {
    negotiation
        .denied()
        .iter()
        .find(|denial| denial.capability() == capability)
        .map(|denial| format!("denied    ({})", denial.detail()))
        .or_else(|| {
            negotiation
                .reduced()
                .iter()
                .find(|reduction| reduction.capability() == capability)
                .map(|reduction| format!("reduced   ({})", reduction.detail()))
        })
        .unwrap_or_else(|| "granted".to_owned())
}

/// Reports what `dir`'s manifest would be granted under the ceiling `flags` describes, and
/// returns the process exit code.
///
/// `flags.events` never reaches negotiation — `doctor` never dispatches anything — but it is
/// still read here, for the `events:` line [`render`] prints, so the flag is observable rather
/// than silently accepted and discarded.
///
/// `stdout` takes the same `&mut impl Write` shape [`crate::ext_fire::run`] uses, so a unit test
/// can assert on the report without printing to the process's real stdout.
pub(crate) fn run(dir: &Path, mut flags: ExtFlags, stdout: &mut impl Write) -> i32 {
    let variables = Variables::from_pairs(std::mem::take(&mut flags.vars));
    let events = std::mem::take(&mut flags.events);
    let ceiling = match resolve_ceiling(flags) {
        Ok(ceiling) => ceiling,
        // `--memory-limit`/`--instruction-limit` are parsed by `clap` before `resolve_ceiling`
        // ever runs, and `resolve_ceiling` always starts from `PolicyName::Confined`, which
        // `Ceiling::new` accepts — so `Error::CeilingUnbounded` cannot reach this arm today. It
        // stays because the signature keeps that library check visible instead of an `expect`.
        Err(error) => {
            eprintln!("airsl ext doctor: {error}");
            return 1;
        }
    };
    let manifest = match Manifest::from_dir(dir, &variables) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("airsl ext doctor: {error}");
            return 1;
        }
    };
    let modules = match airsl::modules::stdlib() {
        Ok(modules) => modules,
        Err(error) => {
            eprintln!("airsl ext doctor: {error}");
            return 1;
        }
    };

    let negotiation = negotiate(&manifest, &ceiling, &modules);
    let decision = ManifestApprover.decide(&ApprovalRequest::new(dir, &manifest, &negotiation));
    let _ = write!(
        stdout,
        "{}",
        render(&manifest, &ceiling, &negotiation, &decision, &events)
    );
    0
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use airsl::extension::{Approver as _, Ceiling, MANIFEST_FILE};
    use airsl::modules::stdlib;
    use airsl::{GrantSet, MemoryLimit, Policy};
    use tempfile::TempDir;

    use super::{
        ApprovalRequest, KIND_WIDTH, Manifest, ManifestApprover, VALUE_WIDTH, Variables, negotiate,
        render, run,
    };
    use crate::cli::ExtFlags;

    /// Writes `extension.toml` (built from `required`/`optional`/`limits` bodies) plus a
    /// `main.lua` that would leave a `marker` file behind if it were ever executed.
    fn fixture(name: &str, required: &str, optional: &str, limits: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join(MANIFEST_FILE),
            format!(
                "[extension]\nname='{name}'\nversion='0.2.0'\nentry='main.lua'\napi=1\n\
                 [capabilities]\n{required}\n[capabilities.optional]\n{optional}\n\
                 [limits]\n{limits}\n"
            ),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("main.lua"),
            "airsstack.fs.write('marker', 'ran')\n",
        )
        .unwrap();
        dir
    }

    fn manifest(dir: &TempDir) -> Manifest {
        Manifest::from_dir(dir.path(), &Variables::none()).unwrap()
    }

    /// Runs [`run`] against `dir` and returns `(exit code, stdout)`, the same shape
    /// [`crate::ext_fire::tests::fire`] uses.
    fn doctor(dir: &std::path::Path, flags: ExtFlags) -> (i32, String) {
        let mut out = Vec::new();
        let code = run(dir, flags, &mut out);
        (code, String::from_utf8(out).unwrap())
    }

    #[test]
    fn render_tags_each_request_and_ends_with_the_decision() {
        // `limits` requests a tighter memory ceiling than the host offers, so `ceiling:` and
        // `negotiated:` carry different memory values below — a swap of the two `policy_block`
        // arguments in `render` would leave every value present but under the wrong label, which
        // the whole-output comparison below catches and an independent `contains` check would not.
        //
        // `abs`, not the unix-spelled literals directly: `/data/in` has a root but no drive, so
        // the manifest validator refuses it on Windows before negotiation ever runs, and the
        // ceiling's own grant has to resolve onto the same drive the manifest value does or the
        // two would silently stop matching.
        let read_root = crate::test_support::abs("/data");
        let read_requested = crate::test_support::abs("/data/in");
        let dir = fixture(
            "journal-indexer",
            &format!("fs.read=['{read_requested}']\nproc.run=['git']\nregex=true"),
            "proc.run=['tar']",
            "memory='8MB'",
        );
        let m = manifest(&dir);
        let ceiling = Ceiling::new(
            Policy::confined().with_grants(
                GrantSet::declared()
                    .with_fs(|fs| fs.read(read_root.as_str()))
                    .with_proc(|p| p.allow(["git"])),
            ),
        )
        .unwrap();
        let negotiation = negotiate(&m, &ceiling, &stdlib().unwrap());
        assert!(negotiation.is_satisfied(), "{:?}", negotiation.denied());
        let decision = ManifestApprover.decide(&ApprovalRequest::new(dir.path(), &m, &negotiation));

        let out = render(&m, &ceiling, &negotiation, &decision, &[]);

        // The `fs.read` request line's value column is built with the same width the production
        // format string uses, rather than hand-counted padding, so it stays correct however long
        // `read_requested` is on the platform actually running the test.
        let requested_fs_read = format!(
            "  {:<w1$} {:<w2$} granted\n",
            "fs.read",
            read_requested,
            w1 = KIND_WIDTH,
            w2 = VALUE_WIDTH,
        );

        assert_eq!(
            out,
            format!(
                concat!(
                    "extension:    journal-indexer 0.2.0 (api 1, entry main.lua)\n",
                    "events:       none\n",
                    "ceiling:\n",
                    "  language:     restricted\n",
                    "  grants:       read {}; exec git\n",
                    "  memory:       67108864 bytes\n",
                    "  instructions: 100000000 instructions\n",
                    "negotiated:\n",
                    "  language:     restricted\n",
                    "  grants:       read {}; exec git\n",
                    "  memory:       8388608 bytes\n",
                    "  instructions: 100000000 instructions\n",
                    "requested:\n",
                    "{}",
                    "  proc.run     git                      granted\n",
                    "  module       regex                    granted\n",
                    "  proc.run     tar                      reduced   (not among the granted executables: git)\n",
                    "decision:     approve\n",
                ),
                read_root, read_requested, requested_fs_read
            )
        );
    }

    #[test]
    fn render_lists_the_declared_events_sorted_and_deduplicated() {
        let dir = fixture("t", "", "", "");
        let m = manifest(&dir);
        let ceiling = Ceiling::new(Policy::confined()).unwrap();
        let negotiation = negotiate(&m, &ceiling, &stdlib().unwrap());
        let decision = ManifestApprover.decide(&ApprovalRequest::new(dir.path(), &m, &negotiation));
        let events = ["stop".to_owned(), "count".to_owned(), "count".to_owned()];

        let out = render(&m, &ceiling, &negotiation, &decision, &events);

        assert!(out.contains("events:       count, stop\n"), "{out}");
    }

    #[test]
    fn a_required_denial_is_listed_and_the_decision_is_deny() {
        // `abs`, not the unix-spelled `/` directly: a bare `/` has a root but no drive, so the
        // manifest validator refuses it before negotiation ever runs on Windows.
        let denied = crate::test_support::abs("/");
        let dir = fixture("t", &format!("fs.read=['{denied}']"), "", "");
        let m = manifest(&dir);
        let ceiling = Ceiling::new(Policy::confined()).unwrap();
        let negotiation = negotiate(&m, &ceiling, &stdlib().unwrap());
        assert!(!negotiation.is_satisfied());
        let decision = ManifestApprover.decide(&ApprovalRequest::new(dir.path(), &m, &negotiation));

        let out = render(&m, &ceiling, &negotiation, &decision, &[]);

        let requested_fs_read = format!(
            "  {:<w1$} {:<w2$} denied    (",
            "fs.read",
            denied,
            w1 = KIND_WIDTH,
            w2 = VALUE_WIDTH,
        );
        assert!(out.contains(&requested_fs_read), "{out}");
        assert!(
            out.contains(&format!("decision:     deny — fs.read `{denied}`:")),
            "{out}"
        );
    }

    #[test]
    fn a_denial_is_tagged_correctly_when_the_requested_root_resolves_to_a_different_path() {
        // A `TempDir` on macOS lives under `/var`, which `canonicalize` resolves to `/private/var`
        // — the same divergence `FsGrant::resolve_root` exists to paper over. The requested
        // subdirectory must actually exist, or `canonicalize` fails and falls back to an
        // absolute-but-unresolved path, which would hide the very divergence this test pins: that
        // `push_block`'s `Capability::FsRead(resolve_root(root))` compares equal to the one
        // `negotiate` denies only because both sides resolve the root the same way.
        let dir = TempDir::new().unwrap();
        let requested = dir.path().join("data");
        std::fs::create_dir(&requested).unwrap();
        std::fs::write(
            dir.path().join(MANIFEST_FILE),
            format!(
                "[extension]\nname='t'\nversion='0.2.0'\nentry='main.lua'\napi=1\n\
                 [capabilities]\nfs.read=['{}']\n[capabilities.optional]\n[limits]\n",
                // `script_literal`, not `requested.display()`: a temp path on Windows carries
                // backslashes, which a TOML basic string reads as escape sequences, so a raw
                // `display()` would fail to parse instead of exercising this test.
                crate::test_support::script_literal(&requested)
            ),
        )
        .unwrap();
        std::fs::write(dir.path().join("main.lua"), "return 1").unwrap();
        let m = Manifest::from_dir(dir.path(), &Variables::none()).unwrap();
        // No read grant at all — the ceiling denies every `fs.read` request, including this one.
        let ceiling = Ceiling::new(Policy::confined()).unwrap();
        let negotiation = negotiate(&m, &ceiling, &stdlib().unwrap());
        assert!(!negotiation.is_satisfied(), "{:?}", negotiation.denied());
        let decision = ManifestApprover.decide(&ApprovalRequest::new(dir.path(), &m, &negotiation));

        let out = render(&m, &ceiling, &negotiation, &decision, &[]);

        // On macOS this is `/private/var/...` where `requested` was `/var/...` — the divergence
        // this test exists to pin. On a platform where the temp root is already canonical the two
        // are equal, and the assertion below still holds: it names whichever form `negotiate` and
        // `push_block` actually agreed on, proving the two sides stayed in sync either way.
        let resolved = airsl::FsGrant::resolve_root(&requested);
        assert!(
            out.contains(&format!("fs.read      {}", resolved.display())),
            "{out}"
        );
        assert!(out.contains(" denied    ("), "{out}");
    }

    #[test]
    fn a_doctor_run_does_not_execute_the_entry_and_writes_the_report_to_stdout() {
        let dir = fixture("t", "", "", "");
        let (code, out) = doctor(dir.path(), ExtFlags::default());
        assert_eq!(code, 0);
        assert!(!dir.path().join("marker").exists());
        assert!(out.starts_with("extension:"), "{out}");
    }

    #[test]
    fn run_exits_zero_on_a_denial_and_one_on_a_parse_error() {
        // `abs`, not the unix-spelled `/` directly: see `a_required_denial_is_listed_and_the_decision_is_deny`.
        let denied = fixture(
            "t",
            &format!("fs.read=['{}']", crate::test_support::abs("/")),
            "",
            "",
        );
        assert_eq!(doctor(denied.path(), ExtFlags::default()).0, 0);

        let unparsable = TempDir::new().unwrap();
        std::fs::write(unparsable.path().join(MANIFEST_FILE), "not = = toml").unwrap();
        assert_eq!(doctor(unparsable.path(), ExtFlags::default()).0, 1);
    }

    #[test]
    fn limits_are_reported_as_the_minimum_in_the_negotiated_block_and_the_ceiling_is_unchanged() {
        let dir = fixture("t", "", "", "memory='8MB'");
        let m = manifest(&dir);
        let ceiling = Ceiling::new(Policy::confined()).unwrap();
        assert_eq!(
            ceiling.policy().limits().memory(),
            Some(MemoryLimit::mebibytes(64))
        );
        let negotiation = negotiate(&m, &ceiling, &stdlib().unwrap());
        let decision = ManifestApprover.decide(&ApprovalRequest::new(dir.path(), &m, &negotiation));

        let out = render(&m, &ceiling, &negotiation, &decision, &[]);

        // A whole-output comparison, not independent `contains` checks: swapping the `ceiling`
        // and `negotiated` arguments at the `policy_block` call sites in `render` would leave
        // every `contains` assertion true (both blocks would still have *a* 67108864 and *a*
        // 8388608 memory line, just under the other label) — this pins which value sits under
        // which label.
        assert_eq!(
            out,
            concat!(
                "extension:    t 0.2.0 (api 1, entry main.lua)\n",
                "events:       none\n",
                "ceiling:\n",
                "  language:     restricted\n",
                "  grants:       none\n",
                "  memory:       67108864 bytes\n",
                "  instructions: 100000000 instructions\n",
                "negotiated:\n",
                "  language:     restricted\n",
                "  grants:       none\n",
                "  memory:       8388608 bytes\n",
                "  instructions: 100000000 instructions\n",
                "requested:\n",
                "decision:     approve\n",
            )
        );
    }

    #[test]
    fn a_missing_manifest_exits_one() {
        let dir = TempDir::new().unwrap();
        assert_eq!(doctor(dir.path(), ExtFlags::default()).0, 1);
    }

    #[test]
    fn a_var_flag_resolves_a_manifest_variable() {
        let dir = fixture("t", "fs.read=['$APP_HOME/data']", "", "");

        // Without `--var`, the manifest cannot resolve `$APP_HOME` and `run` reports the read
        // failure rather than a negotiation.
        let unresolved = Manifest::from_dir(dir.path(), &Variables::none());
        assert!(
            matches!(unresolved, Err(ref error) if error.to_string().contains("$APP_HOME")),
            "{unresolved:?}"
        );
        assert_eq!(doctor(dir.path(), ExtFlags::default()).0, 1);

        // With `--var APP_HOME=<dir>`, the manifest resolves and `doctor` reports a negotiation
        // (exit 0 regardless of what it grants — a report is not a failure).
        let flags = ExtFlags {
            vars: vec![("APP_HOME".to_owned(), dir.path().display().to_string())],
            ..ExtFlags::default()
        };
        assert_eq!(doctor(dir.path(), flags).0, 0);
    }
}
