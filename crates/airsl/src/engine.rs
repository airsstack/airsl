//! The Lua state, configured and ready to run scripts.
//!
//! This is the crate's entry point. It exists as a distinct type from the builder so that a
//! configured engine is immutable in the ways that matter — the sandbox has been applied and the
//! host modules installed before any caller can hold one, which removes the "did I remember to
//! sandbox it" question from every call site.
//!
//! Responsibilities:
//!
//! - [`Engine`], owning the [`mlua::Lua`] state and the installed [`crate::ModuleSet`].
//! - Evaluating a [`Script`], with and without a typed return value.
//! - Dispatching a host event into a handler a script registered through `airsstack.ext`.
//!
//! Non-responsibilities: deciding what to do about a failure. [`Engine::eval`] returns a
//! [`Result`]; [`crate::FailurePolicy`] describes how the caller should treat it.

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread::ThreadId;

use mlua::{FromLuaMulti, LuaSerdeExt as _};

use crate::builder::{EngineBuilder, Missing};
use crate::error::{Error, Result};
use crate::instruction_budget::{BudgetExhausted, InstructionBudget};
use crate::modules::ModuleSet;
use crate::modules::ext::HANDLERS_KEY;
use crate::require_loader::{RequireDisposition, RequireLoader};
use crate::sandbox::Policy;
use crate::script::Script;
use crate::types::{EventName, ModuleName, RootTable};

/// A configured Lua state.
///
/// Build one with [`Engine::builder`]. The sandbox policy is required, so an engine cannot be
/// constructed without deciding what scripts are allowed to reach.
///
/// # Examples
///
/// ```
/// use airsl::{Engine, Policy, Script};
///
/// let engine = Engine::builder().policy(Policy::confined()).build()?;
/// let script = Script::from_source("return airsstack.json.encode({ok = true})", "demo")?;
/// assert_eq!(engine.eval_to::<String>(&script)?, r#"{"ok":true}"#);
/// # Ok::<(), airsl::Error>(())
/// ```
pub struct Engine {
    lua: mlua::Lua,
    modules: ModuleSet,
    policy: Policy,
    budget: Option<InstructionBudget>,
    root: RootTable,
    /// Serialises whole evaluations against each other.
    ///
    /// `mlua` locks its state per *operation* (`mlua-0.12.0/src/state.rs:58`), which is enough for
    /// memory safety and not enough for this crate: an evaluation is four operations — reset the
    /// budget, write `arg`, install `require`, run the chunk — and without a lock spanning them,
    /// two threads sharing an engine interleave and each runs against the other's setup. Measured
    /// before this existed: eight threads evaluating `return arg[1]` on one engine got another
    /// thread's argument 12,247 times out of 16,000.
    ///
    /// Lua execution on one state cannot proceed in parallel regardless, so serialising here
    /// costs an uncontended lock and no throughput.
    evaluating: Mutex<()>,
    /// The thread currently holding `evaluating`, and the event it is inside a handler for, if
    /// any — so a re-entrant call issued from inside a running evaluation is refused with
    /// [`Error::Reentrant`] instead of deadlocking on the lock that thread already holds. Not only
    /// `dispatch`: a host function that calls back into `eval` or `check` on the same engine hits
    /// this the same way. Other threads are not affected: they block on `evaluating` exactly as an
    /// evaluation does.
    evaluating_thread: Mutex<Option<(ThreadId, Option<EventName>)>>,
}

/// Holds the evaluation lock and records which thread holds it; clears the record on drop.
///
/// The record has to survive for exactly as long as the lock's own critical section, which is
/// what makes this a guard rather than a plain assignment around the call: a `dispatch` that
/// returns early through `?` still clears the thread id when the guard drops.
struct Evaluation<'a> {
    _lock: MutexGuard<'a, ()>,
    thread: &'a Mutex<Option<(ThreadId, Option<EventName>)>>,
}

impl Drop for Evaluation<'_> {
    fn drop(&mut self) {
        // Poisoning carries information here — a stale thread id would misreport every later
        // re-entrancy check on this engine for the rest of the process — so, like `evaluating`,
        // a poisoned lock is recovered rather than left holding the previous value forever.
        let mut slot = self.thread.lock().unwrap_or_else(PoisonError::into_inner);
        *slot = None;
    }
}

impl Engine {
    /// Starts configuring an engine.
    #[must_use]
    pub fn builder() -> EngineBuilder<Missing> {
        EngineBuilder::new()
    }

    /// Wraps an already-configured state. Called by [`EngineBuilder::build`].
    pub(crate) const fn from_parts(
        lua: mlua::Lua,
        modules: ModuleSet,
        policy: Policy,
        budget: Option<InstructionBudget>,
        root: RootTable,
    ) -> Self {
        Self {
            lua,
            modules,
            policy,
            budget,
            root,
            evaluating: Mutex::new(()),
            evaluating_thread: Mutex::new(None),
        }
    }

    /// The global table this engine installed its host modules under.
    #[must_use]
    pub const fn root_table(&self) -> &RootTable {
        &self.root
    }

    /// The policy this engine was built with.
    ///
    /// The policy is fixed at construction and cannot be widened afterwards, so this describes the
    /// engine for as long as it exists.
    #[must_use]
    pub const fn policy(&self) -> &Policy {
        &self.policy
    }

    /// The version string the Lua state reports for itself.
    #[must_use]
    pub fn lua_version(&self) -> String {
        self.lua
            .globals()
            .get::<String>("_VERSION")
            .unwrap_or_else(|_| String::from("unknown"))
    }

    /// The names of the installed host modules, in installation order.
    #[must_use]
    pub fn module_names(&self) -> Vec<&ModuleName> {
        self.modules.names()
    }

