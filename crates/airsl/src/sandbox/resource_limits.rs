//! Ceilings on what a script may consume before the host stops it.
//!
//! These three types live together because they are one concept and are never read apart: a
//! [`ResourceLimits`] is meaningless without knowing what its members mean, and neither member is
//! ever configured alone. They are separate from the rest of the policy because they are the only
//! axis enforced by the VM rather than by a host function — a capability grant is checked in Rust
//! before an operation, while these are checked by Lua's own allocator and instruction hook.
//!
//! Responsibilities:
//!
//! - [`MemoryLimit`] and [`InstructionLimit`], the two ceilings.
//! - [`ResourceLimits`], which composes them and applies them to a state.
//!
//! Non-responsibilities: reporting a breach. Applying a limit arms it; turning the resulting
//! failure into a named error is [`crate::Engine`]'s job.

use crate::error::{Error, Result};
use crate::instruction_budget::InstructionBudget;

/// A ceiling on the bytes a Lua state may have allocated at once.
///
/// This is a ceiling on the whole state, not a per-script budget. An engine that has run several
/// scripts carries whatever garbage they left until the collector runs, and that counts against
/// the next script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemoryLimit(usize);

impl MemoryLimit {
    /// A limit of `bytes`.
    ///
    /// Zero is raised to one byte rather than accepted. The underlying VM reads a zero limit as
    /// "unlimited", so honouring it literally would turn the tightest possible request into no
    /// limit at all — the one misreading with a genuinely dangerous outcome.
    #[must_use]
    pub const fn bytes(bytes: usize) -> Self {
        Self(if bytes == 0 { 1 } else { bytes })
    }

    /// A limit of `mib` mebibytes.
    #[must_use]
    pub const fn mebibytes(mib: usize) -> Self {
        Self::bytes(mib.saturating_mul(1024 * 1024))
    }

    /// The ceiling in bytes.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

impl core::fmt::Display for MemoryLimit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} bytes", self.0)
    }
}

/// A ceiling on the VM instructions a single evaluation may execute.
///
/// The only defence against a script that never terminates. No capability grant helps against
/// `while true do end`, because it reaches nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct InstructionLimit(u64);

impl InstructionLimit {
    /// A limit of `count` instructions.
    ///
    /// Zero is raised to one, so that the tightest possible request stops a script immediately
    /// rather than reading as no limit.
    #[must_use]
    pub const fn count(count: u64) -> Self {
        Self(if count == 0 { 1 } else { count })
    }

    /// The ceiling in instructions.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl core::fmt::Display for InstructionLimit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} instructions", self.0)
    }
}

/// The resource ceilings a policy imposes, each independently optional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResourceLimits {
    memory: Option<MemoryLimit>,
    instructions: Option<InstructionLimit>,
}

impl ResourceLimits {
    /// No ceilings at all.
    ///
    /// A script may allocate until the host process is killed and loop until it is interrupted.
    /// Appropriate only where the script is as trusted as the host.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            memory: None,
            instructions: None,
        }
    }

    /// Both ceilings, each independently optional.
    #[must_use]
    pub const fn new(memory: Option<MemoryLimit>, instructions: Option<InstructionLimit>) -> Self {
        Self {
            memory,
            instructions,
        }
    }

    /// The same ceilings with the memory ceiling replaced.
    #[must_use]
    pub const fn with_memory(mut self, memory: Option<MemoryLimit>) -> Self {
        self.memory = memory;
        self
    }

    /// The same ceilings with the instruction ceiling replaced.
    #[must_use]
    pub const fn with_instructions(mut self, instructions: Option<InstructionLimit>) -> Self {
        self.instructions = instructions;
        self
    }

    /// The memory ceiling, if there is one.
    #[must_use]
    pub const fn memory(&self) -> Option<MemoryLimit> {
        self.memory
    }

    /// The instruction ceiling, if there is one.
    #[must_use]
    pub const fn instructions(&self) -> Option<InstructionLimit> {
        self.instructions
    }

    /// Arms both ceilings on `lua`.
    ///
    /// Returns the instruction budget when there is an instruction ceiling, because the count it
    /// holds is per evaluation and the caller has to reset it between scripts. The memory ceiling
    /// needs no such handle: it is a property of the state's allocator and stays armed by itself.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EngineSetup`] when the state does not own its allocator and therefore
    /// cannot be capped, or when the instruction hook cannot be installed.
    pub(crate) fn apply(&self, lua: &mlua::Lua) -> Result<Option<InstructionBudget>> {
        if let Some(limit) = self.memory {
            lua.set_memory_limit(limit.get())
                .map_err(|source| Error::EngineSetup {
                    stage: "memory limit",
                    source: Box::new(source),
                })?;
        }

        self.instructions
            .map(|limit| InstructionBudget::install(lua, limit))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{InstructionLimit, MemoryLimit, ResourceLimits};

    #[test]
    fn a_zero_memory_limit_becomes_the_tightest_real_limit_not_unlimited() {
        assert_eq!(MemoryLimit::bytes(0).get(), 1);
    }

    #[test]
    fn a_zero_instruction_limit_becomes_the_tightest_real_limit_not_unlimited() {
        assert_eq!(InstructionLimit::count(0).get(), 1);
    }

    #[test]
    fn mebibytes_converts_to_bytes() {
        assert_eq!(MemoryLimit::mebibytes(64).get(), 67_108_864);
    }

    #[test]
    fn an_absurd_mebibyte_count_saturates_rather_than_wrapping() {
        assert_eq!(MemoryLimit::mebibytes(usize::MAX).get(), usize::MAX);
    }

    #[test]
    fn none_imposes_no_ceilings() {
        let limits = ResourceLimits::none();
        assert!(limits.memory().is_none());
        assert!(limits.instructions().is_none());
    }

    #[test]
    fn the_withers_replace_one_ceiling_and_leave_the_other() {
        let limits = ResourceLimits::none()
            .with_memory(Some(MemoryLimit::mebibytes(8)))
            .with_instructions(Some(InstructionLimit::count(500)));
        assert_eq!(limits.memory().map(MemoryLimit::get), Some(8 * 1024 * 1024));
        assert_eq!(limits.instructions().map(InstructionLimit::get), Some(500));

        let cleared = limits.with_memory(None);
        assert!(cleared.memory().is_none());
        assert_eq!(cleared.instructions().map(InstructionLimit::get), Some(500));
    }

    #[test]
    fn limits_render_with_their_unit() {
        assert_eq!(MemoryLimit::bytes(1024).to_string(), "1024 bytes");
        assert_eq!(InstructionLimit::count(99).to_string(), "99 instructions");
    }

    #[test]
    fn applying_a_memory_ceiling_caps_the_state() {
        let lua = mlua::Lua::new();
        let limits = ResourceLimits::none().with_memory(Some(MemoryLimit::mebibytes(1)));
        assert!(limits.apply(&lua).unwrap().is_none());
        assert!(
            lua.load("local t = {} for i = 1, 1e9 do t[i] = i end")
                .exec()
                .is_err()
        );
    }

    #[test]
    fn applying_an_instruction_ceiling_hands_back_the_budget_tracking_it() {
        let lua = mlua::Lua::new();
        let limits = ResourceLimits::none().with_instructions(Some(InstructionLimit::count(1000)));
        let budget = limits.apply(&lua).unwrap();
        assert!(budget.is_some());
    }

    #[test]
    fn applying_no_ceiling_leaves_the_state_alone() {
        let lua = mlua::Lua::new();
        assert!(ResourceLimits::none().apply(&lua).unwrap().is_none());
        assert!(lua.load("return 1").exec().is_ok());
    }
}
