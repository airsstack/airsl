//! The `airsstack.env` host module.
//!
//! Its own module rather than a corner of `fs` because the authority is a different shape: not a
//! region of a tree but a list of names. A process environment routinely carries credentials that
//! have nothing to do with the script reading it, so "may read the environment" is almost never
//! the authority anyone means, and this module has no way to express it.
//!
//! Responsibilities: [`Env`], installing `get`, `all` and `set`, and the per-engine overlay that
//! `set` writes into.
//!
//! Non-responsibilities: deciding which names are allowed ([`crate::EnvGrant`]).

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::error::{Error, Result};
use crate::modules::{HostModule, InstallContext};
use crate::sandbox::GrantSet;
use crate::types::ModuleName;

/// The environment as one engine's scripts see it.
///
/// `set` writes here rather than into the process, and `get`, `all` and `proc.run` read here first.
/// Two reasons, and the second is the one that matters: `std::env::set_var` is `unsafe` in Edition
/// 2024 because it races every other thread reading the environment, and this crate forbids
/// `unsafe` outright — but even with a safe way to do it, a sandboxed script silently changing the
/// *host's* environment is not a capability anyone meant to grant. An overlay gives a script the
/// behaviour it expects while keeping the blast radius inside the engine.
#[derive(Debug, Default)]
pub(crate) struct Overlay {
    entries: RwLock<BTreeMap<String, Option<String>>>,
}

impl Overlay {
    /// An empty overlay.
    const fn new() -> Self {
        Self {
            entries: RwLock::new(BTreeMap::new()),
        }
    }

    /// Records that `name` reads as `value`, or as unset when `value` is `None`.
    fn set(&self, name: &str, value: Option<String>) {
        if let Ok(mut entries) = self.entries.write() {
            entries.insert(name.to_owned(), value);
        }
    }

    /// The value `name` has for a script: the overlay if it carries one, else the real environment.
    pub(crate) fn get(&self, name: &str) -> Option<String> {
        self.entries
            .read()
            .ok()
            .and_then(|entries| entries.get(name).cloned())
            .unwrap_or_else(|| std::env::var(name).ok())
    }

    /// The overlay entries to apply to a child process, in sorted order.
    ///
    /// `None` means the child should not inherit the name at all, which is what `env.set(name)`
    /// with no value asked for.
    pub(crate) fn child_entries(&self) -> Vec<(String, Option<String>)> {
        self.entries
            .read()
            .map(|e| e.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }

    /// Every name the overlay has an opinion about, in sorted order.
    fn names(&self) -> Vec<String> {
        self.entries
            .read()
            .map(|e| e.keys().cloned().collect())
            .unwrap_or_default()
    }
}

/// Installs `airsstack.env`.
#[derive(Debug)]
pub struct Env {
    name: ModuleName,
}

impl Env {
    /// Builds the module.
    ///
    /// # Panics
    ///
    /// Never in practice: the name is a literal that satisfies [`ModuleName`]'s rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: ModuleName::new("env")
                .unwrap_or_else(|_| unreachable!("`env` is a valid module name")),
        }
    }
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether `grants` permits touching `name`.
fn allows(grants: &GrantSet, name: &str) -> bool {
    grants.is_unrestricted() || grants.env().allows(name)
}

/// The refusal for a name the policy does not cover.
fn denied(grants: &GrantSet, operation: &'static str, name: &str) -> Error {
    let allowed: Vec<_> = grants.env().names().collect();
    let detail = if allowed.is_empty() {
        format!("`{name}` is not granted — no environment variables are")
    } else {
        format!(
            "`{name}` is not granted — the allowed names are {}",
            allowed.join(", ")
        )
    };
    Error::Denied {
        module: "env",
        operation,
        detail,
    }
}

