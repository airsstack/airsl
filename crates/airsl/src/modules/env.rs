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
use crate::types::{EnvName, ModuleName};

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
    entries: RwLock<BTreeMap<EnvName, Option<String>>>,
}

impl Overlay {
    /// An empty overlay.
    const fn new() -> Self {
        Self {
            entries: RwLock::new(BTreeMap::new()),
        }
    }

    /// Records that `name` reads as `value`, or as unset when `value` is `None`.
    ///
    /// `BTreeMap::insert` replaces the value for an existing key but keeps the key exactly as it
    /// was first inserted. On Windows, a script that calls `set("Path", ..)` then later
    /// `set("PATH", ..)` therefore stores one entry whose key is spelled `Path` — first spelling
    /// wins. That is deterministic, which is the property that matters; it is not a defect to
    /// "fix" toward last-spelling-wins, which would make the stored casing depend on call order
    /// instead of being fixed by it.
    fn set(&self, name: &str, value: Option<String>) {
        if let Ok(mut entries) = self.entries.write() {
            entries.insert(EnvName::new(name), value);
        }
    }

    /// The value `name` has for a script: the overlay if it carries one, else the real environment.
    pub(crate) fn get(&self, name: &str) -> Option<String> {
        let key = EnvName::new(name);
        self.entries
            .read()
            .ok()
            .and_then(|entries| entries.get(&key).cloned())
            .unwrap_or_else(|| std::env::var(name).ok())
    }

