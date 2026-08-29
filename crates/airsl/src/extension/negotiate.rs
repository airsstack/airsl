//! Intersecting a manifest's request with the host's ceiling.
//!
//! Its own module because this is the one place the rule "a request is a maximum the host may
//! grant, never an entitlement" is written down. Each rule reuses the predicate the runtime
//! already enforces — a read root is granted by the same `allows_read` that `fs.read` will
//! check at call time — so what negotiation grants and what the module honours cannot drift.
//!
//! Negotiation is advisory; the runtime guard is authoritative. A requested path that does not
//! exist yet is checked lexically, so a symlink component pointing out of the root passes here
//! and is refused by the module's canonicalising guard at the first call. Nothing negotiated
//! here can widen what the engine enforces.
//!
//! Responsibilities: [`Capability`], [`Denial`], [`Reduction`], [`Negotiation`], and
//! [`negotiate`].
//!
//! Non-responsibilities: deciding whether a reduced extension should load. That is the
//! [`Approver`](crate::extension::approver::Approver) trait, and the host's fail-closed check.

use std::path::PathBuf;

use crate::extension::ceiling::Ceiling;
use crate::extension::manifest::{CapabilityRequest, Manifest};
use crate::modules::ModuleSet;
use crate::sandbox::{GrantSet, Policy, ResourceLimits};
use crate::types::ModuleName;

/// One thing a manifest can ask for.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Capability {
    /// A filesystem read root.
    FsRead(PathBuf),
    /// A filesystem write root.
    FsWrite(PathBuf),
    /// An executable name.
    ProcRun(String),
    /// An environment variable name.
    EnvRead(String),
    /// Presence of a host module.
    Module(ModuleName),
}

impl core::fmt::Display for Capability {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FsRead(p) => write!(f, "fs.read `{}`", p.display()),
            Self::FsWrite(p) => write!(f, "fs.write `{}`", p.display()),
            Self::ProcRun(x) => write!(f, "proc.run `{x}`"),
            Self::EnvRead(n) => write!(f, "env.read `{n}`"),
            Self::Module(m) => write!(f, "module `{m}`"),
        }
    }
}

/// A required capability the ceiling does not cover. Any one of these fails the load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Denial {
    capability: Capability,
    detail: String,
}

impl Denial {
    /// What was asked for.
    #[must_use]
    pub const fn capability(&self) -> &Capability {
        &self.capability
    }

    /// Why it was not granted, naming what the ceiling does allow.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl core::fmt::Display for Denial {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.capability, self.detail)
    }
}

/// An optional capability the ceiling does not cover. Recorded, never fatal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reduction {
    capability: Capability,
    detail: String,
}

impl Reduction {
    /// What was asked for.
    #[must_use]
    pub const fn capability(&self) -> &Capability {
        &self.capability
    }

    /// Why it was not granted.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl core::fmt::Display for Reduction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.capability, self.detail)
    }
}

/// The outcome of intersecting a request with a ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Negotiation {
    policy: Policy,
    reduced: Vec<Reduction>,
    denied: Vec<Denial>,
}

impl Negotiation {
    /// The policy an engine for this extension would be built with.
    #[must_use]
    pub const fn policy(&self) -> &Policy {
        &self.policy
    }

    /// Optional capabilities that were not granted.
    #[must_use]
    pub fn reduced(&self) -> &[Reduction] {
        &self.reduced
    }

    /// Required capabilities that were not granted.
    #[must_use]
    pub fn denied(&self) -> &[Denial] {
        &self.denied
    }

    /// Whether every required capability was granted.
    #[must_use]
    pub const fn is_satisfied(&self) -> bool {
        self.denied.is_empty()
    }
}

/// Where a miss is recorded.
#[derive(Debug, Clone, Copy)]
enum Block {
    Required,
    Optional,
}

/// Accumulates grants and misses across both request blocks.
struct Intersection<'a> {
    ceiling: &'a Ceiling,
    modules: &'a ModuleSet,
    grants: GrantSet,
    reduced: Vec<Reduction>,
    denied: Vec<Denial>,
}