impl HostModule for Env {
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
            module: String::from("env"),
            reason: e.to_string(),
        };
        let grants = Arc::new(context.grants().clone());
        let overlay = overlay();

        let (g, o) = (Arc::clone(&grants), Arc::clone(&overlay));
        let get = lua
            .create_function(move |_, name: mlua::LuaString| {
                let name = name.to_str()?;
                if !allows(&g, &name) {
                    return Err(mlua::Error::from(denied(&g, "get", &name)));
                }
                // An unset variable is `nil`; a refusal raises. A script that cannot tell those
                // apart cannot tell "you may not ask" from "it is not set".
                Ok(o.get(&name))
            })
            .map_err(fail)?;
        table.set("get", get).map_err(fail)?;

        let (g, o) = (Arc::clone(&grants), Arc::clone(&overlay));
        let all = lua
            .create_function(move |lua, ()| {
                let out = lua.create_table()?;
                if g.is_unrestricted() {
                    // Sorted: `std::env::vars` has no defined order, and a table whose iteration
                    // depended on it would make every script reading it non-deterministic.
                    let mut names: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
                    names.extend(o.names());
                    names.sort();
                    names.dedup();
                    for name in names {
                        if let Some(value) = o.get(&name) {
                            out.set(name, value)?;
                        }
                    }
                } else {
                    // Only the granted names, and only those actually set. This is the point of
                    // the allowlist: a script sees what it declared, not what the host inherited.
                    for name in g.env().names() {
                        if let Some(value) = o.get(name) {
                            out.set(name, value)?;
                        }
                    }
                }
                Ok(out)
            })
            .map_err(fail)?;
        table.set("all", all).map_err(fail)?;

        let (g, o) = (grants, overlay);
        let set = lua
            .create_function(
                move |_, (name, value): (mlua::LuaString, Option<mlua::LuaString>)| {
                    let name = name.to_str()?;
                    if !allows(&g, &name) {
                        return Err(mlua::Error::from(denied(&g, "set", &name)));
                    }
                    let value = match value {
                        Some(text) => Some(text.to_str()?.to_owned()),
                        None => None,
                    };
                    o.set(&name, value);
                    Ok(())
                },
            )
            .map_err(fail)?;
        table.set("set", set).map_err(fail)?;

        Ok(())
    }
}

