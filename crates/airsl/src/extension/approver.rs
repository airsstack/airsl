//! Who decides whether a negotiated extension loads.
//!
//! A trait rather than an enum because the interesting approver — one that shows a person the
//! request and waits — needs a terminal, a GUI or a network round-trip, none of which this
//! library should own. The two shipped implementations carry no state and no I/O. An approver
//! can only narrow: the host refuses any negotiation with a denial before the approver is
//! consulted, so an implementation that says `Approve` to everything is `ManifestApprover`,
//! not a hole.
//!
//! Responsibilities: [`Approver`], [`ApprovalRequest`], [`Decision`], [`ManifestApprover`],
//! [`DenyAll`].
//!
//! Non-responsibilities: the fail-closed check. That is the host's, and it runs first.

use std::path::Path;

use crate::extension::manifest::Manifest;
use crate::extension::negotiate::Negotiation;

/// What an approver sees.
#[derive(Debug, Clone, Copy)]
pub struct ApprovalRequest<'a> {
    dir: &'a Path,
    manifest: &'a Manifest,
    negotiation: &'a Negotiation,
}

impl<'a> ApprovalRequest<'a> {
    /// Bundles the inputs.
    #[must_use]
    pub const fn new(dir: &'a Path, manifest: &'a Manifest, negotiation: &'a Negotiation) -> Self {
        Self {
            dir,
            manifest,
            negotiation,
        }
    }

    /// Where the extension's files are.
    #[must_use]
    pub const fn dir(&self) -> &'a Path {
        self.dir
    }

    /// What it asked for.
    #[must_use]
    pub const fn manifest(&self) -> &'a Manifest {
        self.manifest
    }

    /// What it would get.
    #[must_use]
    pub const fn negotiation(&self) -> &'a Negotiation {
        self.negotiation
    }
}

/// An approver's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Load it with the negotiated policy.
    Approve,
    /// Do not load it; the string is reported to the host.
    Deny(String),
}

/// Decides whether a negotiated extension loads.
pub trait Approver: Send + Sync {
    /// The decision for one extension.
    fn decide(&self, request: &ApprovalRequest<'_>) -> Decision;
}

/// Honours the manifest: approves whenever every required capability was granted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ManifestApprover;

impl Approver for ManifestApprover {
    fn decide(&self, request: &ApprovalRequest<'_>) -> Decision {
        if request.negotiation().is_satisfied() {
            Decision::Approve
        } else {
            let denials: Vec<String> = request
                .negotiation()
                .denied()
                .iter()
                .map(ToString::to_string)
                .collect();
            Decision::Deny(denials.join("; "))
        }
    }
}

/// Refuses every extension. For a host that ships the extension API but has it switched off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DenyAll;

impl Approver for DenyAll {
    fn decide(&self, _request: &ApprovalRequest<'_>) -> Decision {
        Decision::Deny(String::from("extensions are disabled"))
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]
    #![expect(
        clippy::panic,
        reason = "a wrong Decision variant is the intended failure signal"
    )]

    use super::{ApprovalRequest, Approver, Decision, DenyAll, ManifestApprover};
    use crate::Policy;
    use crate::extension::ceiling::Ceiling;
    use crate::extension::manifest::{MANIFEST_FILE, Manifest};
    use crate::extension::negotiate::negotiate;
    use crate::extension::variables::Variables;
    use crate::modules::stdlib;

    fn fixture(capabilities: &str) -> (tempfile::TempDir, Manifest) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.lua"), "return 1").unwrap();
        std::fs::write(
            dir.path().join(MANIFEST_FILE),
            format!(
                "[extension]\nname='t'\nversion='1'\nentry='main.lua'\napi=1\n[capabilities]\n{capabilities}\n"
            ),
        )
        .unwrap();
        let m = Manifest::from_dir(dir.path(), &Variables::none()).unwrap();
        (dir, m)
    }

    #[test]
    fn the_manifest_approver_approves_a_satisfied_negotiation() {
        let (dir, m) = fixture("regex = true");
        let n = negotiate(
            &m,
            &Ceiling::new(Policy::confined()).unwrap(),
            &stdlib().unwrap(),
        );
        let req = ApprovalRequest::new(dir.path(), &m, &n);
        assert_eq!(ManifestApprover.decide(&req), Decision::Approve);
    }

    #[test]
    fn the_manifest_approver_denies_listing_every_denial() {
        let (dir, m) = fixture("proc.run=['git']\nenv.read=['X']");
        let n = negotiate(
            &m,
            &Ceiling::new(Policy::confined()).unwrap(),
            &stdlib().unwrap(),
        );
        let req = ApprovalRequest::new(dir.path(), &m, &n);
        let Decision::Deny(reason) = ManifestApprover.decide(&req) else {
            panic!("expected Deny");
        };
        assert!(
            reason.contains("proc.run `git`") && reason.contains("env.read `X`"),
            "{reason}"
        );
    }

    #[test]
    fn deny_all_denies_even_a_satisfied_negotiation() {
        let (dir, m) = fixture("");
        let n = negotiate(
            &m,
            &Ceiling::new(Policy::confined()).unwrap(),
            &stdlib().unwrap(),
        );
        let req = ApprovalRequest::new(dir.path(), &m, &n);
        assert_eq!(
            DenyAll.decide(&req),
            Decision::Deny(String::from("extensions are disabled"))
        );
    }

    #[test]
    fn the_request_exposes_the_directory_for_an_interactive_approver() {
        let (dir, m) = fixture("");
        let n = negotiate(
            &m,
            &Ceiling::new(Policy::confined()).unwrap(),
            &stdlib().unwrap(),
        );
        let req = ApprovalRequest::new(dir.path(), &m, &n);
        assert_eq!(req.dir(), dir.path());
        assert_eq!(req.manifest().name().as_str(), "t");
    }
}
