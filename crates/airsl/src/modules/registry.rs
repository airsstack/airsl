//! The extension seam: how Rust code contributes functions to the `airsstack` Lua table.
//!
//! Exists so the engine does not need to know the set of modules at compile time. The built-in
//! standard library and a module contributed by a downstream crate go through the same trait and
//! the same uniqueness check, which is what lets `airsl` serve as a shared Lua integration point
//! rather than a fixed script runner.
//!
//! Responsibilities:
//!
//! - [`HostModule`], the trait a contributor implements.
//! - [`InstallContext`], what a module is told about the policy it is being installed under.
//! - [`ModuleSet`], an ordered collection of modules with name uniqueness enforced on insert.
//! - `ModuleSet::install`, the one loop that builds the root table and installs each module into
//!   its own subtable, shared by the engine and by [`ModuleSet::open`].
//! - [`ModuleSet::open`], installing the set into a Lua state airsl did not create.
//!
//! Non-responsibilities: installation order effects. Modules are installed in insertion order and
//! must not depend on one another's presence.

use crate::description::ModuleDescription;
use crate::error::{Error, Result};
use crate::sandbox::{GrantSet, Policy};
use crate::types::{ModuleName, RootTable};

/// What a module is told while it is being installed, whether by an engine or by
/// [`ModuleSet::open`].
///
/// The authority a module enforces has to be the same object the policy reports, so it arrives at
/// installation rather than at construction. A module built with its own copy of the grants could
/// disagree with the engine it was installed into, and `airsl doctor` would describe the policy
/// while the module quietly enforced something else — which would make the report worthless
/// exactly where it matters. Handing the policy to `install` makes that disagreement unspellable.
///
/// A module that needs no authority ignores this. One that does captures what it needs into the
/// functions it creates, since the check belongs at the call, not at installation.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct InstallContext<'a> {
    policy: &'a Policy,
    root_table: &'a RootTable,
}

impl<'a> InstallContext<'a> {
    /// Builds the context the engine passes to each module.
    pub(crate) const fn new(policy: &'a Policy, root_table: &'a RootTable) -> Self {
        Self { policy, root_table }
    }

    /// The policy the engine was built with.
    #[must_use]
    pub const fn policy(&self) -> &Policy {
        self.policy
    }

    /// The authority this policy extends to host modules.
    #[must_use]
    pub const fn grants(&self) -> &GrantSet {
        self.policy.grants()
    }

    /// The global the modules are being installed under.
    ///
    /// Not `airsstack` for every engine — an embedding host sets its own — so a module that wants
    /// to name its own path in an error message has to ask rather than assume.
    #[must_use]
    pub const fn root_table(&self) -> &RootTable {
        self.root_table
    }
}

/// A group of Lua functions installed as one subtable of `airsstack`.
///
/// Implementors receive an empty table and populate it. The engine has already created the table
/// and registered it under [`HostModule::name`], so an implementation only sets fields.
///
/// # Examples
///
/// ```
/// use airsl::description::{Function, LuaType, ModuleDescription};
/// use airsl::{HostModule, InstallContext, ModuleName, Result};
///
/// struct Greeter(ModuleName);
///
/// impl HostModule for Greeter {
///     fn name(&self) -> &ModuleName {
///         &self.0
///     }
///
///     fn install(&self, lua: &mlua::Lua, table: &mlua::Table, _cx: &InstallContext<'_>) -> Result<()> {
///         let hello = lua
///             .create_function(|_, who: String| Ok(format!("hello, {who}")))
///             .map_err(|e| airsl::Error::lua("greeter", e))?;
///         table.set("hello", hello).map_err(|e| airsl::Error::lua("greeter", e))
///     }
///
///     fn describe(&self) -> Result<ModuleDescription> {
///         ModuleDescription::builder("Greetings.")
///             .function(
///                 Function::new("hello", "Greets someone by name.")
///                     .param("who", LuaType::STRING, "Who to greet.")
///                     .returns(LuaType::STRING),
///             )
///             .build()
///     }
/// }
/// ```
/// # Thread safety
///
/// The supertraits make an [`Engine`](crate::Engine) shareable between threads. They cost an
/// implementation nothing in practice — a module holds its name and whatever authority the host
/// granted it, and the functions it installs capture the same — but they are part of the contract:
/// a module holding a `Rc` or a `RefCell` will not compile.
pub trait HostModule: Send + Sync {
    /// The key this module is installed under in the `airsstack` table.
    fn name(&self) -> &ModuleName;

    /// Populates `table` with this module's functions.
    ///
    /// `context` carries the policy the engine was built with. A module needing no authority — a
    /// pure computation like `json` — ignores it; one that guards an operation captures the grants
    /// it needs into the functions it creates, and checks them there rather than here.
    ///
    /// # Errors
    ///
    /// Returns an error when a function cannot be created or set, which the engine reports as
    /// [`Error::ModuleInstall`].
    fn install(
        &self,
        lua: &mlua::Lua,
        table: &mlua::Table,
        context: &InstallContext<'_>,
    ) -> Result<()>;