impl Intersection<'_> {
    fn miss(&mut self, block: Block, capability: Capability, detail: String) {
        match block {
            Block::Required => self.denied.push(Denial { capability, detail }),
            Block::Optional => self.reduced.push(Reduction { capability, detail }),
        }
    }

    fn apply(&mut self, block: Block, request: &CapabilityRequest) {
        let ceiling_grants = self.ceiling.policy().grants();

        for root in request.fs_read() {
            // Canonicalise the way the grant itself would, so the containment check sees the
            // same spelling the module will.
            let resolved = crate::sandbox::grants::resolve_root(root.clone());
            if ceiling_grants.fs().allows_read(&resolved) {
                self.grants = std::mem::take(&mut self.grants).with_fs(|fs| fs.read(&resolved));
            } else {
                self.miss(
                    block,
                    Capability::FsRead(resolved),
                    format!(
                        "outside the granted read roots: {}",
                        roots(ceiling_grants.fs().read_roots())
                    ),
                );
            }
        }

        for root in request.fs_write() {
            let resolved = crate::sandbox::grants::resolve_root(root.clone());
            if ceiling_grants.fs().allows_write(&resolved) {
                self.grants = std::mem::take(&mut self.grants).with_fs(|fs| fs.write(&resolved));
            } else {
                self.miss(
                    block,
                    Capability::FsWrite(resolved),
                    format!(
                        "outside the granted write roots: {}",
                        roots(ceiling_grants.fs().write_roots())
                    ),
                );
            }
        }

        for program in request.proc_run() {
            if ceiling_grants.proc().allows(program) {
                self.grants = std::mem::take(&mut self.grants).with_proc(|p| p.allow([program]));
            } else {
                self.miss(
                    block,
                    Capability::ProcRun(program.to_owned()),
                    format!(
                        "not among the granted executables: {}",
                        list(ceiling_grants.proc().executables())
                    ),
                );
            }
        }

        for name in request.env_read() {
            if ceiling_grants.env().allows(name) {
                self.grants = std::mem::take(&mut self.grants).with_env(|e| e.read([name]));
            } else {
                self.miss(
                    block,
                    Capability::EnvRead(name.to_owned()),
                    format!(
                        "not among the granted variables: {}",
                        list(ceiling_grants.env().names())
                    ),
                );
            }
        }

        // Module requests are presence assertions: a hit adds nothing to the policy, only a miss
        // is recorded.
        for module in request.modules() {
            if !self.modules.contains(module) {
                self.miss(
                    block,
                    Capability::Module(module.clone()),
                    "this runtime does not install it".to_owned(),
                );
            }
        }
    }
}

fn roots(paths: &[PathBuf]) -> String {
    if paths.is_empty() {
        return String::from("none");
    }
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn list<'a>(items: impl Iterator<Item = &'a str>) -> String {
    let joined = items.collect::<Vec<_>>().join(", ");
    if joined.is_empty() {
        String::from("none")
    } else {
        joined
    }
}

/// Intersects `manifest`'s request with `ceiling`.
///
/// Required misses become denials, optional misses become reductions, and every grant from
/// either block lands in the returned policy. The language surface is the ceiling's; the
/// limits are the lower of request and ceiling on each axis.
#[must_use]
pub fn negotiate(manifest: &Manifest, ceiling: &Ceiling, modules: &ModuleSet) -> Negotiation {
    let mut work = Intersection {
        ceiling,
        modules,
        grants: GrantSet::declared(),
        reduced: Vec::new(),
        denied: Vec::new(),
    };
    work.apply(Block::Required, manifest.required());
    work.apply(Block::Optional, manifest.optional());

    let policy = Policy::confined()
        .with_language(ceiling.policy().language())
        .with_grants(work.grants)
        .with_limits(lower_limits(manifest.limits(), ceiling.policy().limits()));

    Negotiation {
        policy,
        reduced: work.reduced,
        denied: work.denied,
    }
}

