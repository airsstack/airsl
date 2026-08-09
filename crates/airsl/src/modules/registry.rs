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
//! - [`ModuleSet`], an ordered collection of modules with name uniqueness enforced on insert.
//!
//! Non-responsibilities: installation order effects. Modules are installed in insertion order and
//! must not depend on one another's presence.

use crate::error::{Error, Result};
use crate::types::ModuleName;

/// A group of Lua functions installed as one subtable of `airsstack`.
///
/// Implementors receive an empty table and populate it. The engine has already created the table
/// and registered it under [`HostModule::name`], so an implementation only sets fields.
///
/// # Examples
///
/// ```
/// use airsl::{HostModule, ModuleName, Result};
///
/// struct Greeter(ModuleName);
///
/// impl HostModule for Greeter {
///     fn name(&self) -> &ModuleName {
///         &self.0
///     }
///
///     fn install(&self, lua: &mlua::Lua, table: &mlua::Table) -> Result<()> {
///         let hello = lua
///             .create_function(|_, who: String| Ok(format!("hello, {who}")))
///             .map_err(|e| airsl::Error::lua("greeter", e))?;
///         table.set("hello", hello).map_err(|e| airsl::Error::lua("greeter", e))
///     }
/// }
/// ```
pub trait HostModule {
    /// The key this module is installed under in the `airsstack` table.
    fn name(&self) -> &ModuleName;

    /// Populates `table` with this module's functions.
    ///
    /// # Errors
    ///
    /// Returns an error when a function cannot be created or set, which the engine reports as
    /// [`Error::ModuleInstall`].
    fn install(&self, lua: &mlua::Lua, table: &mlua::Table) -> Result<()>;
}

/// An ordered set of host modules with unique names.
///
/// Insertion order is preserved and is the order modules are installed in, so a listing of the
/// available modules reads the same way every run.
#[derive(Default)]
pub struct ModuleSet {
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

    use super::{HostModule, ModuleSet};
    use crate::error::Result;
    use crate::types::ModuleName;

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

        fn install(&self, _lua: &mlua::Lua, _table: &mlua::Table) -> Result<()> {
            Ok(())
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
}
