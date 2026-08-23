//! The `airsstack.ext` host module: what an extension uses to register itself.
//!
//! Its own module because it is the only built-in whose purpose is the *host program* rather
//! than the script's own work — `on` hands a function to the host to be called later, and
//! `granted` lets the script read the authority it was loaded with. It is installed under every
//! preset like every other module, so `if airsstack.ext then` keeps meaning "does this runtime
//! have it"; under a runtime that dispatches no events `on` refuses and says so.
//!
//! Handlers live in a Lua registry table rather than in Rust, so the engine holds no Lua
//! references across calls and the script's own closures stay ordinary Lua values.
//!
//! Responsibilities: [`Ext`], installing `on` and `granted`, and the registry key the engine's
//! dispatcher reads handlers from.
//!
//! Non-responsibilities: calling a handler. That is [`crate::Engine::dispatch`], which owns the
//! evaluation lock and the instruction budget.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::modules::{HostModule, InstallContext};
use crate::sandbox::{GrantSet, ResourceLimits};
use crate::types::{EventName, ModuleName};

/// Registry key of the table mapping event name → handler function.
pub(crate) const HANDLERS_KEY: &str = "airsl.ext.handlers";

/// Installs `airsstack.ext`.
#[derive(Debug)]
pub struct Ext {
    name: ModuleName,
    events: BTreeSet<EventName>,
}

impl Ext {
    /// Builds the module with no declared events, which is what [`mod@crate::modules::stdlib`]
    /// installs: `on` refuses every call and names the reason.
    #[must_use]
    pub fn new() -> Self {
        Self::with_events([])
    }

    /// Builds the module with the events a host dispatches; `on` accepts exactly these.
    ///
    /// # Panics
    ///
    /// Never in practice: the name is a literal that satisfies [`ModuleName`]'s rules, and the
    /// crate's own test suite covers it.
    #[must_use]
    pub fn with_events(events: impl IntoIterator<Item = EventName>) -> Self {
        Self {
            name: ModuleName::new("ext")
                .unwrap_or_else(|_| unreachable!("`ext` is a valid module name")),
            events: events.into_iter().collect(),
        }
    }
}

impl Default for Ext {
    fn default() -> Self {
        Self::new()
    }
}

impl HostModule for Ext {
    fn name(&self) -> &ModuleName {
        &self.name
    }

    fn install(
        &self,
        lua: &mlua::Lua,
        table: &mlua::Table,
        context: &InstallContext<'_>,
    ) -> Result<()> {
        let fail = |e: mlua::Error| Error::ModuleInstall {
            module: String::from("ext"),
            reason: e.to_string(),
        };

        let handlers = lua.create_table().map_err(fail)?;
        lua.set_named_registry_value(HANDLERS_KEY, handlers)
            .map_err(fail)?;

        let declared: Vec<String> = self.events.iter().map(ToString::to_string).collect();
        let accepted = self.events.clone();
        let on = lua
            .create_function(
                move |lua, (event, handler): (mlua::LuaString, mlua::Function)| {
                    let raw = event.to_str()?.to_owned();
                    let name = EventName::new(raw.clone())?;
                    if !accepted.contains(&name) {
                        return Err(mlua::Error::from(Error::UnknownEvent {
                            event: raw,
                            declared: declared.clone(),
                        }));
                    }
                    let handlers: mlua::Table = lua.named_registry_value(HANDLERS_KEY)?;
                    handlers.set(name.as_str(), handler)
                },
            )
            .map_err(fail)?;
        table.set("on", on).map_err(fail)?;

        let snapshot =
            granted_table(lua, context.grants(), context.policy().limits()).map_err(fail)?;
        lua.set_named_registry_value(GRANTED_KEY, snapshot)
            .map_err(fail)?;
        let granted = lua
            .create_function(|lua, ()| {
                let snapshot: mlua::Table = lua.named_registry_value(GRANTED_KEY)?;
                deep_copy(lua, &snapshot)
            })
            .map_err(fail)?;
        table.set("granted", granted).map_err(fail)?;

        Ok(())
    }
}

