//! What the host modules a script reaches are permitted to touch.
//!
//! Its own type, rather than a field on the policy, because a grant is parameterised and a
//! parameterised thing needs somewhere to keep its parameters. The distinction this axis exists to
//! draw is not "may this script touch files" but "may it touch *these* files", and a boolean
//! cannot express that.
//!
//! Responsibilities: [`GrantSet`], the two reaches it can describe, and the set of
//! [`HostAccess`] members it holds.
//!
//! Non-responsibilities: enforcement. A grant is a promise the host module keeps, checked inside
//! the Rust function before the operation it guards. Nothing here reaches the VM.

use crate::paths::rules::native::to_script_string;
use crate::sandbox::grants::{EnvGrant, FsGrant, ProcGrant};
use crate::sandbox::host_access::HostAccess;

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
/// Alongside the three allowlists the set holds [`HostAccess`] members, the unparameterised
/// capabilities (standard streams, clocks, the working directory, `require`, time zones). The
/// default, [`GrantSet::declared`], holds all of them and no allowlist entries. A host that wants a
/// script held to less starts from [`GrantSet::sealed`] and names what it adds:
///
/// ```
/// use airsl::{GrantSet, HostAccess, Policy};
///
/// // Built from the working directory, not a hardcoded literal: a unix-spelled absolute path has
/// // no drive on Windows, so it would resolve against whichever drive this doctest happens to run
/// // from rather than matching the paths checked below.
/// let repo = std::env::current_dir().unwrap().join("repo");
/// let index = repo.join(".index");
/// let policy = Policy::confined().with_grants(
///     GrantSet::sealed()
///         .with_access(HostAccess::Stdout)
///         .with_access(HostAccess::Require)
///         .with_fs(|fs| fs.read(&repo).write(&index))
///         .with_env(|env| env.read(["HOME"]))
///         .with_proc(|proc| proc.allow(["git"])),
/// );
/// assert!(policy.grants().allows(HostAccess::Stdout));
/// assert!(!policy.grants().allows(HostAccess::Clock));
/// assert!(policy.grants().fs().allows_read(&repo.join("src")));
/// assert!(!policy.grants().fs().allows_write(&repo.join("src")));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GrantSet {
    reach: GrantReach,
    /// One bit per [`HostAccess`] member, so every constructor stays `const`.
    access: u8,
    fs: FsGrant,
    env: EnvGrant,
    proc: ProcGrant,
}

impl GrantSet {
    /// The baseline a confined script gets: every [`HostAccess`] member and no allowlist entries.
    ///
    /// Building a restricted set on this reopens every member; start from [`GrantSet::sealed`]
    /// instead when the set must hold only what the host names.
    #[must_use]
    pub const fn declared() -> Self {
        Self {
            reach: GrantReach::Declared,
            access: HostAccess::ALL_BITS,
            fs: FsGrant::none(),
            env: EnvGrant::none(),
            proc: ProcGrant::none(),
        }
    }

    /// Nothing on any axis: no [`HostAccess`] member and no allowlist entries.
    ///
    /// The base to build a `pure`-based grant set on, because [`GrantSet::declared`] holds every
    /// member and adding to it can only widen.
    ///
    /// # Examples
    ///
    /// ```
    /// use airsl::{GrantSet, HostAccess};
    ///
    /// let grants = GrantSet::sealed().with_access(HostAccess::Clock);
    /// assert!(grants.allows(HostAccess::Clock));
    /// assert!(!grants.allows(HostAccess::Stdout));
    /// assert!(GrantSet::sealed().is_empty());
    /// ```
    #[must_use]
    pub const fn sealed() -> Self {
        Self {
            reach: GrantReach::Declared,
            access: 0,
            fs: FsGrant::none(),
            env: EnvGrant::none(),
            proc: ProcGrant::none(),
        }
    }

    /// Everything the host process itself can reach.
    ///
    /// For first-party code trusted to the same degree as the host. A module consulting this set
    /// performs no containment check at all, so the per-axis allowlists are not consulted and need
    /// not be populated. It holds every [`HostAccess`] member, so [`GrantSet::with_access`] on it
    /// changes nothing and the result still equals the original.
    #[must_use]
    pub const fn unrestricted() -> Self {
        Self {
            reach: GrantReach::Unrestricted,
            access: HostAccess::ALL_BITS,
            fs: FsGrant::none(),
            env: EnvGrant::none(),
            proc: ProcGrant::none(),
        }
    }

