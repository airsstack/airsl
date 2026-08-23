//! One loaded extension: a directory, its manifest, the negotiation that admitted it, and the
//! engine it runs in.
//!
//! Separate from [`super::host`]'s registry because the load sequence is the security boundary of
//! the whole system, and it should be readable — and usable — on its own. A program that manages
//! one extension without a registry calls [`Extension::load`] with a [`LoadContext`] it built by
//! hand and gets exactly what [`super::host::ExtensionHost`] gets. The sequence is a type-state:
//! [`Extension::approve`] runs the manifest, negotiation and approver steps and yields an
//! [`Approved`]; only [`Approved::start`] builds an engine and evaluates the entry script. Nothing
//! else can reach the engine, so a denial is proved to have run before any extension code does.
//!
//! Responsibilities: [`LoadContext`], [`Approved`], [`Extension`] and the steps of the load
//! sequence, including the second containment check of the entry at start time.
//!
//! Non-responsibilities: the registry, duplicate names, `load_dir`, `broadcast` (see
//! [`super::host`]); the intersection itself ([`mod@super::negotiate`]).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::engine::Engine;
use crate::error::{Error, Result};
use crate::extension::{
    ApprovalRequest, Approver, Ceiling, Decision, Manifest, Negotiation, Variables, negotiate,
};
use crate::modules::ModuleSet;
use crate::modules::ext::Ext;
use crate::sandbox::Policy;
use crate::script::Script;
use crate::types::{EventName, ExtensionName, RootTable};

/// Everything a load needs that is not the directory.
///
/// Borrowed from the host for each load; the `modules` set is owned because the engine that
/// [`Approved::start`] builds consumes it.
pub struct LoadContext<'a, A: Approver> {
    ceiling: &'a Ceiling,
    events: &'a BTreeSet<EventName>,
    variables: &'a Variables,
    approver: &'a A,
    root_table: &'a RootTable,
    modules: ModuleSet,
}

impl<'a, A: Approver> LoadContext<'a, A> {
    /// Bundles the shared inputs of one load.
    ///
    /// `modules` must be a fresh set: the `ext` module in it is replaced with one that knows
    /// `events` before the engine is built, and the engine then takes ownership of the whole set.
    #[must_use]
    pub const fn new(
        ceiling: &'a Ceiling,
        events: &'a BTreeSet<EventName>,
        variables: &'a Variables,
        approver: &'a A,
        root_table: &'a RootTable,
        modules: ModuleSet,
    ) -> Self {
        Self {
            ceiling,
            events,
            variables,
            approver,
            root_table,
            modules,
        }
    }
}

/// A manifest that has been parsed, negotiated and approved, but not yet run.
pub struct Approved<'a, A: Approver> {
    dir: PathBuf,
    manifest: Manifest,
    negotiation: Negotiation,
    context: LoadContext<'a, A>,
}

impl<A: Approver> Approved<'_, A> {
    /// The name the manifest declared — available before any code runs, so a registry can refuse
    /// a duplicate without building an engine.
    #[must_use]
    pub const fn name(&self) -> &ExtensionName {
        self.manifest.name()
    }

    /// Builds the engine under the negotiated policy and evaluates the entry script.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManifestInvalid`] (`extension.entry`) when the entry no longer resolves
    /// inside the extension directory — it was validated once at parse time, but a symlink can be
    /// swapped in between; [`Error::ModuleNotFound`] if the module set has no `ext` to replace (a
    /// custom set that dropped it); any [`Error::EngineSetup`] or [`Error::ScriptRead`]; and
    /// whatever the entry script raises — [`Error::Lua`], [`Error::UnknownEvent`] from `ext.on`,
    /// or a resource limit breach.
    pub fn start(self) -> Result<Extension> {
        let Self {
            dir,
            manifest,
            negotiation,
            mut context,
        } = self;

        let entry = Self::recheck_entry(&dir, manifest.entry())?;
        context
            .modules
            .replace(Box::new(Ext::with_events(context.events.iter().cloned())))?;

        let engine = Engine::builder()
            .policy(negotiation.policy().clone())
            .root_table(context.root_table.clone())
            .stdlib(context.modules)
            .build()?;

        let script = Script::from_file(&entry)?
            .with_root(&dir)
            .with_name(format!(
                "{}/{}",
                manifest.name(),
                manifest.entry().display()
            ))?;
        engine.eval(&script)?;

        Ok(Extension {
            dir,
            manifest,
            negotiation,
            engine,
        })
    }

    /// Step 7 of the load sequence: the containment proof from parse time is re-run at the moment
    /// the file is about to be opened, because the path a manifest returns is the *unresolved*
    /// relative one.
    fn recheck_entry(dir: &Path, entry: &Path) -> Result<PathBuf> {
        let invalid = |reason: String| Error::ManifestInvalid {
            field: "extension.entry",
            reason,
        };
        let root = dir
            .canonicalize()
            .map_err(|e| invalid(format!("{}: {e}", dir.display())))?;
        let full = dir.join(entry);
        let resolved = full
            .canonicalize()
            .map_err(|e| invalid(format!("{}: {e}", full.display())))?;
        if !resolved.starts_with(&root) {
            return Err(invalid(format!(
                "`{}` resolves outside the extension directory",
                entry.display()
            )));
        }
        Ok(resolved)
    }
}