/// Registry key of the grant snapshot `granted` copies from.
const GRANTED_KEY: &str = "airsl.ext.granted";

/// Renders `roots` as strings in sorted (byte) order.
///
/// `FsGrant::read_roots`/`write_roots` report insertion order, which is a host declaration detail
/// a script has no way to observe independently. Two hosts granting the same roots in a different
/// order would otherwise hand a script different `granted()` output for the same authority, which
/// is the kind of instability determinism is meant to rule out.
// `to_string_lossy` rather than `.display()`: the two render a non-UTF-8 root identically
// (both substitute the replacement character), but `granted()` is machine-read by a script —
// unlike a `Display` impl meant for a human — so the choice is made explicit here rather than
// inherited from a formatting trait. Matches the convention `fs`'s own returned paths use
// (`modules/fs.rs`, e.g. `target.to_string_lossy().into_owned()`).
fn sorted_roots(roots: &[PathBuf]) -> Vec<String> {
    let mut rendered: Vec<String> = roots
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    rendered.sort();
    rendered
}

/// Renders the grants and ceilings as the table `ext.granted()` returns.
///
/// Built once at install and copied per call, so a script that mutates what it was handed
/// cannot change what the next call reports.
fn granted_table(
    lua: &mlua::Lua,
    grants: &GrantSet,
    limits: &ResourceLimits,
) -> mlua::Result<mlua::Table> {
    let root = lua.create_table()?;

    let fs = lua.create_table()?;
    let read = lua.create_table()?;
    for (i, path) in sorted_roots(grants.fs().read_roots())
        .into_iter()
        .enumerate()
    {
        read.set(i + 1, path)?;
    }
    let write = lua.create_table()?;
    for (i, path) in sorted_roots(grants.fs().write_roots())
        .into_iter()
        .enumerate()
    {
        write.set(i + 1, path)?;
    }
    fs.set("read", read)?;
    fs.set("write", write)?;
    root.set("fs", fs)?;

    let env = lua.create_table()?;
    for name in grants.env().names() {
        env.set(name, true)?;
    }
    root.set("env", env)?;

    let proc = lua.create_table()?;
    let run = lua.create_table()?;
    for program in grants.proc().executables() {
        run.set(program, true)?;
    }
    proc.set("run", run)?;
    root.set("proc", proc)?;

    let ceilings = lua.create_table()?;
    if let Some(memory) = limits.memory() {
        ceilings.set("memory", memory.get())?;
    }
    if let Some(instructions) = limits.instructions() {
        ceilings.set("instructions", instructions.get())?;
    }
    root.set("limits", ceilings)?;

    root.set("unrestricted", grants.is_unrestricted())?;
    Ok(root)
}