    /// Adds one [`HostAccess`] member; a member already present stays as it is.
    ///
    /// # Examples
    ///
    /// ```
    /// use airsl::{GrantSet, HostAccess};
    ///
    /// let once = GrantSet::sealed().with_access(HostAccess::Stdout);
    /// let twice = once.clone().with_access(HostAccess::Stdout);
    /// assert_eq!(once, twice);
    /// assert!(once.allows(HostAccess::Stdout));
    /// ```
    #[must_use]
    pub const fn with_access(mut self, member: HostAccess) -> Self {
        self.access |= member.bit();
        self
    }

    /// Whether `member` is permitted: always for an unrestricted set, otherwise only if held.
    ///
    /// # Examples
    ///
    /// ```
    /// use airsl::{GrantSet, HostAccess};
    ///
    /// assert!(GrantSet::declared().allows(HostAccess::Cwd));
    /// assert!(!GrantSet::sealed().allows(HostAccess::Cwd));
    /// assert!(GrantSet::unrestricted().allows(HostAccess::Cwd));
    /// ```
    #[must_use]
    pub const fn allows(&self, member: HostAccess) -> bool {
        self.is_unrestricted() || self.access & member.bit() != 0
    }

    /// The members this set permits, in [`HostAccess::ALL`] order; all seven when unrestricted.
    ///
    /// # Examples
    ///
    /// ```
    /// use airsl::{GrantSet, HostAccess};
    ///
    /// let grants = GrantSet::sealed()
    ///     .with_access(HostAccess::Zones)
    ///     .with_access(HostAccess::Stdin);
    /// let names: Vec<_> = grants.access().map(HostAccess::as_str).collect();
    /// assert_eq!(names, ["stdin", "zones"]);
    /// ```
    pub fn access(&self) -> impl Iterator<Item = HostAccess> {
        HostAccess::ALL
            .into_iter()
            .filter(|member| self.allows(*member))
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
    /// True for [`GrantSet::sealed`], and false once any axis carries something, including a
    /// [`HostAccess`] member — which is what makes `airsl doctor` able to say "none" honestly rather
    /// than by assumption.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self.reach, GrantReach::Declared)
            && self.access == 0
            && self.fs.is_empty()
            && self.env.is_empty()
            && self.proc.is_empty()
    }
}