/// A running extension.
#[derive(Debug)]
pub struct Extension {
    dir: PathBuf,
    manifest: Manifest,
    negotiation: Negotiation,
    engine: Engine,
}

impl Extension {
    /// Steps 1–4 of the load sequence: read and validate the manifest, intersect it with the
    /// ceiling, fail closed on any required denial, then ask the approver.
    ///
    /// # Errors
    ///
    /// Every manifest error from [`Manifest::from_dir`]; [`Error::ExtensionDenied`] when a
    /// required capability is outside the ceiling (`detail` joins every denial with `; `) or the
    /// approver returns [`Decision::Deny`] (`detail` is its reason).
    pub fn approve<A: Approver>(
        dir: impl AsRef<Path>,
        context: LoadContext<'_, A>,
    ) -> Result<Approved<'_, A>> {
        let dir = dir.as_ref().to_path_buf();
        let manifest = Manifest::from_dir(&dir, context.variables)?;
        let negotiation = negotiate(&manifest, context.ceiling, &context.modules);

        if !negotiation.denied().is_empty() {
            let detail = negotiation
                .denied()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(Error::ExtensionDenied {
                extension: manifest.name().to_string(),
                detail,
            });
        }

        let request = ApprovalRequest::new(&dir, &manifest, &negotiation);
        if let Decision::Deny(reason) = context.approver.decide(&request) {
            return Err(Error::ExtensionDenied {
                extension: manifest.name().to_string(),
                detail: reason,
            });
        }