    /// Takes the evaluation lock, blocking behind any other thread, and records this thread —
    /// unless this thread already holds it, in which case the lock is never touched and the call
    /// is refused instead of deadlocked.
    ///
    /// The thread id is read before either lock is taken, not inside the critical section: acquiring
    /// it can itself panic (`std::thread::current` — after thread-local teardown, notably in a
    /// custom test harness), and a panic while holding `evaluating_thread` would poison the very
    /// lock re-entrancy detection depends on.
    ///
    /// `event` names the handler this call is running for, if any — `dispatch` passes its own
    /// event, `eval`/`check` pass `None`. It is what a *nested* re-entrant call reports: a handler
    /// that calls a host function that calls back into `eval` sees `Error::Reentrant` naming the
    /// handler's own event, not the inner `eval`'s (which has none).
    ///
    /// Poisoning `evaluating` carries no information: the guarded value is `()`, and a panic in a
    /// host function leaves the Lua state to `mlua`'s own recovery rather than to this lock.
    /// Poisoning `evaluating_thread` is recovered the same way, for the reason its own doc comment
    /// gives: leaving it poisoned would misreport every later call as reentrant or never as
    /// reentrant, depending which side of the poison the read landed on.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Reentrant`] when this thread is already inside an evaluation on this
    /// engine.
    fn begin_evaluation(&self, event: Option<&EventName>) -> Result<Evaluation<'_>> {
        let current = std::thread::current().id();

        {
            let holder = self
                .evaluating_thread
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if let Some((thread, outer_event)) = holder.as_ref()
                && *thread == current
            {
                return Err(Error::Reentrant {
                    event: outer_event.as_ref().map(EventName::to_string),
                });
            }
        }

        let lock = self
            .evaluating
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut slot = self
            .evaluating_thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *slot = Some((current, event.cloned()));
        drop(slot);

