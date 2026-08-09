//! The running count of VM instructions an evaluation has spent, and the hook that stops it.
//!
//! Separate from the limit it enforces because the two have different lifetimes: an
//! [`InstructionLimit`] is a value on a policy and never changes, while the count is per
//! evaluation and has to be reset before each one. Keeping them apart is what makes a reused
//! engine correct — otherwise the second script on an engine would start from the first script's
//! total and be stopped almost immediately.
//!
//! Responsibilities:
//!
//! - [`InstructionBudget`], which owns the counter and the hook that increments it.
//! - [`BudgetExhausted`], the error the hook raises to abort a script.
//!
//! Non-responsibilities: choosing the ceiling, and deciding what a breach means to the caller.
//! Those are [`crate::ResourceLimits`] and [`crate::Engine`] respectively.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use mlua::{HookTriggers, VmState};

use crate::error::{Error, Result};
use crate::sandbox::InstructionLimit;

/// How many instructions pass between hook calls.
///
/// The ceiling is therefore enforced to within this many instructions rather than exactly. A
/// smaller interval buys precision nobody needs at a cost paid by every script that runs, since
/// the hook fires on the VM's hot path.
const CHECK_INTERVAL: u32 = 10_000;

/// Raised by the instruction hook to abort a script that has spent its budget.
///
/// An external error rather than a plain message so the engine can recognise it by type. A script
/// is free to raise a string that reads like this one, and matching on text would let it disguise
/// its own failure as a resource breach or the reverse.
#[derive(Debug, thiserror::Error)]
#[error("instruction budget of {limit} exhausted")]
pub(crate) struct BudgetExhausted {
    /// The ceiling that was passed.
    pub(crate) limit: u64,
}

/// The instruction counter for one engine, shared with the hook that increments it.
#[derive(Debug)]
pub(crate) struct InstructionBudget {
    spent: Arc<AtomicU64>,
    limit: u64,
}

impl InstructionBudget {
    /// Arms `limit` on `lua` and returns the budget tracking it.
    ///
    /// Uses a global hook rather than a thread hook so that threads the VM creates later inherit
    /// it; a per-thread hook would leave anything running on another thread uncounted.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EngineSetup`] when the hook cannot be installed.
    pub(crate) fn install(lua: &mlua::Lua, limit: InstructionLimit) -> Result<Self> {
        let spent = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&spent);
        let ceiling = limit.get();

        lua.set_global_hook(
            HookTriggers::new().every_nth_instruction(CHECK_INTERVAL),
            move |_, _| {
                let used = counter.fetch_add(u64::from(CHECK_INTERVAL), Ordering::Relaxed)
                    + u64::from(CHECK_INTERVAL);
                if used > ceiling {
                    Err(mlua::Error::external(BudgetExhausted { limit: ceiling }))
                } else {
                    Ok(VmState::Continue)
                }
            },
        )
        .map_err(|source| Error::EngineSetup {
            stage: "instruction limit",
            source: Box::new(source),
        })?;

        Ok(Self {
            spent,
            limit: ceiling,
        })
    }

    /// Returns the counter to zero, giving the next evaluation the full budget.
    pub(crate) fn reset(&self) {
        self.spent.store(0, Ordering::Relaxed);
    }

    /// Whether the counter has passed the ceiling.
    pub(crate) fn is_exhausted(&self) -> bool {
        self.spent.load(Ordering::Relaxed) > self.limit
    }

    /// The ceiling this budget enforces.
    pub(crate) const fn limit(&self) -> u64 {
        self.limit
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{BudgetExhausted, InstructionBudget};
    use crate::sandbox::InstructionLimit;

    fn armed(limit: u64) -> (mlua::Lua, InstructionBudget) {
        let lua = mlua::Lua::new();
        let budget = InstructionBudget::install(&lua, InstructionLimit::count(limit)).unwrap();
        (lua, budget)
    }

    #[test]
    fn a_terminating_script_finishes_within_its_budget() {
        let (lua, budget) = armed(10_000_000);
        assert!(
            lua.load("local x = 0 for i = 1, 100 do x = x + i end")
                .exec()
                .is_ok()
        );
        assert!(!budget.is_exhausted());
    }

    #[test]
    fn an_endless_loop_is_stopped() {
        let (lua, budget) = armed(100_000);
        let error = lua.load("while true do end").exec().unwrap_err();
        assert!(budget.is_exhausted());
        assert!(error.downcast_ref::<BudgetExhausted>().is_some(), "{error}");
    }

    #[test]
    fn an_endless_loop_inside_a_coroutine_is_also_stopped() {
        let (lua, budget) = armed(100_000);
        let source = "local co = coroutine.create(function() while true do end end)
                      local ok, err = coroutine.resume(co)
                      if not ok then error(err, 0) end";
        assert!(lua.load(source).exec().is_err());
        assert!(
            budget.is_exhausted(),
            "the hook did not reach the coroutine body"
        );
    }

    #[test]
    fn resetting_gives_the_next_evaluation_the_whole_budget() {
        let (lua, budget) = armed(100_000);
        assert!(lua.load("while true do end").exec().is_err());
        assert!(budget.is_exhausted());

        budget.reset();
        assert!(!budget.is_exhausted());
        assert!(lua.load("return 1").exec().is_ok());
    }

    #[test]
    fn the_budget_reports_the_ceiling_it_enforces() {
        let (_lua, budget) = armed(4242);
        assert_eq!(budget.limit(), 4242);
    }
}