        Ok(Approved {
            dir,
            manifest,
            negotiation,
            context,
        })
    }

    /// [`Extension::approve`] followed by [`Approved::start`].
    ///
    /// # Errors
    ///
    /// The union of both.
    pub fn load<A: Approver>(dir: impl AsRef<Path>, context: LoadContext<'_, A>) -> Result<Self> {
        Self::approve(dir, context)?.start()
    }

    /// Invokes the handler registered for `event`, if any.
    ///
    /// # Errors
    ///
    /// Exactly those of [`Engine::dispatch`].
    pub fn call(
        &self,
        event: &EventName,
        payload: &serde_json::Value,
    ) -> Result<Option<serde_json::Value>> {
        self.engine.dispatch(event, payload)
    }

    /// The extension's declared name.
    #[must_use]
    pub const fn name(&self) -> &ExtensionName {
        self.manifest.name()
    }

    /// The manifest this extension was loaded from.
    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The policy negotiation actually granted — never wider than what the manifest asked for.
    #[must_use]
    pub const fn granted(&self) -> &Policy {
        self.negotiation.policy()
    }

    /// The full negotiation record: what was granted, reduced, and (had any been required) denied.
    #[must_use]
    pub const fn report(&self) -> &Negotiation {
        &self.negotiation
    }

    /// The directory this extension was loaded from.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use std::collections::BTreeSet;
    use std::fs;

    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::extension::{DenyAll, ManifestApprover};
    use crate::modules::stdlib;
    use crate::sandbox::{GrantSet, InstructionLimit, MemoryLimit, Policy, ResourceLimits};

    /// Writes `extension.toml` + `main.lua` into a fresh directory and returns it.
    fn fixture(name: &str, manifest_extra: &str, lua: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("extension.toml"),
            format!(
                "[extension]\nname = \"{name}\"\nversion = \"0.1.0\"\nentry = \"main.lua\"\napi = 1\n{manifest_extra}"
            ),
        )
        .unwrap();
        fs::write(dir.path().join("main.lua"), lua).unwrap();
        dir
    }

    fn events(names: &[&str]) -> BTreeSet<EventName> {
        names.iter().map(|n| EventName::new(*n).unwrap()).collect()
    }

    fn context<'a, A: Approver>(
        ceiling: &'a Ceiling,
        events: &'a BTreeSet<EventName>,
        variables: &'a Variables,
        approver: &'a A,
        root: &'a RootTable,
    ) -> LoadContext<'a, A> {
        LoadContext::new(
            ceiling,
            events,
            variables,
            approver,
            root,
            stdlib().unwrap(),
        )
    }

    const ECHO: &str = r#"
        airsstack.ext.on("ping", function(p) return { got = p.n } end)
    "#;

    #[test]
    fn a_clean_extension_loads_and_answers_a_call() {
        let dir = fixture("echo", "", ECHO);
        let ceiling = Ceiling::new(Policy::confined()).unwrap();
        let events = events(&["ping"]);
        let variables = Variables::none();
        let approver = ManifestApprover;
        let root = RootTable::default();

        let ext = Extension::load(
            dir.path(),
            context(&ceiling, &events, &variables, &approver, &root),
        )
        .unwrap();

        let result = ext
            .call(&EventName::new("ping").unwrap(), &json!({"n": 3}))
            .unwrap();
        assert_eq!(result, Some(json!({"got": 3})));
    }

    #[test]
    fn an_event_with_no_handler_answers_none() {
        let dir = fixture("echo", "", ECHO);
        let ceiling = Ceiling::new(Policy::confined()).unwrap();
        let events = events(&["ping", "pong"]);
        let variables = Variables::none();
        let approver = ManifestApprover;
        let root = RootTable::default();

        let ext = Extension::load(
            dir.path(),
            context(&ceiling, &events, &variables, &approver, &root),
        )
        .unwrap();

        let result = ext
            .call(&EventName::new("pong").unwrap(), &json!({}))
            .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn a_required_capability_outside_the_ceiling_is_denied_before_any_engine_exists() {
        // The manifest requests both a denied capability (`fs.read` of `/`, which no ceiling in
        // this test grants) and a granted one (`fs.write` of the extension's own directory), so
        // that the entry script *could* prove it ran by writing a marker — if it were ever given
        // the chance to. `dir` has to be known before the manifest and script are written, so this
        // test builds its fixture directly rather than through the shared `fixture` helper.
        let dir = TempDir::new().unwrap();
        let marker = dir.path().join("marker");
        fs::write(
            dir.path().join("extension.toml"),
            format!(
                "[extension]\nname = \"x\"\nversion = \"0.1.0\"\nentry = \"main.lua\"\napi = 1\n\
                 [capabilities]\nfs.read = [\"/\"]\nfs.write = [\"{}\"]\n",
                dir.path().display()
            ),
        )
        .unwrap();
        fs::write(
            dir.path().join("main.lua"),
            format!("airsstack.fs.write('{}', 'ran')\n", marker.display()),
        )
        .unwrap();

        let ceiling = Ceiling::new(
            Policy::confined().with_grants(GrantSet::declared().with_fs(|fs| fs.write(dir.path()))),
        )
        .unwrap();
        let events = events(&[]);
        let variables = Variables::none();
        let approver = ManifestApprover;
        let root = RootTable::default();

        let err = Extension::load(
            dir.path(),
            context(&ceiling, &events, &variables, &approver, &root),
        )
        .unwrap_err();

        assert!(
            matches!(&err, Error::ExtensionDenied { detail, .. } if detail.contains("fs.read") && detail.contains("outside the granted read roots")),
            "{err}"
        );
        assert!(!marker.exists(), "the entry script must not have run");
    }

    #[test]
    fn the_approver_can_refuse_an_otherwise_satisfied_request() {
        let dir = fixture("echo", "", ECHO);
        let ceiling = Ceiling::new(Policy::confined()).unwrap();
        let events = events(&["ping"]);
        let variables = Variables::none();
        let approver = DenyAll;
        let root = RootTable::default();

        let err = Extension::load(
            dir.path(),
            context(&ceiling, &events, &variables, &approver, &root),
        )
        .unwrap_err();

        assert!(
            matches!(&err, Error::ExtensionDenied { detail, .. } if detail == "extensions are disabled"),
            "{err}"
        );
    }

    #[test]
    fn an_undeclared_event_in_ext_on_fails_the_load() {
        // `ext.on` raises `Error::UnknownEvent` as a Lua error, so it surfaces at the top as
        // `Error::Lua` — a load failure, not a handler that silently never fires — but the
        // message still names what was declared.
        let dir = fixture("echo", "", r#"airsstack.ext.on("other", function() end)"#);
        let ceiling = Ceiling::new(Policy::confined()).unwrap();
        let events = events(&["ping"]);
        let variables = Variables::none();
        let approver = ManifestApprover;
        let root = RootTable::default();

        let err = Extension::load(
            dir.path(),
            context(&ceiling, &events, &variables, &approver, &root),
        )
        .unwrap_err();

        assert!(matches!(err, Error::Lua { .. }), "{err}");
        assert!(
            err.to_string().contains("is not one this host dispatches"),
            "{err}"
        );
    }

    #[test]
    fn a_runaway_entry_script_is_an_instruction_breach() {
        let dir = fixture("runaway", "", "while true do end");
        let ceiling = Ceiling::new(Policy::confined().with_limits(ResourceLimits::new(
            Some(MemoryLimit::mebibytes(64)),
            Some(InstructionLimit::count(10_000)),
        )))
        .unwrap();
        let events = events(&[]);
        let variables = Variables::none();
        let approver = ManifestApprover;
        let root = RootTable::default();

        let err = Extension::load(
            dir.path(),
            context(&ceiling, &events, &variables, &approver, &root),
        )
        .unwrap_err();

        assert!(err.exhausted_limit().is_some(), "{err}");
    }

    #[test]
    fn granted_is_the_negotiated_policy() {
        let dir = fixture("echo", "", ECHO);
        let ceiling = Ceiling::new(Policy::confined()).unwrap();
        let events = events(&["ping"]);
        let variables = Variables::none();
        let approver = ManifestApprover;
        let root = RootTable::default();

        let ext = Extension::load(
            dir.path(),
            context(&ceiling, &events, &variables, &approver, &root),
        )
        .unwrap();

        assert_eq!(ext.granted(), ext.report().policy());
    }

    #[test]
    fn an_entry_swapped_for_an_escaping_symlink_after_validation_is_refused() {
        let dir = fixture("echo", "", "return 1");
        let ceiling = Ceiling::new(Policy::confined()).unwrap();
        let events = events(&[]);
        let variables = Variables::none();
        let approver = ManifestApprover;
        let root = RootTable::default();

        let pending = Extension::approve(
            dir.path(),
            context(&ceiling, &events, &variables, &approver, &root),
        )
        .unwrap();

        let outside = TempDir::new().unwrap();
        let outside_file = outside.path().join("evil.lua");
        fs::write(&outside_file, "return 1").unwrap();
        fs::remove_file(dir.path().join("main.lua")).unwrap();
        std::os::unix::fs::symlink(&outside_file, dir.path().join("main.lua")).unwrap();

        let err = pending.start().unwrap_err();
        assert!(
            matches!(&err, Error::ManifestInvalid { field, .. } if *field == "extension.entry"),
            "{err}"
        );
    }

    #[test]
    fn approve_exposes_the_name_before_start() {
        let dir = fixture("echo", "", "return 1");
        let ceiling = Ceiling::new(Policy::confined()).unwrap();
        let events = events(&[]);
        let variables = Variables::none();
        let approver = ManifestApprover;
        let root = RootTable::default();

        let pending = Extension::approve(
            dir.path(),
            context(&ceiling, &events, &variables, &approver, &root),
        )
        .unwrap();

        assert_eq!(pending.name().as_str(), "echo");
    }

    #[test]
    fn extension_is_send_and_sync() {
        const fn assert<T: Send + Sync>() {}
        assert::<Extension>();
    }
}
