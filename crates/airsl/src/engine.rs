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
//!
//! Non-responsibilities: deciding what to do about a failure. [`Engine::eval`] returns a
//! [`Result`]; [`crate::FailurePolicy`] describes how the caller should treat it.

use std::sync::{Mutex, PoisonError};

use mlua::FromLuaMulti;

use crate::builder::{EngineBuilder, Missing};
use crate::error::{Error, Result};
use crate::instruction_budget::{BudgetExhausted, InstructionBudget};
use crate::modules::ModuleSet;
use crate::require_loader::RequireLoader;
use crate::sandbox::Policy;
use crate::script::Script;
use crate::types::{ModuleName, RootTable};

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

    /// Runs `script`, discarding whatever it returns.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Lua`] when the chunk fails to compile or raises while running.
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
    /// resource ceiling, or returns something that cannot be converted to `T`.
    pub fn eval_to<T: FromLuaMulti>(&self, script: &Script) -> Result<T> {
        // Poisoning carries no information here: the guarded value is `()`, and a panic in a host
        // function leaves the Lua state to `mlua`'s own recovery rather than to this lock.
        let _guard = self
            .evaluating
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        if let Some(budget) = self.budget.as_ref() {
            budget.reset();
        }
        self.set_arguments(script)?;
        self.set_require(script)?;

        self.lua
            .load(script.source())
            .set_name(script.name().as_lua())
            .eval::<T>()
            .map_err(|error| self.classify(script, error))
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

    /// Installs or clears the confined `require` for the script about to run.
    ///
    /// Per evaluation because the directory it resolves against belongs to the script, not to the
    /// engine. A script built from source has no directory and so gets no `require` at all.
    ///
    /// The table of already-loaded modules outlives this call: it is keyed by canonical absolute
    /// path, so it stays correct across roots, and discarding it would make every evaluation
    /// re-run every module it requires.
    fn set_require(&self, script: &Script) -> Result<()> {
        match script.root() {
            Some(root) if RequireLoader::applies_to(self.policy.language()) => {
                RequireLoader::new(root).install(&self.lua)
            }
            _ => RequireLoader::remove(&self.lua),
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
    /// Returns [`Error::Lua`] when the chunk does not compile.
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
        let _guard = self
            .evaluating
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        // `into_function` compiles and hands back the chunk rather than calling it, which is the
        // whole distinction from `eval`: a driver script's body runs on load, so anything that
        // executed it would perform the side effect it exists for.
        self.lua
            .load(script.source())
            .set_name(script.name().as_lua())
            .into_function()
            .map(|_| ())
            .map_err(|error| self.classify(script, error))
    }

    /// Names the failure a script produced, separating a resource breach from a script defect.
    ///
    /// Both decisions are made on structure rather than on message text. A script is free to raise
    /// a string that reads exactly like either report, and matching on the text would let it
    /// disguise its own failure as a resource breach or the reverse.
    fn classify(&self, script: &Script, error: mlua::Error) -> Error {
        let chunk = script.name().as_str();

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

    use super::Engine;
    use crate::{
        ExhaustedLimit, InstructionLimit, MemoryLimit, Policy, ResourceLimits, RootTable, Script,
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
    fn module_names_reports_the_installed_standard_library() {
        let engine = engine();
        let names: Vec<_> = engine
            .module_names()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(names.contains(&String::from("json")), "{names:?}");
    }
}