/// The one overlay every engine in this process shares.
///
/// Process-wide rather than per engine because `proc.run` has to hand the same view to a child,
/// and a child inherits from the process. Scoping it per engine would mean two engines disagreeing
/// about what a spawned command sees, which is harder to explain than one shared overlay.
pub(crate) fn overlay() -> Arc<Overlay> {
    static SHARED: std::sync::OnceLock<Arc<Overlay>> = std::sync::OnceLock::new();
    Arc::clone(SHARED.get_or_init(|| Arc::new(Overlay::new())))
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Env;
    use crate::{Engine, GrantSet, HostModule as _, Policy, Script};

    fn granted(names: &[&str]) -> Engine {
        let names: Vec<String> = names.iter().map(|s| (*s).to_owned()).collect();
        Engine::builder()
            .policy(
                Policy::confined()
                    .with_grants(GrantSet::declared().with_env(|env| env.read(names))),
            )
            .build()
            .unwrap()
    }

    fn eval<T: mlua::FromLuaMulti>(engine: &Engine, source: &str) -> crate::Result<T> {
        engine.eval_to::<T>(&Script::from_source(source, "test").unwrap())
    }

    #[test]
    fn the_module_is_named_env() {
        assert_eq!(Env::new().name().as_str(), "env");
    }

    #[test]
    fn a_granted_name_can_be_read() {
        let engine = granted(&["AIRSL_TEST_GRANTED"]);
        eval::<()>(&engine, "airsstack.env.set('AIRSL_TEST_GRANTED', 'value')").unwrap();
        assert_eq!(
            eval::<String>(&engine, "return airsstack.env.get('AIRSL_TEST_GRANTED')").unwrap(),
            "value"
        );
    }

    #[test]
    fn an_ungranted_name_is_refused_rather_than_reported_as_unset() {
        // The distinction matters: a script must be able to tell "you may not ask" from "it is
        // not set", or it will report a missing configuration when it was actually denied.
        let engine = granted(&["ALLOWED"]);
        let err = eval::<Option<String>>(&engine, "return airsstack.env.get('PATH')").unwrap_err();
        assert!(err.to_string().contains("env.get denied"), "{err}");
    }

    #[test]
    fn a_granted_but_unset_name_reads_as_nil() {
        let engine = granted(&["AIRSL_TEST_DEFINITELY_UNSET"]);
        let kind: String = eval(
            &engine,
            "return type(airsstack.env.get('AIRSL_TEST_DEFINITELY_UNSET'))",
        )
        .unwrap();
        assert_eq!(kind, "nil");
    }

    #[test]
    fn all_returns_only_the_granted_names() {
        let engine = granted(&["AIRSL_TEST_ALL"]);
        eval::<()>(&engine, "airsstack.env.set('AIRSL_TEST_ALL', 'x')").unwrap();
        let out: String = eval(
            &engine,
            "local names = {}
             for name in pairs(airsstack.env.all()) do names[#names+1] = name end
             table.sort(names)
             return table.concat(names, ',')",
        )
        .unwrap();
        assert_eq!(out, "AIRSL_TEST_ALL");
    }

    #[test]
    fn all_does_not_leak_the_hosts_environment() {
        // PATH is set in essentially every environment this runs in, and is not granted here.
        let engine = granted(&["AIRSL_TEST_ALL_2"]);
        let found: bool = eval(&engine, "return airsstack.env.all().PATH ~= nil").unwrap();
        assert!(!found, "an ungranted variable reached the script");
    }

    #[test]
    fn setting_an_ungranted_name_is_refused() {
        let engine = granted(&["ALLOWED"]);
        let err = eval::<()>(&engine, "airsstack.env.set('PATH', '/evil')").unwrap_err();
        assert!(err.to_string().contains("env.set denied"), "{err}");
    }

    #[test]
    fn set_does_not_change_the_host_process_environment() {
        // The overlay exists so a sandboxed script cannot reach out of the engine, and because
        // `std::env::set_var` is unsafe in Edition 2024 while this crate forbids `unsafe`.
        let engine = granted(&["AIRSL_TEST_HOST_UNTOUCHED"]);
        eval::<()>(
            &engine,
            "airsstack.env.set('AIRSL_TEST_HOST_UNTOUCHED', 'from-lua')",
        )
        .unwrap();
        assert!(std::env::var("AIRSL_TEST_HOST_UNTOUCHED").is_err());
    }

    #[test]
    fn set_with_no_value_makes_the_name_read_as_unset() {
        let engine = granted(&["AIRSL_TEST_CLEARED"]);
        eval::<()>(&engine, "airsstack.env.set('AIRSL_TEST_CLEARED', 'x')").unwrap();
        eval::<()>(&engine, "airsstack.env.set('AIRSL_TEST_CLEARED')").unwrap();
        let kind: String = eval(
            &engine,
            "return type(airsstack.env.get('AIRSL_TEST_CLEARED'))",
        )
        .unwrap();
        assert_eq!(kind, "nil");
    }

    #[test]
    fn a_policy_granting_nothing_refuses_every_name() {
        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();
        let err = eval::<Option<String>>(&engine, "return airsstack.env.get('HOME')").unwrap_err();
        assert!(
            err.to_string().contains("no environment variables"),
            "{err}"
        );
    }

    #[test]
    fn a_trusted_policy_reads_anything() {
        let engine = Engine::builder().policy(Policy::trusted()).build().unwrap();
        let kind: String = eval(&engine, "return type(airsstack.env.get('PATH'))").unwrap();
        assert_eq!(kind, "string");
    }
}