/// Copies a tree of tables. Only tables and scalars appear in the snapshot, so this is total.
fn deep_copy(lua: &mlua::Lua, source: &mlua::Table) -> mlua::Result<mlua::Table> {
    let target = lua.create_table()?;
    for pair in source.pairs::<mlua::Value, mlua::Value>() {
        let (key, value) = pair?;
        let value = match value {
            mlua::Value::Table(inner) => mlua::Value::Table(deep_copy(lua, &inner)?),
            other => other,
        };
        target.set(key, value)?;
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Ext;
    use crate::modules::stdlib;
    use crate::{Engine, EventName, GrantSet, Policy, Script};

    fn engine_with_events(events: &[&str]) -> Engine {
        let mut set = stdlib().unwrap();
        set.replace(Box::new(Ext::with_events(
            events.iter().map(|e| EventName::new(*e).unwrap()),
        )))
        .unwrap();
        Engine::builder()
            .policy(Policy::confined())
            .stdlib(set)
            .build()
            .unwrap()
    }

    fn run(engine: &Engine, source: &str) -> crate::Result<String> {
        engine.eval_to::<String>(&Script::from_source(source, "t").unwrap())
    }

    #[test]
    fn the_module_is_installed_under_every_preset() {
        for policy in [Policy::trusted(), Policy::confined(), Policy::pure()] {
            let engine = Engine::builder().policy(policy).build().unwrap();
            assert_eq!(
                run(&engine, "return type(airsstack.ext.on)").unwrap(),
                "function"
            );
        }
    }

    #[test]
    fn on_accepts_a_declared_event() {
        let engine = engine_with_events(&["note_saved"]);
        assert!(
            run(
                &engine,
                "airsstack.ext.on('note_saved', function() end) return 'ok'"
            )
            .is_ok()
        );
    }

    #[test]
    fn on_refuses_an_undeclared_event_and_names_the_declared_ones() {
        let engine = engine_with_events(&["note_saved", "query"]);
        let err = run(&engine, "airsstack.ext.on('tpyo', function() end)").unwrap_err();
        let text = err.to_string();
        assert!(text.contains("`tpyo`"), "{text}");
        assert!(text.contains("note_saved, query"), "{text}");
    }

    #[test]
    fn on_under_the_default_stdlib_says_the_runtime_dispatches_no_events() {
        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();
        let err = run(&engine, "airsstack.ext.on('anything', function() end)").unwrap_err();
        assert!(
            err.to_string()
                .contains("this runtime dispatches no events"),
            "{err}"
        );
    }

    #[test]
    fn on_refuses_a_malformed_event_name() {
        let engine = engine_with_events(&["note_saved"]);
        let err = run(&engine, "airsstack.ext.on('Note-Saved', function() end)").unwrap_err();
        assert!(err.to_string().contains("event name"), "{err}");
    }

    #[test]
    fn granted_reports_the_install_time_grants() {
        let dir = tempfile::tempdir().unwrap();
        let policy = Policy::confined().with_grants(
            GrantSet::declared()
                .with_fs(|fs| fs.read(dir.path()))
                .with_env(|env| env.read(["APP_HOME"]))
                .with_proc(|proc| proc.allow(["git"])),
        );
        let engine = Engine::builder().policy(policy).build().unwrap();
        let found = run(
            &engine,
            "local g = airsstack.ext.granted() \
             return table.concat({#g.fs.read, #g.fs.write, tostring(g.env.APP_HOME), \
             tostring(g.proc.run.git), tostring(g.limits.memory ~= nil)}, ',')",
        )
        .unwrap();
        assert_eq!(found, "1,0,true,true,true");
    }

    #[test]
    fn granted_lists_fs_roots_in_sorted_order_rather_than_insertion_order() {
        // Two temp dirs, granted in whichever order sorts *against* their byte order, so a pass
        // proves the table was sorted rather than merely coincidentally ordered.
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let (later, earlier) = if first.path() < second.path() {
            (second.path(), first.path())
        } else {
            (first.path(), second.path())
        };
        let policy = Policy::confined().with_grants(
            GrantSet::declared()
                .with_fs(|fs| fs.read(later).read(earlier).write(later).write(earlier)),
        );
        let engine = Engine::builder().policy(policy).build().unwrap();
        let found = run(
            &engine,
            "local g = airsstack.ext.granted() \
             return tostring(g.fs.read[1] < g.fs.read[2]) .. ',' .. \
                    tostring(g.fs.write[1] < g.fs.write[2])",
        )
        .unwrap();
        assert_eq!(found, "true,true");
    }

    #[test]
    fn granted_returns_a_copy_each_time() {
        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();
        let found = run(
            &engine,
            "airsstack.ext.granted().env.INJECTED = true \
             return tostring(airsstack.ext.granted().env.INJECTED)",
        )
        .unwrap();
        assert_eq!(found, "nil");
    }

    #[test]
    fn granted_omits_a_ceiling_that_is_unlimited() {
        let engine = Engine::builder().policy(Policy::trusted()).build().unwrap();
        let found = run(
            &engine,
            "local g = airsstack.ext.granted() \
             return tostring(g.limits.memory) .. ',' .. tostring(g.unrestricted)",
        )
        .unwrap();
        assert_eq!(found, "nil,true");
    }
}