impl Default for GrantSet {
    /// The same as [`GrantSet::declared`].
    fn default() -> Self {
        Self::declared()
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

        // This `Display` is what `airsl doctor` prints to a human terminal, where a Windows
        // reader might reasonably expect `C:\a` back. It renders through the script vocabulary
        // anyway: the same roots already appear `/`-spelled in the guard's refusal message and in
        // `ext.granted()`, so a native `Display` here would spell one root two ways depending on
        // which message a reader happened to hit. One vocabulary, uniformly, beats matching shell
        // convention in one of three places.
        let mut parts = Vec::new();
        if self.access != 0 {
            let names: Vec<_> = self.access().map(HostAccess::as_str).collect();
            parts.push(format!("access {}", names.join(",")));
        }
        for root in self.fs.read_roots() {
            parts.push(format!("read {}", to_script_string(root)));
        }
        for root in self.fs.write_roots() {
            parts.push(format!("write {}", to_script_string(root)));
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
    use crate::sandbox::host_access::HostAccess;

    const ALL_SEVEN: &str = "access stdin,stdout,stderr,clock,cwd,require,zones";

    #[test]
    fn declared_is_the_default_reach() {
        assert_eq!(GrantSet::default(), GrantSet::declared());
    }

    #[test]
    fn declared_holds_every_host_access_member() {
        let grants = GrantSet::declared();
        for member in HostAccess::ALL {
            assert!(grants.allows(member), "{member}");
        }
        assert!(!grants.is_empty());
        assert!(!grants.is_unrestricted());
    }

    #[test]
    fn sealed_grants_nothing_and_is_empty() {
        let grants = GrantSet::sealed();
        assert!(grants.is_empty());
        for member in HostAccess::ALL {
            assert!(!grants.allows(member), "{member}");
        }
        assert_eq!(grants.to_string(), "none");
    }

    #[test]
    fn sealed_grants_nothing_and_waives_nothing() {
        let grants = GrantSet::sealed();
        assert!(!grants.is_unrestricted());
        let root = crate::test_support::abs("/a");
        assert!(!grants.fs().allows_read(root.as_ref()));
        assert!(!grants.fs().allows_write(root.as_ref()));
        assert!(grants.fs().read_roots().is_empty());
        assert!(grants.fs().write_roots().is_empty());
        assert_eq!(grants.env().names().count(), 0);
        assert_eq!(grants.proc().executables().count(), 0);
    }

    #[test]
    fn unrestricted_waives_containment() {
        let grants = GrantSet::unrestricted();
        assert!(grants.is_unrestricted());
        assert!(!grants.is_empty());
    }

    #[test]
    fn unrestricted_allows_every_member() {
        let grants = GrantSet::unrestricted();
        for member in HostAccess::ALL {
            assert!(grants.allows(member), "{member}");
        }
        assert_eq!(grants.access().collect::<Vec<_>>(), HostAccess::ALL);
    }

    #[test]
    fn adding_a_member_to_an_unrestricted_set_changes_nothing() {
        let grants = GrantSet::unrestricted().with_access(HostAccess::Clock);
        assert_eq!(grants, GrantSet::unrestricted());
        assert_eq!(grants.access().collect::<Vec<_>>(), HostAccess::ALL);
    }

    #[test]
    fn the_two_reaches_are_distinguishable() {
        assert_ne!(GrantSet::declared(), GrantSet::unrestricted());
    }

    #[test]
    fn declared_and_sealed_are_distinguishable() {
        assert_ne!(GrantSet::declared(), GrantSet::sealed());
    }

    #[test]
    fn with_access_adds_exactly_the_named_member() {
        let grants = GrantSet::sealed().with_access(HostAccess::Clock);
        for member in HostAccess::ALL {
            assert_eq!(
                grants.allows(member),
                member == HostAccess::Clock,
                "{member}"
            );
        }
        assert!(!grants.is_empty());
    }

    #[test]
    fn with_access_is_idempotent() {
        let once = GrantSet::sealed().with_access(HostAccess::Stdout);
        let twice = once.clone().with_access(HostAccess::Stdout);
        assert_eq!(once, twice);
        assert_eq!(twice.access().collect::<Vec<_>>(), [HostAccess::Stdout]);
    }

    #[test]
    fn access_lists_members_in_declaration_order_whatever_order_they_were_added() {
        let grants = GrantSet::sealed()
            .with_access(HostAccess::Zones)
            .with_access(HostAccess::Stdin)
            .with_access(HostAccess::Cwd);
        assert_eq!(
            grants.access().collect::<Vec<_>>(),
            [HostAccess::Stdin, HostAccess::Cwd, HostAccess::Zones]
        );
    }

    #[test]
    fn each_reach_renders_for_a_report() {
        assert_eq!(GrantSet::sealed().to_string(), "none");
        assert_eq!(GrantSet::declared().to_string(), ALL_SEVEN);
        assert_eq!(GrantSet::unrestricted().to_string(), "unrestricted");
    }

    #[test]
    fn display_lists_access_first_then_the_allowlists() {
        let a = crate::test_support::abs("/a");
        let grants = GrantSet::sealed()
            .with_access(HostAccess::Stderr)
            .with_fs(|fs| fs.read(a.as_str()));
        assert_eq!(grants.to_string(), format!("access stderr; read {a}"));
    }

    #[test]
    fn display_renders_roots_through_the_script_vocabulary_not_native_spelling() {
        // `to_script_string` renders with the platform separator converted to the script's `/`,
        // same reasoning as the sibling vocabulary tests in `modules::path`: identical on this
        // unix host, where `/` is already both the native and the script separator, and proved
        // for real by `paths::rules::to_script_string`'s own flavour-taking test.
        //
        // `abs`, not the unix-spelled literals directly: a bare `/a` has a root but no drive, so
        // `FsGrant::read`'s `resolve_root` would resolve it against whichever drive the test
        // happens to run from on Windows, instead of the drive the expectation below names.
        let a = crate::test_support::abs("/a");
        let b = crate::test_support::abs("/b");
        let grants = GrantSet::declared().with_fs(|fs| fs.read(a.as_str()).write(b.as_str()));
        assert_eq!(
            grants.to_string(),
            format!("{ALL_SEVEN}; read {a}; write {b}")
        );
    }

    #[test]
    fn the_constructors_can_build_constants() {
        const EVERYTHING: GrantSet = GrantSet::declared();
        const ONLY_REQUIRE: GrantSet = GrantSet::sealed().with_access(HostAccess::Require);

        assert!(EVERYTHING.allows(HostAccess::Zones));
        assert!(ONLY_REQUIRE.allows(HostAccess::Require));
        assert!(!ONLY_REQUIRE.allows(HostAccess::Clock));
    }

    // Evaluated at compile time, so dropping `const` from `GrantSet::allows` (or from a
    // constructor) fails the build rather than slipping past the runtime asserts above. `forget`
    // because `GrantSet` has a destructor, which a const context may not run (E0493).
    const _: () = {
        let grants = GrantSet::sealed().with_access(HostAccess::Require);
        assert!(grants.allows(HostAccess::Require) && !grants.allows(HostAccess::Clock));
        core::mem::forget(grants);
    };
}