        Ok(Evaluation {
            _lock: lock,
            thread: &self.evaluating_thread,
        })
    }

    /// Runs `script`, discarding whatever it returns.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Lua`] when the chunk fails to compile or raises while running, and
    /// [`Error::Reentrant`] when called from a thread already evaluating on this engine — see
    /// [`Engine::eval_to`].
    pub fn eval(&self, script: &Script) -> Result<()> {
        self.eval_to::<()>(script)
    }

    /// Runs `script` and converts its return value to `T`.
    ///
    /// The instruction budget is reset first, so every evaluation on a reused engine gets the
    /// whole ceiling rather than what the previous script left of it.
    ///
    /// Evaluations on one engine are serialised, so threads sharing an engine each get their own
    /// arguments, their own `require` root and the whole instruction budget. They do not run
    /// concurrently — one Lua state cannot execute in parallel — so a shared engine is a way to
    /// avoid rebuilding a state, not a way to get parallelism.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Lua`] when the chunk fails to compile, raises while running, exceeds a
    /// resource ceiling, or returns something that cannot be converted to `T`. Returns
    /// [`Error::Reentrant`] when called from a host function that is itself running inside an
    /// evaluation already in progress on this engine's own thread — the same protection
    /// [`Engine::dispatch`] gives a handler, extended to `eval`/`check` because a host function
    /// reachable from a handler can call either.
    pub fn eval_to<T: FromLuaMulti>(&self, script: &Script) -> Result<T> {
        let _evaluation = self.begin_evaluation(None)?;

        if let Some(budget) = self.budget.as_ref() {
            budget.reset();
        }
        self.set_arguments(script)?;
        self.set_require(script)?;

        self.lua
            .load(script.source())
            .set_name(script.name().as_lua())
            .eval::<T>()
            .map_err(|error| self.classify(script.name().as_str(), error))
    }

    /// Installs the script's arguments as the global `arg` table.
    ///
    /// Written before every evaluation rather than once at construction, so two scripts run on one
    /// engine each see their own arguments instead of whichever ran first.
    ///
    /// `arg[0]` is the script's own name, which is Lua's convention for a standalone script and
    /// what a ported shell script reads where it previously read `$0`.
    fn set_arguments(&self, script: &Script) -> Result<()> {
        let fail = |source: mlua::Error| Error::lua(script.name().as_str(), source);

        let table = self.lua.create_table().map_err(fail)?;
        table.set(0, script.name().as_str()).map_err(fail)?;
        for (index, value) in script.args().iter().enumerate() {
            table.set(index + 1, value.as_str()).map_err(fail)?;
        }
        self.lua.globals().set("arg", table).map_err(fail)
    }

    /// Gives the script about to run whichever `require` its surface and its root call for.
    ///
    /// Per evaluation because the directory it resolves against belongs to the script, not to the
    /// engine. A confined script built from source has no directory and so gets no `require` at
    /// all.
    ///
    /// The table of already-loaded modules outlives this call: it is keyed by canonical absolute
    /// path, so it stays correct across roots, and discarding it would make every evaluation
    /// re-run every module it requires.
    fn set_require(&self, script: &Script) -> Result<()> {
        match RequireDisposition::decide(self.policy.language(), script.root()) {
            // Nothing to install and — the part worth stating — nothing to clear. The `require`
            // sitting in the globals table is the one `package` brought with it when the state was
            // opened, and it is part of what this surface promises.
            RequireDisposition::Native => Ok(()),
            RequireDisposition::Confined(root) => RequireLoader::new(root).install(&self.lua),
            RequireDisposition::Absent => RequireLoader::remove(&self.lua),
        }
    }

    /// Compiles `script` without running a line of it.
    ///
    /// Exists because a script nothing loads is a script nothing checks. A hook's entry point is
    /// typically required by no test — the tests exercise the modules underneath it — so a syntax
    /// error there survives a green test run, and then `--fail-open` swallows it at the moment the
    /// hook fires. The failure is silent at both ends: CI says nothing and the session says
    /// nothing, and the hook has simply stopped working.
    ///
    /// This catches what the parser can see and no more. A misspelled field, a `nil` arithmetic, a
    /// module that raises the moment it is required — all compile happily and are a test's job.
    ///
    /// The policy is irrelevant here: parsing does not consult the globals table, so a chunk
    /// compiles or does not compile identically under every preset. Any engine will do.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Lua`] when the chunk does not compile. Returns [`Error::Reentrant`] when
    /// called from a host function already running inside an evaluation on this engine's own
    /// thread — see [`Engine::eval_to`].
    ///
    /// # Examples
    ///
    /// ```
    /// use airsl::{Engine, Policy, Script};
    ///
    /// let engine = Engine::builder().policy(Policy::pure()).build()?;
    /// assert!(engine.check(&Script::from_source("return 1 + 1", "ok")?).is_ok());
    /// assert!(engine.check(&Script::from_source("local function f(", "bad")?).is_err());
    /// # Ok::<(), airsl::Error>(())
    /// ```
    pub fn check(&self, script: &Script) -> Result<()> {
        let _evaluation = self.begin_evaluation(None)?;

        // `into_function` compiles and hands back the chunk rather than calling it, which is the
        // whole distinction from `eval`: a driver script's body runs on load, so anything that
        // executed it would perform the side effect it exists for.
        self.lua
            .load(script.source())
            .set_name(script.name().as_lua())
            .into_function()
            .map(|_| ())
            .map_err(|error| self.classify(script.name().as_str(), error))
    }

    /// Calls the handler a script registered for `event` through `airsstack.ext.on`.
    ///
    /// The payload crosses into Lua as a table built from `payload`, and the handler's first
    /// return value crosses back as JSON with sorted keys; a handler that returns nothing yields
    /// `Some(Value::Null)`. An event with no registered handler yields `None`, which is the normal
    /// case for an extension that chose not to subscribe, not a failure.
    ///
    /// Like [`Engine::eval_to`], a dispatch resets the instruction budget, holds the evaluation
    /// lock for its whole duration, and serialises against other threads. Unlike `eval_to`, it
    /// does not touch `arg` or `require`: the `require` installed when the registering script ran
    /// is still in place and still points at that script's root.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Reentrant`] when called from the thread already evaluating on this engine
    /// — a handler calling a host function that dispatches back. Returns
    /// [`Error::InstructionLimit`] or [`Error::MemoryLimit`] when the handler breaches a ceiling,
    /// and [`Error::Lua`] when it raises or when its result cannot be represented as JSON.
    pub fn dispatch(
        &self,
        event: &EventName,
        payload: &serde_json::Value,
    ) -> Result<Option<serde_json::Value>> {
        let _evaluation = self.begin_evaluation(Some(event))?;

        if let Some(budget) = self.budget.as_ref() {
            budget.reset();
        }

        let chunk = event.as_str();
        let fail = |error: mlua::Error| self.classify(chunk, error);

        let handlers: mlua::Value = self.lua.named_registry_value(HANDLERS_KEY).map_err(fail)?;
        let mlua::Value::Table(handlers) = handlers else {
            return Ok(None);
        };
        let handler: mlua::Value = handlers.get(chunk).map_err(fail)?;
        let mlua::Value::Function(handler) = handler else {
            return Ok(None);
        };

        let argument = self.lua.to_value(payload).map_err(fail)?;
        let returned: mlua::Value = handler.call(argument).map_err(fail)?;

        // Through `convert::sorted` rather than a second copy of its serializer options, so the
        // sorted-keys decision has exactly one place it can change.
        serde_json::to_value(crate::convert::sorted(&returned))
            .map(Some)
            // Wrapped as an `mlua::Error` and passed through `classify` like every other failure
            // in this function, rather than constructed directly — a `serde_json` failure cannot
            // be a resource breach today, but nothing here should be allowed to bypass the
            // structural check by construction.
            .map_err(|error| fail(mlua::Error::external(error)))
    }

    /// Names the failure a chunk produced, separating a resource breach from a script defect.
    ///
    /// Both decisions are made on structure rather than on message text. A script is free to raise
    /// a string that reads exactly like either report, and matching on the text would let it
    /// disguise its own failure as a resource breach or the reverse.
    ///
    /// `chunk` names whatever ran — a script's own chunk name for `eval`/`check`, or the event
    /// name for [`Engine::dispatch`] — so a breach report always says what was running when it
    /// happened rather than reusing a name that would not fit an event.
    fn classify(&self, chunk: &str, error: mlua::Error) -> Error {
        if let Some(budget) = self.budget.as_ref()
            && (budget.is_exhausted() || error.downcast_ref::<BudgetExhausted>().is_some())
        {
            return Error::InstructionLimit {
                chunk: chunk.to_owned(),
                limit: budget.limit(),
            };
        }

        if let Some(limit) = self.policy.limits().memory()
            && exhausted_memory(&error)
        {
            return Error::MemoryLimit {
                chunk: chunk.to_owned(),
                limit: limit.get(),
                source: Box::new(error),
            };
        }

        Error::lua(chunk, error)
    }
}

/// Whether the VM ran out of memory anywhere in this error's chain.
///
/// The allocator failure is usually wrapped by the callback or context that was running when it
/// happened, so the outermost variant is rarely the informative one.
fn exhausted_memory(error: &mlua::Error) -> bool {
    error.chain().any(|link| {
        matches!(
            link.downcast_ref::<mlua::Error>(),
            Some(mlua::Error::MemoryError(_))
        )
    })
}