    /// The overlay entries to apply to a child process, in sorted order.
    ///
    /// `None` means the child should not inherit the name at all, which is what `env.set(name)`
    /// with no value asked for.
    ///
    /// `std::process::Command`'s own environment map folds case on Windows, exactly as
    /// [`EnvName`] does — so no change is needed here to keep the two agreeing. At most one
    /// overlay entry can exist per folded identity, since the map is now keyed by `EnvName`
    /// itself, so `Command`'s map can never collapse two of this overlay's entries into one; and
    /// the order this returns entries in is not something a script can observe, since it only
    /// ever reaches the *child's* environment block, not back into this engine.
    pub(crate) fn child_entries(&self) -> Vec<(String, Option<String>)> {
        self.entries
            .read()
            .map(|e| {
                e.iter()
                    .map(|(k, v)| (k.as_str().to_owned(), v.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every name the overlay has an opinion about, in sorted order.
    fn names(&self) -> Vec<String> {
        self.entries
            .read()
            .map(|e| e.keys().map(|k| k.as_str().to_owned()).collect())
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

/// The names `env.all()` reports for the unrestricted branch, host spelling preferred, in a
/// deterministic order that does not depend on `host`'s iteration order.
///
/// A free function, rather than logic inlined in the closure `install` builds, so it can be
/// tested directly — without an engine, and without depending on the real process environment for
/// its inputs.
///
/// Host names beginning with `=` are dropped: `std::env::vars()` surfaces Windows' per-drive
/// current-directory pseudo-variables (`=C:`, `=ExitCode`, …), which are process-private state,
/// not environment a script should ever see. An overlay name beginning with `=` is **not**
/// filtered the same way — the `=` rule exists to hide what the host volunteers involuntarily, and
/// an overlay entry is something the script itself wrote under a grant; hiding a script's own
/// write from `all()` while `get()` still returns it would be the more surprising behaviour of the
/// two.
///
/// Deduplication is by [`EnvName`] identity: on Windows a host `Path` and an overlay `PATH` are
/// one name, and the host spelling is kept, because `all()` should describe what a script's own
/// process actually inherited under the identity the platform itself uses, and only fall back to
/// the overlay's spelling for a name the host never had at all.
fn merged_names(
    host: impl IntoIterator<Item = String>,
    overlay: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let mut by_identity: BTreeMap<EnvName, String> = BTreeMap::new();
    for name in host {
        if name.starts_with('=') {
            continue;
        }
        by_identity.insert(EnvName::new(name.clone()), name);
    }
    for name in overlay {
        by_identity
            .entry(EnvName::new(name.clone()))
            .or_insert(name);
    }
    by_identity.into_values().collect()
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
                    // Sorted, and deduplicated by platform identity rather than by exact
                    // spelling: `std::env::vars` has no defined order, so a table whose iteration
                    // depended on it would make every script reading it non-deterministic, and on
                    // Windows a host `Path` and an overlay `PATH` are one name, not two, so both
                    // must not appear.
                    let names = merged_names(std::env::vars().map(|(k, _)| k), o.names());
                    for name in names {
                        if let Some(value) = o.get(&name) {
                            out.set(name, value)?;
                        }
                    }
                } else {
                    // Only the granted names, spelled the way the grant spells them, and only
                    // those actually set. This is the point of the allowlist: a script sees what
                    // it declared, not what the host inherited — host casing has no bearing here,
                    // because this branch enumerates no host names at all. It cannot produce a
                    // duplicate either: `EnvGrant`'s own `BTreeSet<EnvName>` is already
                    // fold-deduped, so two grant entries can never share an identity to begin
                    // with.
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

    use super::{Env, Overlay, merged_names};
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
        // A single-key probe (`.PATH`) would prove nothing on Windows: a leak there would surface
        // under the key `Path`, a Lua table index is an exact byte match, and `.PATH` would stay
        // `nil` regardless of whether the leak happened. Scanning every key with a fold-insensitive
        // comparison instead makes the assertion about the property — no PATH-shaped key reaches
        // the script — rather than about one spelling of it.
        let scan = "
            local leaked = 0
            for name in pairs(airsstack.env.all()) do
                if name:upper() == 'PATH' then leaked = leaked + 1 end
            end
            return leaked";

        let engine = granted(&["AIRSL_TEST_ALL_2"]);
        let leaked: i64 = eval(&engine, scan).unwrap();
        assert_eq!(leaked, 0, "an ungranted variable reached the script");

        // The positive control: a probe that can never fire is indistinguishable from a probe
        // that finds nothing. Running the identical scan under a trusted policy — where PATH is
        // not withheld — establishes that the scan does detect a PATH-shaped key when one really
        // is present, so the assertion above is not simply true regardless of the outcome.
        let trusted_engine = Engine::builder().policy(Policy::trusted()).build().unwrap();
        let present: i64 = eval(&trusted_engine, scan).unwrap();
        assert!(
            present > 0,
            "the positive control did not find PATH under a trusted policy"
        );
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

    #[cfg(windows)]
    #[test]
    fn an_overlay_entry_is_found_under_every_casing_the_platform_calls_the_same_name() {
        // Before the fold this returned the *host's* PATH: `set` wrote the key `Path`, the
        // case-sensitive map missed on `PATH`, and `get` fell through to `std::env::var`, which
        // on Windows is case-insensitive and answers. `proc::which` looks up `PATH`, so it
        // resolved against the host while the child spawned by `proc.run` — whose `Command` env
        // map folds — received the script's. `which` and `run` disagreed.
        let overlay = Overlay::new();
        overlay.set("Path", Some(String::from(r"C:\fixture")));
        assert_eq!(overlay.get("PATH").as_deref(), Some(r"C:\fixture"));
    }

    #[cfg(unix)]
    #[test]
    fn an_overlay_entry_is_found_only_under_the_casing_it_was_written_with() {
        // The unix twin of the test above: `Path` and `PATH` are two names here, so a `Path`
        // entry must not answer for `PATH`. Folding on unix would be a widening, not a fix.
        let overlay = Overlay::new();
        overlay.set("Path", Some(String::from("/fixture")));
        assert_ne!(overlay.get("PATH").as_deref(), Some("/fixture"));
    }

    #[test]
    fn merged_names_filters_windows_pseudo_variables_from_the_host_list() {
        // Testable on macOS with a synthetic host list, which is the point of taking one rather
        // than reading `std::env::vars()` directly.
        let names = merged_names(
            [
                String::from("=C:"),
                String::from("=ExitCode"),
                String::from("HOME"),
            ],
            [],
        );
        assert_eq!(names, vec![String::from("HOME")]);
    }

    #[test]
    fn merged_names_includes_an_overlay_only_name() {
        let names = merged_names([String::from("HOME")], [String::from("EXTRA")]);
        assert_eq!(names, vec![String::from("EXTRA"), String::from("HOME")]);
    }

    #[test]
    fn merged_names_output_order_does_not_depend_on_host_iteration_order() {
        let forward = merged_names(
            [
                String::from("ZED"),
                String::from("ALPHA"),
                String::from("MID"),
            ],
            [],
        );
        let shuffled = merged_names(
            [
                String::from("MID"),
                String::from("ZED"),
                String::from("ALPHA"),
            ],
            [],
        );
        assert_eq!(forward, shuffled);
    }

    #[test]
    fn an_overlay_name_beginning_with_equals_is_not_filtered() {
        // The `=` filter exists for what the host volunteers involuntarily; a script's own write
        // under a grant is not that, and hiding it here while `get()` still answers it would be
        // the more surprising behaviour.
        let names = merged_names([], [String::from("=SCRIPT_WROTE_THIS")]);
        assert_eq!(names, vec![String::from("=SCRIPT_WROTE_THIS")]);
    }

    #[cfg(windows)]
    #[test]
    fn merged_names_prefers_host_casing_when_the_overlay_repeats_the_same_name_on_windows() {
        // One entry, not two: `Path` (host) and `PATH` (overlay) are the same identity there.
        let names = merged_names([String::from("Path")], [String::from("PATH")]);
        assert_eq!(names, vec![String::from("Path")]);
    }

    #[cfg(unix)]
    #[test]
    fn merged_names_keeps_differently_cased_host_and_overlay_names_as_two_names_on_unix() {
        let names = merged_names([String::from("PATH")], [String::from("Path")]);
        assert_eq!(names, vec![String::from("PATH"), String::from("Path")]);
    }

    #[cfg(windows)]
    #[test]
    fn setting_a_name_under_two_casings_keeps_the_first_spelling_as_the_stored_key() {
        // `BTreeMap::insert` replaces the value for an existing key but keeps the key as first
        // inserted, so `set("Path", ..)` followed by `set("PATH", ..)` stores one entry spelled
        // `Path`. That is deterministic, which is what matters here — not a bug to "fix" into
        // last-spelling-wins, which would make the stored casing depend on call order instead.
        let overlay = Overlay::new();
        overlay.set("Path", Some(String::from("first")));
        overlay.set("PATH", Some(String::from("second")));
        let entries = overlay.child_entries();
        assert_eq!(
            entries,
            vec![(String::from("Path"), Some(String::from("second")))]
        );
    }
}