    /// Describes the keys [`install`](Self::install) sets, for editors and for the drift check.
    ///
    /// The description lists every function and field `install` puts in the table, with the types
    /// of their parameters and results, and the records those types name. It is data about the
    /// surface, so it needs no Lua state. [`luals::verify`](crate::luals::verify) installs the
    /// module into a scratch state and fails when the two disagree, so a host calls it from its own
    /// tests to keep the description honest.
    ///
    /// # Errors
    ///
    /// Returns what [`ModuleDescriptionBuilder::build`](crate::description::ModuleDescriptionBuilder::build)
    /// returns when the description it was given breaks one of its rules.
    fn describe(&self) -> Result<ModuleDescription>;
}

/// An ordered set of host modules with unique names.
///
/// Insertion order is preserved and is the order modules are installed in, so a listing of the
/// available modules reads the same way every run.
#[derive(Default)]
pub struct ModuleSet {
    // dyn: the set is the extension seam — its members are chosen at runtime by the host and
    // contributed by crates this one never imports, so there is no closed set to enumerate.
    modules: Vec<Box<dyn HostModule>>,
}

impl ModuleSet {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
        }
    }

    /// Adds `module`, keeping insertion order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DuplicateModule`] when a module with the same name is already present.
    pub fn insert(&mut self, module: Box<dyn HostModule>) -> Result<()> {
        if self.contains(module.name()) {
            return Err(Error::DuplicateModule {
                module: module.name().to_string(),
            });
        }
        self.modules.push(module);
        Ok(())
    }

    /// Replaces the registered module with the same name, keeping its position.
    ///
    /// Exists so a host can take [`mod@crate::modules::stdlib`] and swap one built-in for a
    /// differently-configured instance — the `ext` module with the host's event list — without
    /// rebuilding the list or changing installation order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ModuleNotFound`] if no module with that name is registered; the set is
    /// left unchanged.
    pub fn replace(&mut self, module: Box<dyn HostModule>) -> Result<()> {
        let Some(slot) = self.modules.iter_mut().find(|m| m.name() == module.name()) else {
            return Err(Error::ModuleNotFound {
                module: module.name().to_string(),
            });
        };
        *slot = module;
        Ok(())
    }

    /// Whether a module is registered under `name`.
    #[must_use]
    pub fn contains(&self, name: &ModuleName) -> bool {
        self.modules.iter().any(|m| m.name() == name)
    }

    /// The registered names, in installation order.
    #[must_use]
    pub fn names(&self) -> Vec<&ModuleName> {
        self.modules.iter().map(|m| m.name()).collect()
    }

    /// How many modules are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Whether no modules are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// The modules, in installation order.
    pub fn iter(&self) -> impl Iterator<Item = &dyn HostModule> {
        self.modules.iter().map(AsRef::as_ref)
    }

    /// Creates the root table and installs every module into its own subtable, in set order.
    ///
    /// Every module is told `policy` and `root_table`, so a module that guards an operation reads
    /// its authority from the same place [`crate::Engine::policy`] reports it from. Returns the
    /// root table and sets nothing else: the global and the `require` registration belong to the
    /// caller.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ModuleInstall`] when a table cannot be created or set, and a module's own
    /// install error unchanged.
    pub(crate) fn install(
        &self,
        lua: &mlua::Lua,
        policy: &Policy,
        root_table: &RootTable,
    ) -> Result<mlua::Table> {
        let fail = |e: mlua::Error| Error::root_table_install(root_table, &e);

        let context = InstallContext::new(policy, root_table);
        let root = lua.create_table().map_err(fail)?;
        for module in self.iter() {
            let table = lua.create_table().map_err(fail)?;
            module.install(lua, &table, &context)?;
            root.set(module.name().as_str(), table).map_err(fail)?;
        }
        Ok(root)
    }

    /// Installs every module into a Lua state airsl did not create, and returns the root table.
    ///
    /// The returned table holds one subtable per module, in set order. The caller decides where it
    /// goes: this sets no global, registers nothing for `require` and does not touch
    /// `package.loaded`. It arms no memory or instruction ceiling and loads, withholds or reshapes
    /// no Lua library, so the state keeps exactly the surface its owner gave it.
    ///
    /// Modules are installed with [`Policy::trusted`] authority. A [`Policy`] is not a parameter
    /// because its language surface and resource limits cannot be applied to a state airsl did not
    /// create, and offering them would promise a confinement this call cannot deliver. Host
    /// authority is therefore unrestricted: use this only where the script is as trusted as the
    /// program loading it.
    ///
    /// State that is process-wide rather than per-state, such as the `env` overlay and the origin
    /// `time` measures from, is shared with every other Lua state in the process.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ModuleInstall`] when a table cannot be created or set, and a module's own
    /// install error unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use airsl::RootTable;
    /// use airsl::mlua::Lua;
    ///
    /// let lua = Lua::new();
    /// let modules = airsl::modules::standalone()?;
    /// let root = modules.open(&lua, &RootTable::default())?;
    ///
    /// let json: airsl::mlua::Table = root.get("json").map_err(|e| airsl::Error::lua("json", e))?;
    /// assert!(json.contains_key("encode").map_err(|e| airsl::Error::lua("json", e))?);
    /// # Ok::<(), airsl::Error>(())
    /// ```
    pub fn open(&self, lua: &mlua::Lua, root_table: &RootTable) -> Result<mlua::Table> {
        self.install(lua, &Policy::trusted(), root_table)
    }
}