/// The lower of two optional ceilings on each axis; an unlimited ceiling lets the request stand.
fn lower_limits(
    request: &crate::extension::manifest::LimitRequest,
    ceiling: &ResourceLimits,
) -> ResourceLimits {
    let memory = match (request.memory(), ceiling.memory()) {
        (Some(r), Some(c)) => Some(r.min(c)),
        (None, c) => c,
        (r, None) => r,
    };
    let instructions = match (request.instructions(), ceiling.instructions()) {
        (Some(r), Some(c)) => Some(r.min(c)),
        (None, c) => c,
        (r, None) => r,
    };
    ResourceLimits::new(memory, instructions)
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use std::path::Path;

    use super::{Capability, negotiate};
    use crate::extension::ceiling::Ceiling;
    use crate::extension::manifest::{MANIFEST_FILE, Manifest};
    use crate::extension::variables::Variables;
    use crate::modules::stdlib;
    use crate::{GrantSet, InstructionLimit, MemoryLimit, Policy, ResourceLimits};

    /// Writes a manifest whose `[capabilities]` body is `required` and optional body is
    /// `optional`, into a fresh directory, and loads it with `$HOME` = the directory.
    fn manifest(dir: &Path, required: &str, optional: &str, limits: &str) -> Manifest {
        std::fs::write(dir.join("main.lua"), "return 1").unwrap();
        std::fs::write(
            dir.join(MANIFEST_FILE),
            format!(
                "[extension]\nname='t'\nversion='1'\nentry='main.lua'\napi=1\n\
                 [capabilities]\n{required}\n[capabilities.optional]\n{optional}\n[limits]\n{limits}\n"
            ),
        )
        .unwrap();
        let vars = Variables::none().with("HOME", dir.display().to_string());
        Manifest::from_dir(dir, &vars).unwrap()
    }

    fn ceiling(dir: &Path) -> Ceiling {
        Ceiling::new(
            Policy::confined().with_grants(
                GrantSet::declared()
                    .with_fs(|fs| fs.read(dir).write(dir.join("out")))
                    .with_proc(|p| p.allow(["git", "tar"]))
                    .with_env(|e| e.read(["HOME"])),
            ),
        )
        .unwrap()
    }

    #[test]
    fn a_request_inside_the_ceiling_is_granted_in_full() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("out")).unwrap();
        let m = manifest(
            dir.path(),
            "fs.read=['$HOME']\nfs.write=['$HOME/out']\nproc.run=['git']\nenv.read=['HOME']",
            "",
            "",
        );
        let n = negotiate(&m, &ceiling(dir.path()), &stdlib().unwrap());
        assert!(n.is_satisfied(), "{:?}", n.denied());
        assert!(n.reduced().is_empty());
        let grants = n.policy().grants();
        assert_eq!(grants.fs().read_roots().len(), 1);
        assert_eq!(grants.fs().write_roots().len(), 1);
        assert!(grants.proc().allows("git"));
        assert!(grants.env().allows("HOME"));
    }

    #[test]
    fn a_required_read_outside_the_ceiling_is_denied_naming_the_roots() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest(dir.path(), "fs.read=['/']", "", "");
        let n = negotiate(&m, &ceiling(dir.path()), &stdlib().unwrap());
        assert_eq!(n.denied().len(), 1);
        assert!(matches!(n.denied()[0].capability(), Capability::FsRead(p) if p == Path::new("/")));
        assert!(
            n.denied()[0].detail().contains("granted read roots"),
            "{}",
            n.denied()[0]
        );
        assert!(n.policy().grants().fs().read_roots().is_empty());
    }

    #[test]
    fn a_write_is_checked_against_write_roots_not_read_roots() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest(dir.path(), "fs.write=['$HOME']", "", "");
        let n = negotiate(&m, &ceiling(dir.path()), &stdlib().unwrap());
        assert!(matches!(n.denied()[0].capability(), Capability::FsWrite(_)));
        assert!(
            n.denied()[0].to_string().contains("fs.write"),
            "{}",
            n.denied()[0]
        );
    }

    #[test]
    fn an_optional_miss_is_a_reduction_and_the_rest_still_lands() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest(
            dir.path(),
            "proc.run=['git']",
            "proc.run=['curl']\nenv.read=['HOME']",
            "",
        );
        let n = negotiate(&m, &ceiling(dir.path()), &stdlib().unwrap());
        assert!(n.is_satisfied());
        assert_eq!(n.reduced().len(), 1);
        assert!(matches!(n.reduced()[0].capability(), Capability::ProcRun(p) if p == "curl"));
        assert!(n.reduced()[0].detail().contains("granted executables"));
        assert!(
            n.reduced()[0].to_string().contains("proc.run `curl`"),
            "{}",
            n.reduced()[0]
        );
        assert!(n.policy().grants().proc().allows("git"));
        assert!(!n.policy().grants().proc().allows("curl"));
        assert!(
            n.policy().grants().env().allows("HOME"),
            "optional grants that fit still land"
        );
    }

    #[test]
    fn proc_and_env_misses_name_what_was_granted() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest(dir.path(), "proc.run=['rm']\nenv.read=['SECRET']", "", "");
        let n = negotiate(&m, &ceiling(dir.path()), &stdlib().unwrap());
        let texts: Vec<String> = n.denied().iter().map(ToString::to_string).collect();
        assert!(texts[0].contains("git, tar"), "{texts:?}");
        assert!(texts[1].contains("HOME"), "{texts:?}");
    }

    #[test]
    fn an_empty_ceiling_says_none_rather_than_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest(dir.path(), "proc.run=['git']", "", "");
        let empty = Ceiling::new(Policy::confined()).unwrap();
        let n = negotiate(&m, &empty, &stdlib().unwrap());
        assert!(
            n.denied()[0].detail().ends_with("none"),
            "{}",
            n.denied()[0]
        );
    }

    #[test]
    fn the_language_surface_is_the_ceilings() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest(dir.path(), "", "", "");
        let n = negotiate(
            &m,
            &Ceiling::new(Policy::pure()).unwrap(),
            &stdlib().unwrap(),
        );
        assert_eq!(n.policy().language(), Policy::pure().language());
    }

    #[test]
    fn a_nonexistent_path_inside_the_ceiling_is_still_granted() {
        // `resolve_root` only canonicalises what already exists, so a write root the extension
        // has not created yet must land under the ceiling on its literal spelling, not be denied
        // for failing to resolve.
        let dir = tempfile::tempdir().unwrap();
        // Canonicalised up front: on macOS `tempdir()` returns a path under `/var`, itself a
        // symlink to `/private/var`. `resolve_root`'s fallback for a non-existent path makes it
        // absolute but does not resolve symlinks, so the manifest has to be written against the
        // same canonical spelling the ceiling's existing-directory root resolves to, or the two
        // sides would never agree on what "inside" means.
        let canonical = dir.path().canonicalize().unwrap();
        let m = manifest(&canonical, "fs.read=['$HOME/does-not-exist-yet']", "", "");
        let n = negotiate(&m, &ceiling(&canonical), &stdlib().unwrap());
        assert!(n.is_satisfied(), "{:?}", n.denied());
        assert_eq!(
            n.policy().grants().fs().read_roots(),
            [canonical.join("does-not-exist-yet")]
        );
    }

    #[test]
    fn a_dot_dot_request_cannot_reach_negotiation_to_escape_the_ceiling() {
        // A manifest path spelled with `..` would canonicalise-and-fall-back to a literal `..`
        // root for a target that does not exist yet (`resolve_root`'s documented fallback), which
        // would pass the lexical `starts_with` containment check while never being reachable at
        // runtime. `Manifest::validate` closes this before negotiation ever sees such a request.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.lua"), "return 1").unwrap();
        std::fs::write(
            dir.path().join(MANIFEST_FILE),
            format!(
                "[extension]\nname='t'\nversion='1'\nentry='main.lua'\napi=1\n\
                 [capabilities]\nfs.read=['{}/../escape/nonexistent']\n",
                dir.path().display()
            ),
        )
        .unwrap();
        let vars = Variables::none().with("HOME", dir.path().display().to_string());
        assert!(Manifest::from_dir(dir.path(), &vars).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn a_symlink_pointing_out_of_the_root_is_denied_when_it_exists() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        let m = manifest(dir.path(), "fs.read=['$HOME/link']", "", "");
        let n = negotiate(&m, &ceiling(dir.path()), &stdlib().unwrap());
        assert!(
            !n.is_satisfied(),
            "canonicalisation resolves the link out of the root"
        );
    }

    #[test]
    fn limits_are_the_lower_of_request_and_ceiling_on_each_axis() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest(dir.path(), "", "", "memory='1MB'\ninstructions=500_000_000");
        let c = Ceiling::new(Policy::confined().with_limits(ResourceLimits::new(
            Some(MemoryLimit::mebibytes(64)),
            Some(InstructionLimit::count(1_000)),
        )))
        .unwrap();
        let n = negotiate(&m, &c, &stdlib().unwrap());
        assert_eq!(
            n.policy().limits().memory().unwrap().get(),
            1024 * 1024,
            "request was lower"
        );
        assert_eq!(
            n.policy().limits().instructions().unwrap().get(),
            1_000,
            "ceiling was lower"
        );
    }

    #[test]
    fn an_absent_request_takes_the_ceilings_limit_and_an_unlimited_ceiling_lets_the_request_stand()
    {
        let dir = tempfile::tempdir().unwrap();
        let absent = manifest(dir.path(), "", "", "");
        let n = negotiate(
            &absent,
            &Ceiling::new(Policy::confined()).unwrap(),
            &stdlib().unwrap(),
        );
        assert_eq!(n.policy().limits(), Policy::confined().limits());

        let asked = manifest(dir.path(), "", "", "memory='2MB'");
        let unlimited =
            Ceiling::new(Policy::confined().with_limits(ResourceLimits::none())).unwrap();
        let n = negotiate(&asked, &unlimited, &stdlib().unwrap());
        assert_eq!(n.policy().limits().memory().unwrap().get(), 2 * 1024 * 1024);
        assert!(n.policy().limits().instructions().is_none());
    }

    #[test]
    fn a_present_module_is_granted_silently_and_an_absent_one_is_denied() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest(dir.path(), "regex = true\nredis = true", "", "");
        let n = negotiate(&m, &ceiling(dir.path()), &stdlib().unwrap());
        assert_eq!(n.denied().len(), 1);
        assert!(
            matches!(n.denied()[0].capability(), Capability::Module(name) if name.as_str() == "redis")
        );
        assert!(
            n.denied()[0].to_string().contains("module `redis`"),
            "{}",
            n.denied()[0]
        );
        assert!(
            n.policy().grants().is_empty(),
            "presence assertions add nothing to the policy"
        );
    }
}
