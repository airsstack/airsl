//! What the host modules a script reaches are permitted to touch.
//!
//! Its own type, rather than a field on the policy, because a grant is parameterised and a
//! parameterised thing needs somewhere to keep its parameters. The distinction this axis exists to
//! draw is not "may this script touch files" but "may it touch *these* files", and a boolean
//! cannot express that.
//!
//! Responsibilities: [`GrantSet`] and the two reaches it can describe.
//!
//! Non-responsibilities: enforcement. A grant is a promise the host module keeps, checked inside
//! the Rust function before the operation it guards. Nothing here reaches the VM.

use crate::sandbox::grants::{EnvGrant, FsGrant, ProcGrant};

/// How far the grants in a [`GrantSet`] reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum GrantReach {
    /// Only what the host declared, and nothing beyond it.
    #[default]
    Declared,
    /// Everything the host itself can reach.
    Unrestricted,
}

/// The authority a policy extends to the host modules a script can reach.
///
/// Either unrestricted — the host trusts the script as much as itself, and no module performs a
/// containment check at all — or a set of declared, parameterised grants. The distinction the
/// declared side exists to draw is never "may this script touch files" but "may it touch *these*
/// files", which is why each axis carries its own allowlist rather than a boolean.
///
/// Nothing is granted by default. A host that wants a script to reach something says so:
///
/// ```
/// use airsl::{GrantSet, Policy};
///
/// let policy = Policy::confined().with_grants(
///     GrantSet::declared()
///         .with_fs(|fs| fs.read("/repo").write("/repo/.index"))
///         .with_env(|env| env.read(["HOME"]))
///         .with_proc(|proc| proc.allow(["git"])),
/// );
/// assert!(policy.grants().fs().allows_read(std::path::Path::new("/repo/src")));
/// assert!(!policy.grants().fs().allows_write(std::path::Path::new("/repo/src")));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct GrantSet {
    reach: GrantReach,
    fs: FsGrant,
    env: EnvGrant,
    proc: ProcGrant,
}

impl GrantSet {
    /// Only the grants the host declared, which starts as none at all.
    #[must_use]
    pub const fn declared() -> Self {
        Self {
            reach: GrantReach::Declared,
            fs: FsGrant::none(),
            env: EnvGrant::none(),
            proc: ProcGrant::none(),
        }
    }

    /// Everything the host process itself can reach.
    ///
    /// For first-party code trusted to the same degree as the host. A module consulting this set
    /// performs no containment check at all, so the per-axis allowlists are not consulted and need
    /// not be populated.
    #[must_use]
    pub const fn unrestricted() -> Self {
        Self {
            reach: GrantReach::Unrestricted,
            fs: FsGrant::none(),
            env: EnvGrant::none(),
            proc: ProcGrant::none(),
        }
    }

    /// Replaces the filesystem authority, built by the closure from what is already there.
    ///
    /// A closure rather than a value so that the common case reads as one expression and the
    /// grants compose without a caller naming the type.
    #[must_use]
    pub fn with_fs(mut self, build: impl FnOnce(FsGrant) -> FsGrant) -> Self {
        self.fs = build(self.fs);
        self
    }

    /// Replaces the environment authority.
    #[must_use]
    pub fn with_env(mut self, build: impl FnOnce(EnvGrant) -> EnvGrant) -> Self {
        self.env = build(self.env);
        self
    }

    /// Replaces the process authority.
    #[must_use]
    pub fn with_proc(mut self, build: impl FnOnce(ProcGrant) -> ProcGrant) -> Self {
        self.proc = build(self.proc);
        self
    }

    /// The filesystem authority.
    #[must_use]
    pub const fn fs(&self) -> &FsGrant {
        &self.fs
    }

    /// The environment authority.
    #[must_use]
    pub const fn env(&self) -> &EnvGrant {
        &self.env
    }

    /// The process authority.
    #[must_use]
    pub const fn proc(&self) -> &ProcGrant {
        &self.proc
    }

    /// Whether this set waives containment entirely.
    #[must_use]
    pub const fn is_unrestricted(&self) -> bool {
        matches!(self.reach, GrantReach::Unrestricted)
    }

    /// Whether this set grants nothing at all.
    ///
    /// True for a freshly declared set, and false once any axis carries something — which is what
    /// makes `airsl doctor` able to say "none" honestly rather than by assumption.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self.reach, GrantReach::Declared)
            && self.fs.is_empty()
            && self.env.is_empty()
            && self.proc.is_empty()
    }
}

impl core::fmt::Display for GrantSet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_unrestricted() {
            return f.write_str("unrestricted");
        }
        if self.is_empty() {
            return f.write_str("none");
        }

        let mut parts = Vec::new();
        for root in self.fs.read_roots() {
            parts.push(format!("read {}", root.display()));
        }
        for root in self.fs.write_roots() {
            parts.push(format!("write {}", root.display()));
        }
        if !self.env.is_empty() {
            parts.push(format!(
                "env {}",
                self.env.names().collect::<Vec<_>>().join(",")
            ));
        }
        if !self.proc.is_empty() {
            parts.push(format!(
                "exec {}",
                self.proc.executables().collect::<Vec<_>>().join(",")
            ));
        }
        f.write_str(&parts.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::GrantSet;

    #[test]
    fn declared_is_the_default_reach() {
        assert_eq!(GrantSet::default(), GrantSet::declared());
    }

    #[test]
    fn declared_grants_nothing_and_waives_nothing() {
        let grants = GrantSet::declared();
        assert!(grants.is_empty());
        assert!(!grants.is_unrestricted());
    }

    #[test]
    fn unrestricted_waives_containment() {
        let grants = GrantSet::unrestricted();
        assert!(grants.is_unrestricted());
        assert!(!grants.is_empty());
    }

    #[test]
    fn the_two_reaches_are_distinguishable() {
        assert_ne!(GrantSet::declared(), GrantSet::unrestricted());
    }

    #[test]
    fn each_reach_renders_for_a_report() {
        assert_eq!(GrantSet::declared().to_string(), "none");
        assert_eq!(GrantSet::unrestricted().to_string(), "unrestricted");
    }
}