impl core::fmt::Debug for ModuleSet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ModuleSet")
            .field("modules", &self.names())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use std::sync::{Arc, Mutex};

    use super::{HostModule, InstallContext, ModuleSet};
    use crate::description::ModuleDescription;
    use crate::error::Result;
    use crate::modules::Fs;
    use crate::sandbox::Policy;
    use crate::types::{ModuleName, RootTable};

    struct Failing(ModuleName);

    impl HostModule for Failing {
        fn name(&self) -> &ModuleName {
            &self.0
        }

        fn install(
            &self,
            _lua: &mlua::Lua,
            _table: &mlua::Table,
            _context: &InstallContext<'_>,
        ) -> Result<()> {
            Err(crate::Error::lua(
                "failing",
                mlua::Error::RuntimeError("refused".into()),
            ))
        }

        fn describe(&self) -> Result<ModuleDescription> {
            ModuleDescription::builder("A failing stub.").build()
        }
    }

    /// Records its name each time it is installed, so a test can observe the install order.
    struct Recorder {
        name: ModuleName,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl HostModule for Recorder {
        fn name(&self) -> &ModuleName {
            &self.name
        }

        fn install(
            &self,
            _lua: &mlua::Lua,
            _table: &mlua::Table,
            _context: &InstallContext<'_>,
        ) -> Result<()> {
            self.log.lock().unwrap().push(self.name.to_string());
            Ok(())
        }

        fn describe(&self) -> Result<ModuleDescription> {
            ModuleDescription::builder("A recording stub.").build()
        }
    }

    struct Stub(ModuleName);

    impl Stub {
        fn boxed(name: &str) -> Box<dyn HostModule> {
            Box::new(Self(ModuleName::new(name).unwrap()))
        }
    }

    impl HostModule for Stub {
        fn name(&self) -> &ModuleName {
            &self.0
        }

        fn install(
            &self,
            _lua: &mlua::Lua,
            _table: &mlua::Table,
            _context: &InstallContext<'_>,
        ) -> Result<()> {
            Ok(())
        }

        fn describe(&self) -> Result<ModuleDescription> {
            ModuleDescription::builder("A stub.").build()
        }
    }

    #[test]
    fn a_new_set_is_empty() {
        let set = ModuleSet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn insertion_order_is_preserved() {
        let mut set = ModuleSet::new();
        for name in ["json", "fs", "path"] {
            set.insert(Stub::boxed(name)).unwrap();
        }
        let names: Vec<_> = set.names().iter().map(ToString::to_string).collect();
        assert_eq!(names, ["json", "fs", "path"]);
    }

    #[test]
    fn a_duplicate_name_is_rejected_and_leaves_the_set_unchanged() {
        let mut set = ModuleSet::new();
        set.insert(Stub::boxed("fs")).unwrap();
        let err = set.insert(Stub::boxed("fs")).unwrap_err();
        assert!(err.to_string().contains("`fs`"), "{err}");
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn contains_reports_registered_names_only() {
        let mut set = ModuleSet::new();
        set.insert(Stub::boxed("fs")).unwrap();
        assert!(set.contains(&ModuleName::new("fs").unwrap()));
        assert!(!set.contains(&ModuleName::new("json").unwrap()));
    }

    #[test]
    fn iter_yields_every_module_in_order() {
        let mut set = ModuleSet::new();
        set.insert(Stub::boxed("a")).unwrap();
        set.insert(Stub::boxed("b")).unwrap();
        let seen: Vec<_> = set.iter().map(|m| m.name().to_string()).collect();
        assert_eq!(seen, ["a", "b"]);
    }

    #[test]
    fn replace_swaps_a_registered_module_in_place() {
        let mut set = ModuleSet::new();
        set.insert(Stub::boxed("a")).unwrap();
        set.insert(Stub::boxed("b")).unwrap();
        set.insert(Stub::boxed("c")).unwrap();
        set.replace(Stub::boxed("b")).unwrap();
        let seen: Vec<_> = set.iter().map(|m| m.name().to_string()).collect();
        assert_eq!(seen, ["a", "b", "c"], "order is preserved");
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn replace_refuses_a_module_that_is_not_registered() {
        let mut set = ModuleSet::new();
        set.insert(Stub::boxed("a")).unwrap();
        let err = set.replace(Stub::boxed("zzz")).unwrap_err();
        assert!(
            matches!(err, crate::Error::ModuleNotFound { ref module } if module == "zzz"),
            "{err}"
        );
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn install_hands_modules_the_policy_it_is_given() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("note.txt");
        std::fs::write(&file, "hello").unwrap();

        let mut set = ModuleSet::new();
        set.insert(Box::new(Fs::new())).unwrap();
        let lua = mlua::Lua::new();
        let root = set
            .install(&lua, &Policy::confined(), &RootTable::default())
            .unwrap();

        let fs: mlua::Table = root.get("fs").unwrap();
        let read: mlua::Function = fs.get("read").unwrap();
        let err = read
            .call::<String>(file.to_string_lossy().into_owned())
            .unwrap_err();
        assert!(
            err.to_string().contains("outside the granted read roots"),
            "{err}"
        );
    }

    #[test]
    fn open_returns_one_subtable_per_module() {
        let mut set = ModuleSet::new();
        for name in ["json", "fs", "path"] {
            set.insert(Stub::boxed(name)).unwrap();
        }
        let lua = mlua::Lua::new();
        let root = set.open(&lua, &RootTable::default()).unwrap();

        let mut keys = Vec::new();
        for pair in root.pairs::<String, mlua::Table>() {
            keys.push(pair.unwrap().0);
        }
        keys.sort();
        assert_eq!(keys, ["fs", "json", "path"]);
    }

    #[test]
    fn open_installs_modules_in_set_order() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut set = ModuleSet::new();
        for name in ["json", "fs", "path"] {
            let module = Recorder {
                name: ModuleName::new(name).unwrap(),
                log: Arc::clone(&log),
            };
            set.insert(Box::new(module)).unwrap();
        }
        let lua = mlua::Lua::new();
        set.open(&lua, &RootTable::default()).unwrap();

        let installed = log.lock().unwrap().clone();
        let expected: Vec<_> = set.names().iter().map(ToString::to_string).collect();
        assert_eq!(installed, expected);
    }

    #[test]
    fn open_sets_no_global() {
        let mut set = ModuleSet::new();
        set.insert(Stub::boxed("json")).unwrap();
        let lua = mlua::Lua::new();
        set.open(&lua, &RootTable::default()).unwrap();
        let global: mlua::Value = lua.globals().get("airsstack").unwrap();
        assert!(global.is_nil());
    }

    #[test]
    fn open_leaves_package_loaded_and_the_require_registry_untouched() {
        let mut set = ModuleSet::new();
        set.insert(Stub::boxed("json")).unwrap();
        let lua = mlua::Lua::new();
        let root_table = RootTable::default();
        set.open(&lua, &root_table).unwrap();

        let package: mlua::Table = lua.globals().get("package").unwrap();
        let loaded: mlua::Table = package.get("loaded").unwrap();
        let entry: mlua::Value = loaded.get("airsstack").unwrap();
        assert!(entry.is_nil());
        // `answer` fails only when `require::root::register` never wrote its registry value.
        assert!(crate::require::root::answer(&lua, &root_table, b"airsstack").is_err());
    }

    #[test]
    fn open_installs_with_trusted_authority() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("note.txt");
        std::fs::write(&file, "hello").unwrap();

        let mut set = ModuleSet::new();
        set.insert(Box::new(Fs::new())).unwrap();
        let lua = mlua::Lua::new();
        let root = set.open(&lua, &RootTable::default()).unwrap();

        let fs: mlua::Table = root.get("fs").unwrap();
        let read: mlua::Function = fs.get("read").unwrap();
        let content: String = read.call(file.to_string_lossy().into_owned()).unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn open_on_an_empty_set_returns_an_empty_table() {
        let lua = mlua::Lua::new();
        let root = ModuleSet::new().open(&lua, &RootTable::default()).unwrap();
        assert_eq!(root.pairs::<mlua::Value, mlua::Value>().count(), 0);
    }

    #[test]
    fn open_propagates_a_module_install_error() {
        let mut set = ModuleSet::new();
        set.insert(Box::new(Failing(ModuleName::new("bad").unwrap())))
            .unwrap();
        let lua = mlua::Lua::new();
        let err = set.open(&lua, &RootTable::default()).unwrap_err();
        let expected =
            crate::Error::lua("failing", mlua::Error::RuntimeError("refused".into())).to_string();
        assert_eq!(err.to_string(), expected);
    }
}