impl core::fmt::Debug for Engine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Engine")
            .field("root", &self.root)
            .field("policy", &self.policy)
            .field("modules", &self.modules)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use std::sync::{Arc, OnceLock};

    use serde_json::json;

    use super::Engine;
    use crate::modules::{Ext, HostModule, InstallContext, ModuleSet, stdlib};
    use crate::{
        EventName, ExhaustedLimit, InstructionLimit, MemoryLimit, ModuleName, Policy,
        ResourceLimits, RootTable, Script,
    };

    /// Fails to compile if `T` is not shareable between threads.
    const fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn an_engine_can_be_sent_and_shared_between_threads() {
        assert_send_sync::<Engine>();
    }

    fn engine() -> Engine {
        Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap()
    }

    fn script(source: &str) -> Script {
        Script::from_source(source, "test").unwrap()
    }

    #[test]
    fn eval_runs_a_chunk_for_its_effect() {
        assert!(engine().eval(&script("local x = 1")).is_ok());
    }

    #[test]
    fn check_accepts_a_chunk_that_compiles() {
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        let script = Script::from_source("return 1 + 1", "ok").unwrap();
        assert!(engine.check(&script).is_ok());
    }

    #[test]
    fn check_refuses_a_chunk_that_does_not_compile() {
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        let script = Script::from_source("local function f(", "bad").unwrap();
        let err = engine.check(&script).unwrap_err();
        assert!(
            err.to_string().contains("bad"),
            "the chunk name names it: {err}"
        );
    }

    #[test]
    fn check_compiles_without_running_the_chunk() {
        // The distinction from `eval`, and the reason a driver script can be checked at all: its
        // body runs on load, so anything that executed it would perform the side effect it exists
        // for. A chunk that raises immediately still compiles.
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        let script = Script::from_source("error('this must not run')", "raises").unwrap();
        assert!(engine.check(&script).is_ok(), "compiling must not execute");
        assert!(
            engine.eval(&script).is_err(),
            "and running it must still raise"
        );
    }

    #[test]
    fn check_agrees_across_every_policy_preset() {
        // Parsing does not consult the globals table, so the answer cannot depend on the surface.
        for policy in [Policy::trusted(), Policy::confined(), Policy::pure()] {
            let engine = Engine::builder().policy(policy).build().unwrap();
            let good = Script::from_source("local x = io", "g").unwrap();
            let bad = Script::from_source("if true then", "b").unwrap();
            assert!(engine.check(&good).is_ok());
            assert!(engine.check(&bad).is_err());
        }
    }

    #[test]
    fn eval_to_converts_the_return_value() {
        assert_eq!(
            engine().eval_to::<i64>(&script("return 6 * 7")).unwrap(),
            42
        );
    }

    #[test]
    fn a_syntax_error_is_reported_against_the_chunk_name() {
        let err = engine().eval(&script("this is not lua")).unwrap_err();
        assert!(err.to_string().contains("test"), "{err}");
    }

    #[test]
    fn a_runtime_error_is_returned_not_panicked() {
        let err = engine().eval(&script("error('boom')")).unwrap_err();
        assert!(err.to_string().contains("boom"), "{err}");
    }

    #[test]
    fn a_custom_root_table_replaces_the_default_entirely() {
        let engine = Engine::builder()
            .policy(Policy::confined())
            .root_table(RootTable::new("myapp").unwrap())
            .build()
            .unwrap();
        assert_eq!(engine.root_table().as_str(), "myapp");
        assert_eq!(
            engine
                .eval_to::<String>(&script("return type(myapp.json)"))
                .unwrap(),
            "table"
        );
        assert_eq!(
            engine
                .eval_to::<String>(&script("return type(airsstack)"))
                .unwrap(),
            "nil"
        );
    }

    #[test]
    fn the_default_root_table_is_visible_to_scripts() {
        let found = engine()
            .eval_to::<String>(&script("return type(airsstack)"))
            .unwrap();
        assert_eq!(found, "table");
    }

    #[test]
    fn an_endless_loop_is_named_as_an_instruction_breach() {
        let engine = Engine::builder()
            .policy(Policy::confined().with_limits(
                ResourceLimits::none().with_instructions(Some(InstructionLimit::count(100_000))),
            ))
            .build()
            .unwrap();
        let err = engine.eval(&script("while true do end")).unwrap_err();
        assert_eq!(err.exhausted_limit(), Some(ExhaustedLimit::Instructions));
    }

    #[test]
    fn an_unbounded_allocation_is_named_as_a_memory_breach() {
        let engine =
            Engine::builder()
                .policy(Policy::confined().with_limits(
                    ResourceLimits::none().with_memory(Some(MemoryLimit::mebibytes(1))),
                ))
                .build()
                .unwrap();
        let err = engine
            .eval(&script("local t = {} for i = 1, 1e9 do t[i] = i end"))
            .unwrap_err();
        assert_eq!(err.exhausted_limit(), Some(ExhaustedLimit::Memory));
    }

    #[test]
    fn a_script_that_merely_failed_is_not_named_as_a_breach() {
        let engine = engine();
        for source in ["error('boom')", "this is not lua", "error('out of memory')"] {
            let err = engine.eval(&script(source)).unwrap_err();
            assert_eq!(err.exhausted_limit(), None, "{source}");
        }
    }

    #[test]
    fn the_instruction_budget_is_restored_between_scripts_on_one_engine() {
        let engine = Engine::builder()
            .policy(Policy::confined().with_limits(
                ResourceLimits::none().with_instructions(Some(InstructionLimit::count(1_000_000))),
            ))
            .build()
            .unwrap();

        assert!(engine.eval(&script("while true do end")).is_err());
        assert_eq!(engine.eval_to::<i64>(&script("return 7")).unwrap(), 7);
    }

    #[test]
    fn a_script_sees_its_own_arguments_in_the_arg_table() {
        let engine = engine();
        let source = script("return arg[1] .. arg[2]").with_args(["one", "two"]);
        assert_eq!(engine.eval_to::<String>(&source).unwrap(), "onetwo");
    }

    #[test]
    fn a_script_without_arguments_sees_an_empty_arg_table() {
        let engine = engine();
        assert_eq!(engine.eval_to::<i64>(&script("return #arg")).unwrap(), 0);
    }

    #[test]
    fn two_scripts_on_one_engine_each_see_their_own_arguments() {
        let engine = engine();
        let first = script("return arg[1]").with_args(["first"]);
        let second = script("return arg[1]").with_args(["second"]);
        assert_eq!(engine.eval_to::<String>(&first).unwrap(), "first");
        assert_eq!(engine.eval_to::<String>(&second).unwrap(), "second");
        assert_eq!(engine.eval_to::<String>(&first).unwrap(), "first");
    }

    #[test]
    fn a_script_sees_its_own_name_in_arg_zero() {
        let engine = engine();
        assert_eq!(
            engine.eval_to::<String>(&script("return arg[0]")).unwrap(),
            "test"
        );
    }

    #[test]
    fn concurrent_evaluations_each_see_their_own_arguments() {
        // Before the evaluation lock this returned another thread's argument on the large
        // majority of iterations, because writing `arg` and running the chunk were separate
        // acquisitions of `mlua`'s per-operation lock.
        let engine = std::sync::Arc::new(engine());

        // Spawned eagerly: the threads have to overlap for the race to be reachable at all, so
        // this cannot be a lazy iterator that starts each one as it is joined.
        let mut threads = Vec::new();
        for id in 0..4u32 {
            let engine = std::sync::Arc::clone(&engine);
            threads.push(std::thread::spawn(move || {
                let want = id.to_string();
                let source = script("return arg[1]").with_args([want.clone()]);
                (0..500)
                    .filter(|_| engine.eval_to::<String>(&source).unwrap() != want)
                    .count()
            }));
        }

        let wrong: usize = threads.into_iter().map(|t| t.join().unwrap()).sum();
        assert_eq!(
            wrong, 0,
            "{wrong} evaluations saw another thread's arguments"
        );
    }

    #[test]
    fn a_required_module_is_cached_across_evaluations_on_one_engine() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("counter.lua"),
            "COUNT = (COUNT or 0) + 1 return COUNT",
        )
        .unwrap();
        let path = dir.path().join("main.lua");
        std::fs::write(&path, "return require('counter')").unwrap();

        let engine = engine();
        let script = Script::from_file(&path).unwrap();
        for _ in 0..3 {
            assert_eq!(
                engine.eval_to::<i64>(&script).unwrap(),
                1,
                "the module re-ran, so the cache did not survive the evaluation"
            );
        }
    }

    #[test]
    fn a_module_that_raised_can_be_required_again_rather_than_reported_as_a_cycle() {
        let dir = tempfile::tempdir().unwrap();
        let module = dir.path().join("flaky.lua");
        std::fs::write(&module, "error('first attempt fails')").unwrap();
        let path = dir.path().join("main.lua");
        std::fs::write(&path, "return require('flaky')").unwrap();

        let engine = engine();
        let script = Script::from_file(&path).unwrap();
        let first = engine.eval(&script).unwrap_err();
        assert!(first.to_string().contains("first attempt fails"), "{first}");

        // The cache now outlives the evaluation, so a leftover in-progress marker would turn the
        // real error into a permanent and untrue cycle report.
        std::fs::write(&module, "return 7").unwrap();
        assert_eq!(engine.eval_to::<i64>(&script).unwrap(), 7);
    }

    #[test]
    fn the_engine_reports_the_lua_version_it_embeds() {
        assert!(engine().lua_version().starts_with("Lua 5."));
    }

    #[test]
    fn a_script_from_source_has_no_require_at_all() {
        assert_eq!(
            engine()
                .eval_to::<String>(&script("return type(require)"))
                .unwrap(),
            "nil"
        );
    }

    #[test]
    fn a_script_on_disk_can_require_a_sibling() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("lib.lua"), "return { answer = 42 }").unwrap();
        let path = dir.path().join("main.lua");
        std::fs::write(&path, "return require('lib').answer").unwrap();

        let engine = engine();
        let script = Script::from_file(&path).unwrap();
        assert_eq!(engine.eval_to::<i64>(&script).unwrap(), 42);
    }

    #[test]
    fn a_required_module_runs_once_however_often_it_is_required() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("counter.lua"),
            "COUNT = (COUNT or 0) + 1 return COUNT",
        )
        .unwrap();
        let path = dir.path().join("main.lua");
        std::fs::write(&path, "return require('counter') + require('counter')").unwrap();

        let engine = engine();
        let script = Script::from_file(&path).unwrap();
        assert_eq!(engine.eval_to::<i64>(&script).unwrap(), 2);
    }

    #[test]
    fn a_require_that_escapes_the_root_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.lua");
        std::fs::write(&path, "return require('../secrets')").unwrap();

        let engine = engine();
        let err = engine.eval(&Script::from_file(&path).unwrap()).unwrap_err();
        assert!(err.to_string().contains("require target"), "{err}");
    }

    #[test]
    fn a_missing_module_is_reported_rather_than_silently_nil() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.lua");
        std::fs::write(&path, "return require('absent')").unwrap();

        let engine = engine();
        let err = engine.eval(&Script::from_file(&path).unwrap()).unwrap_err();
        assert!(err.to_string().contains("not found"), "{err}");
    }

    #[test]
    fn a_require_cycle_errors_rather_than_recursing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.lua"), "return require('b')").unwrap();
        std::fs::write(dir.path().join("b.lua"), "return require('a')").unwrap();
        let path = dir.path().join("main.lua");
        std::fs::write(&path, "return require('a')").unwrap();

        let engine = engine();
        let err = engine.eval(&Script::from_file(&path).unwrap()).unwrap_err();
        assert!(err.to_string().contains("requires itself"), "{err}");
    }

    #[test]
    fn a_pure_policy_gives_no_require_even_to_a_script_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("lib.lua"), "return 1").unwrap();
        let path = dir.path().join("main.lua");
        std::fs::write(&path, "return type(require)").unwrap();

        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        let script = Script::from_file(&path).unwrap();
        assert_eq!(engine.eval_to::<String>(&script).unwrap(), "nil");
    }

    #[test]
    fn a_trusted_policy_leaves_lua_s_own_require_in_place() {
        // A script from source, so nothing here could have installed a confined loader: whatever
        // `require` is bound to is the one `package` arrived with. It stayed a function only once
        // the engine stopped clearing the global on every surface but `restricted`.
        let engine = Engine::builder().policy(Policy::trusted()).build().unwrap();
        assert_eq!(
            engine
                .eval_to::<String>(&script("return type(require)"))
                .unwrap(),
            "function"
        );
    }

    #[test]
    fn a_trusted_script_requires_through_package_path_rather_than_a_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("lib.lua"), "return { answer = 42 }").unwrap();
        let path = dir.path().join("main.lua");
        // Resolved through `package.path`, which is the whole point of the arrangement: `full`
        // keeps Lua's own resolution, so proving the global merely exists would not show that the
        // surface delivers what it documents.
        std::fs::write(
            &path,
            "package.path = arg[1] .. '/?.lua' return require('lib').answer",
        )
        .unwrap();

        let engine = Engine::builder().policy(Policy::trusted()).build().unwrap();
        let script = Script::from_file(&path)
            .unwrap()
            .with_args([dir.path().display().to_string()]);
        assert_eq!(engine.eval_to::<i64>(&script).unwrap(), 42);
    }

    #[test]
    fn module_names_reports_the_installed_standard_library() {
        let engine = engine();
        let names: Vec<_> = engine
            .module_names()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(names.contains(&String::from("json")), "{names:?}");
    }

    fn dispatching_engine(events: &[&str], policy: Policy) -> Engine {
        let mut set = stdlib().unwrap();
        set.replace(Box::new(Ext::with_events(
            events.iter().map(|e| EventName::new(*e).unwrap()),
        )))
        .unwrap();
        Engine::builder()
            .policy(policy)
            .stdlib(set)
            .build()
            .unwrap()
    }

    fn event(name: &str) -> EventName {
        EventName::new(name).unwrap()
    }

    #[test]
    fn dispatch_without_a_handler_is_silence_not_an_error() {
        let engine = dispatching_engine(&["ping"], Policy::confined());
        assert_eq!(engine.dispatch(&event("ping"), &json!({})).unwrap(), None);
    }

    #[test]
    fn dispatch_passes_the_payload_and_returns_the_handler_result() {
        let engine = dispatching_engine(&["echo"], Policy::confined());
        engine
            .eval(&script(
                "airsstack.ext.on('echo', function(p) return { got = p.n * 2, tags = p.tags } end)",
            ))
            .unwrap();
        let reply = engine
            .dispatch(&event("echo"), &json!({ "n": 21, "tags": ["a", "b"] }))
            .unwrap();
        assert_eq!(reply, Some(json!({ "got": 42, "tags": ["a", "b"] })));
    }

    #[test]
    fn a_handler_returning_nothing_yields_json_null() {
        let engine = dispatching_engine(&["fire"], Policy::confined());
        engine
            .eval(&script("airsstack.ext.on('fire', function() end)"))
            .unwrap();
        assert_eq!(
            engine.dispatch(&event("fire"), &json!(null)).unwrap(),
            Some(serde_json::Value::Null)
        );
    }

    #[test]
    fn a_handler_returning_an_empty_table_yields_an_empty_json_object() {
        // `nil` and `{}` cross the boundary as different JSON shapes; conflating them would make
        // "no reply" indistinguishable from "an empty object reply".
        let engine = dispatching_engine(&["empty"], Policy::confined());
        engine
            .eval(&script(
                "airsstack.ext.on('empty', function() return {} end)",
            ))
            .unwrap();
        assert_eq!(
            engine.dispatch(&event("empty"), &json!(null)).unwrap(),
            Some(json!({}))
        );
    }

    #[test]
    fn state_persists_between_dispatches() {
        let engine = dispatching_engine(&["tick"], Policy::confined());
        engine
            .eval(&script(
                "local n = 0 airsstack.ext.on('tick', function() n = n + 1 return n end)",
            ))
            .unwrap();
        for expected in 1..=3 {
            assert_eq!(
                engine.dispatch(&event("tick"), &json!(null)).unwrap(),
                Some(json!(expected))
            );
        }
    }

    #[test]
    fn a_runaway_handler_is_named_as_an_instruction_breach_and_the_next_call_is_unaffected() {
        let policy = Policy::confined().with_limits(
            ResourceLimits::none().with_instructions(Some(InstructionLimit::count(100_000))),
        );
        let engine = dispatching_engine(&["spin", "ok"], policy);
        engine
            .eval(&script(
                "airsstack.ext.on('spin', function() while true do end end) \
                 airsstack.ext.on('ok', function() return 1 end)",
            ))
            .unwrap();
        let err = engine.dispatch(&event("spin"), &json!(null)).unwrap_err();
        assert_eq!(err.exhausted_limit(), Some(ExhaustedLimit::Instructions));
        assert!(
            err.to_string().contains("spin"),
            "the event names the chunk: {err}"
        );
        assert_eq!(
            engine.dispatch(&event("ok"), &json!(null)).unwrap(),
            Some(json!(1))
        );
    }

    #[test]
    fn a_handler_that_allocates_past_the_memory_ceiling_is_named_as_a_memory_breach() {
        let policy = Policy::confined()
            .with_limits(ResourceLimits::none().with_memory(Some(MemoryLimit::mebibytes(1))));
        let engine = dispatching_engine(&["grow"], policy);
        engine
            .eval(&script(
                "airsstack.ext.on('grow', function() \
                     local t = {} for i = 1, 1e9 do t[i] = i end return 1 \
                 end)",
            ))
            .unwrap();
        let err = engine.dispatch(&event("grow"), &json!(null)).unwrap_err();
        assert_eq!(err.exhausted_limit(), Some(ExhaustedLimit::Memory));
    }

    #[test]
    fn dispatch_on_an_engine_without_the_ext_module_reports_no_handler_rather_than_an_error() {
        // `Ext` supplies the `HANDLERS_KEY` registry table `dispatch` reads. An engine built from
        // a `ModuleSet` that never installed `Ext` has no such table, which is the same shape as
        // "no handler registered" and dispatch must not treat it as a defect.
        let engine = Engine::builder()
            .policy(Policy::confined())
            .stdlib(ModuleSet::new())
            .build()
            .unwrap();
        assert_eq!(
            engine.dispatch(&event("anything"), &json!(null)).unwrap(),
            None
        );
    }

    #[test]
    fn a_handler_that_raises_is_a_lua_error_naming_the_event() {
        let engine = dispatching_engine(&["bad"], Policy::confined());
        engine
            .eval(&script(
                "airsstack.ext.on('bad', function() error('boom') end)",
            ))
            .unwrap();
        let err = engine.dispatch(&event("bad"), &json!(null)).unwrap_err();
        assert!(err.exhausted_limit().is_none());
        let text = err.to_string();
        assert!(text.contains("bad") && text.contains("boom"), "{text}");
    }

    #[test]
    fn re_registering_an_event_replaces_the_handler() {
        let engine = dispatching_engine(&["v"], Policy::confined());
        engine
            .eval(&script(
                "airsstack.ext.on('v', function() return 1 end) \
                 airsstack.ext.on('v', function() return 2 end)",
            ))
            .unwrap();
        assert_eq!(
            engine.dispatch(&event("v"), &json!(null)).unwrap(),
            Some(json!(2))
        );
    }

    /// A module whose single function dispatches `reenter` on the engine it was installed into.
    struct Boomerang {
        name: ModuleName,
        engine: Arc<OnceLock<Engine>>,
    }

    impl HostModule for Boomerang {
        fn name(&self) -> &ModuleName {
            &self.name
        }

        fn install(
            &self,
            lua: &mlua::Lua,
            table: &mlua::Table,
            _context: &InstallContext<'_>,
        ) -> crate::Result<()> {
            let engine = Arc::clone(&self.engine);
            let back = lua
                .create_function(move |_, ()| {
                    let Some(engine) = engine.get() else {
                        return Ok(String::from("no engine"));
                    };
                    match engine.dispatch(&event("reenter"), &serde_json::Value::Null) {
                        Ok(_) => Ok(String::from("dispatched")),
                        Err(error) => Ok(error.to_string()),
                    }
                })
                .map_err(|e| crate::Error::lua("boomerang", e))?;
            table
                .set("back", back)
                .map_err(|e| crate::Error::lua("boomerang", e))
        }
    }

    #[test]
    fn a_handler_dispatching_back_into_its_own_engine_is_refused_not_deadlocked() {
        let slot = Arc::new(OnceLock::new());
        let mut set = stdlib().unwrap();
        set.replace(Box::new(Ext::with_events([event("reenter")])))
            .unwrap();
        set.insert(Box::new(Boomerang {
            name: ModuleName::new("boomerang").unwrap(),
            engine: Arc::clone(&slot),
        }))
        .unwrap();
        let engine = Engine::builder()
            .policy(Policy::confined())
            .stdlib(set)
            .build()
            .unwrap();
        assert!(slot.set(engine).is_ok());
        let engine = slot.get().unwrap();

        engine
            .eval(&script(
                "airsstack.ext.on('reenter', function() return airsstack.boomerang.back() end)",
            ))
            .unwrap();
        let reply = engine.dispatch(&event("reenter"), &json!(null)).unwrap();
        let text = reply.unwrap().as_str().unwrap().to_owned();
        assert!(
            text.contains("re-entrant call during handler for event `reenter`"),
            "the inner dispatch was refused with Reentrant: {text}"
        );
    }

    /// A module whose single function calls `eval` back on the engine it was installed into —
    /// the case beyond `dispatch` that [`Error::Reentrant`] also has to cover: a host function
    /// reachable from a handler is not limited to calling `dispatch` back.
    struct EvalBoomerang {
        name: ModuleName,
        engine: Arc<OnceLock<Engine>>,
    }

    impl HostModule for EvalBoomerang {
        fn name(&self) -> &ModuleName {
            &self.name
        }

        fn install(
            &self,
            lua: &mlua::Lua,
            table: &mlua::Table,
            _context: &InstallContext<'_>,
        ) -> crate::Result<()> {
            let engine = Arc::clone(&self.engine);
            let back = lua
                .create_function(move |_, ()| {
                    let Some(engine) = engine.get() else {
                        return Ok(String::from("no engine"));
                    };
                    match engine.eval(&script("return 1")) {
                        Ok(()) => Ok(String::from("evaluated")),
                        Err(error) => Ok(error.to_string()),
                    }
                })
                .map_err(|e| crate::Error::lua("eval_boomerang", e))?;
            table
                .set("back", back)
                .map_err(|e| crate::Error::lua("eval_boomerang", e))
        }
    }

    #[test]
    fn a_handler_calling_eval_back_on_its_own_engine_is_refused_not_deadlocked() {
        let slot = Arc::new(OnceLock::new());
        let mut set = stdlib().unwrap();
        set.replace(Box::new(Ext::with_events([event("reenter")])))
            .unwrap();
        set.insert(Box::new(EvalBoomerang {
            name: ModuleName::new("eval_boomerang").unwrap(),
            engine: Arc::clone(&slot),
        }))
        .unwrap();
        let engine = Engine::builder()
            .policy(Policy::confined())
            .stdlib(set)
            .build()
            .unwrap();
        assert!(slot.set(engine).is_ok());
        let engine = slot.get().unwrap();

        engine
            .eval(&script(
                "airsstack.ext.on('reenter', function() return airsstack.eval_boomerang.back() end)",
            ))
            .unwrap();
        let reply = engine.dispatch(&event("reenter"), &json!(null)).unwrap();
        let text = reply.unwrap().as_str().unwrap().to_owned();
        assert!(
            text.contains("re-entrant call during handler for event `reenter`"),
            "the inner eval was refused with Reentrant, naming the outer handler's event: {text}"
        );
    }

    #[test]
    fn a_plain_eval_that_calls_eval_back_on_itself_reports_no_event() {
        // No handler and no dispatch anywhere in this call chain — the re-entrancy is real, but
        // there is no event to name, which is what `Error::Reentrant { event: None }` exists for.
        let slot = Arc::new(OnceLock::new());
        let mut set = stdlib().unwrap();
        set.insert(Box::new(EvalBoomerang {
            name: ModuleName::new("eval_boomerang").unwrap(),
            engine: Arc::clone(&slot),
        }))
        .unwrap();
        let engine = Engine::builder()
            .policy(Policy::confined())
            .stdlib(set)
            .build()
            .unwrap();
        assert!(slot.set(engine).is_ok());
        let engine = slot.get().unwrap();

        let reply = engine
            .eval_to::<String>(&script("return airsstack.eval_boomerang.back()"))
            .unwrap();
        assert_eq!(reply, "re-entrant evaluation");
    }

    /// A module whose function poisons `evaluating_thread` — the same way a panic mid-critical-
    /// section would — and then attempts a re-entrant `eval`, so the test can see whether
    /// detection survives the poison rather than silently disabling itself.
    struct Poisoner {
        name: ModuleName,
        engine: Arc<OnceLock<Engine>>,
    }

    /// Poisons `evaluating_thread` without changing what it holds: locks it, panics while the
    /// guard is alive, and catches the unwind on the same thread. The slot still names the
    /// calling thread when the guard's `Drop` runs mid-unwind, so the poisoned `Mutex` keeps the
    /// correct value — only its poison flag changes.
    #[expect(
        clippy::unwrap_used,
        reason = "test helper poisoning a lock on purpose; a panic here is the intended trigger"
    )]
    #[expect(
        clippy::panic,
        reason = "the panic is the poisoning mechanism itself, caught by `catch_unwind` and never \
                  propagated past this function"
    )]
    fn poison_evaluating_thread(engine: &Engine) {
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = engine.evaluating_thread.lock().unwrap();
            panic!("deliberately poisoning `evaluating_thread` for the test");
        }));
        assert!(poisoned.is_err(), "the panic must have actually unwound");
        assert!(engine.evaluating_thread.is_poisoned());
    }

    impl HostModule for Poisoner {
        fn name(&self) -> &ModuleName {
            &self.name
        }

        fn install(
            &self,
            lua: &mlua::Lua,
            table: &mlua::Table,
            _context: &InstallContext<'_>,
        ) -> crate::Result<()> {
            let engine = Arc::clone(&self.engine);
            let trigger = lua
                .create_function(move |_, ()| {
                    let Some(engine) = engine.get() else {
                        return Ok(String::from("no engine"));
                    };

                    poison_evaluating_thread(engine);

                    match engine.eval(&script("return 1")) {
                        Ok(()) => Ok(String::from("not reentrant")),
                        Err(error) => Ok(error.to_string()),
                    }
                })
                .map_err(|e| crate::Error::lua("poisoner", e))?;
            table
                .set("trigger", trigger)
                .map_err(|e| crate::Error::lua("poisoner", e))
        }
    }

    #[test]
    #[expect(
        clippy::unwrap_used,
        reason = "test poisons and inspects the lock directly; a panic here is the failure signal"
    )]
    fn a_poisoned_evaluating_thread_lock_still_detects_reentrancy_rather_than_deadlocking() {
        let slot = Arc::new(OnceLock::new());
        let mut set = stdlib().unwrap();
        set.insert(Box::new(Poisoner {
            name: ModuleName::new("poisoner").unwrap(),
            engine: Arc::clone(&slot),
        }))
        .unwrap();
        let engine = Engine::builder()
            .policy(Policy::confined())
            .stdlib(set)
            .build()
            .unwrap();
        assert!(slot.set(engine).is_ok());
        let engine = slot.get().unwrap();

        let reply = engine
            .eval_to::<String>(&script("return airsstack.poisoner.trigger()"))
            .unwrap();
        assert_eq!(
            reply, "re-entrant evaluation",
            "poisoning must not silently disable re-entrancy detection"
        );

        // The lock stays poisoned (poisoning this way is permanent by design), but recovery keeps
        // the engine usable rather than making every later call fail.
        assert!(engine.eval(&script("return 1")).is_ok());
    }

    #[test]
    fn threads_dispatching_concurrently_serialise_without_a_spurious_reentrant_error() {
        let engine = Arc::new(dispatching_engine(&["add"], Policy::confined()));
        engine
            .eval(&script(
                "airsstack.ext.on('add', function(p) return p.a + p.b end)",
            ))
            .unwrap();

        let workers: Vec<_> = (0..8)
            .map(|i| {
                let engine = Arc::clone(&engine);
                std::thread::spawn(move || {
                    let mut results = Vec::new();
                    for j in 0..200 {
                        let reply = engine
                            .dispatch(&event("add"), &json!({ "a": i, "b": j }))
                            .unwrap();
                        results.push(reply == Some(json!(i + j)));
                    }
                    results
                })
            })
            .collect();

        for worker in workers {
            let results = worker.join().unwrap();
            assert!(
                results.iter().all(|ok| *ok),
                "every reply matched its own arguments"
            );
        }
    }
}
