//! What a script is allowed to reach, and what it may consume doing so.
//!
//! Exists as one type composing three because the three axes are genuinely independent. A script
//! may need the full language surface and a tight memory ceiling, or a minimal surface and no
//! ceiling at all, and a single enum over the combinations would have to enumerate their product
//! to say any of it.
//!
//! Responsibilities:
//!
//! - [`Policy`], composing the language surface, the grants and the resource limits.
//! - The three presets, which are what most callers should use.
//!
//! Non-responsibilities: applying any of it. [`crate::EngineBuilder`] reads a policy while building
//! the state; nothing here touches the VM.

use crate::sandbox::grant_set::GrantSet;
use crate::sandbox::language_surface::LanguageSurface;
use crate::sandbox::resource_limits::{InstructionLimit, MemoryLimit, ResourceLimits};

/// Memory ceiling the confined preset imposes.
const CONFINED_MEMORY: MemoryLimit = MemoryLimit::mebibytes(64);

/// Instruction ceiling the confined preset imposes.
const CONFINED_INSTRUCTIONS: InstructionLimit = InstructionLimit::count(100_000_000);

/// Memory ceiling the pure preset imposes.
const PURE_MEMORY: MemoryLimit = MemoryLimit::mebibytes(16);

/// Instruction ceiling the pure preset imposes.
const PURE_INSTRUCTIONS: InstructionLimit = InstructionLimit::count(10_000_000);

/// What a script may reach, and what it may spend.
///
/// Build one from a preset and adjust it if needed. There is no builder: once the presets exist a
/// policy has no field that must be chosen, so every combination is one wither away.
///
/// # Examples
///
/// ```
/// use airsl::{Policy, ResourceLimits};
///
/// let policy = Policy::confined();
/// assert!(policy.limits().memory().is_some());
///
/// let unbounded = Policy::confined().with_limits(ResourceLimits::none());
/// assert!(unbounded.limits().memory().is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    language: LanguageSurface,
    grants: GrantSet,
    limits: ResourceLimits,
}

impl Policy {
    /// Every safe Lua library, unrestricted grants, no ceilings.
    ///
    /// For first-party code trusted to the same degree as the host. A script under this policy can
    /// read and write arbitrary files and spawn processes without going through a host module.
    #[must_use]
    pub const fn trusted() -> Self {
        Self {
            language: LanguageSurface::Full,
            grants: GrantSet::unrestricted(),
            limits: ResourceLimits::none(),
        }
    }

    /// The default: a restricted language surface, declared grants only, and both ceilings.
    ///
    /// What a script written by someone other than the host author should run under.
    #[must_use]
    pub const fn confined() -> Self {
        Self {
            language: LanguageSurface::Restricted,
            grants: GrantSet::declared(),
            limits: ResourceLimits::new(Some(CONFINED_MEMORY), Some(CONFINED_INSTRUCTIONS)),
        }
    }

    /// A minimal language surface, no grants, and tight ceilings.
    ///
    /// For evaluating configuration, expressions and generated snippets. This is the configuration
    /// whose guarantees are strongest and easiest to state, which makes it the right target for
    /// adversarial testing.
    #[must_use]
    pub const fn pure() -> Self {
        Self {
            language: LanguageSurface::Minimal,
            grants: GrantSet::declared(),
            limits: ResourceLimits::new(Some(PURE_MEMORY), Some(PURE_INSTRUCTIONS)),
        }
    }

    /// The same policy on a different language surface.
    #[must_use]
    pub const fn with_language(mut self, language: LanguageSurface) -> Self {
        self.language = language;
        self
    }

    /// The same policy with different grants.
    ///
    /// Not `const`, unlike its siblings: a [`GrantSet`] owns its allowlists, so replacing one
    /// drops the old value and a const context cannot run a destructor.
    #[must_use]
    pub fn with_grants(mut self, grants: GrantSet) -> Self {
        self.grants = grants;
        self
    }

    /// The same policy with different ceilings.
    #[must_use]
    pub const fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = limits;
        self
    }

    /// The language surface this policy selects.
    #[must_use]
    pub const fn language(&self) -> LanguageSurface {
        self.language
    }

    /// The grants this policy extends.
    #[must_use]
    pub const fn grants(&self) -> &GrantSet {
        &self.grants
    }

    /// The ceilings this policy imposes.
    #[must_use]
    pub const fn limits(&self) -> &ResourceLimits {
        &self.limits
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::confined()
    }
}

#[cfg(test)]
mod tests {
    use super::Policy;
    use crate::sandbox::grant_set::GrantSet;
    use crate::sandbox::language_surface::LanguageSurface;
    use crate::sandbox::resource_limits::{MemoryLimit, ResourceLimits};

    #[test]
    fn confined_is_the_default_policy() {
        assert_eq!(Policy::default(), Policy::confined());
    }

    #[test]
    fn trusted_waives_the_surface_the_grants_and_the_ceilings() {
        let policy = Policy::trusted();
        assert_eq!(policy.language(), LanguageSurface::Full);
        assert!(policy.grants().is_unrestricted());
        assert_eq!(*policy.limits(), ResourceLimits::none());
    }

    #[test]
    fn confined_restricts_the_surface_and_imposes_both_ceilings() {
        let policy = Policy::confined();
        assert_eq!(policy.language(), LanguageSurface::Restricted);
        assert!(policy.grants().is_empty());
        assert!(policy.limits().memory().is_some());
        assert!(policy.limits().instructions().is_some());
    }

    #[test]
    fn pure_is_tighter_than_confined_on_every_axis_that_can_be() {
        let pure = Policy::pure();
        let confined = Policy::confined();
        assert_eq!(pure.language(), LanguageSurface::Minimal);
        assert!(pure.limits().memory() < confined.limits().memory());
        assert!(pure.limits().instructions() < confined.limits().instructions());
    }

    #[test]
    fn each_wither_replaces_one_axis_and_leaves_the_others() {
        let policy = Policy::confined().with_language(LanguageSurface::Minimal);
        assert_eq!(policy.language(), LanguageSurface::Minimal);
        assert_eq!(policy.limits(), Policy::confined().limits());

        let policy = Policy::confined().with_grants(GrantSet::unrestricted());
        assert!(policy.grants().is_unrestricted());
        assert_eq!(policy.language(), LanguageSurface::Restricted);

        let policy = Policy::confined().with_limits(ResourceLimits::none());
        assert_eq!(*policy.limits(), ResourceLimits::none());
        assert_eq!(policy.language(), LanguageSurface::Restricted);
    }

    #[test]
    fn a_policy_can_be_tightened_beyond_any_preset() {
        let policy = Policy::pure()
            .with_limits(ResourceLimits::none().with_memory(Some(MemoryLimit::bytes(1))));
        assert_eq!(policy.limits().memory().map(MemoryLimit::get), Some(1));
    }
}
