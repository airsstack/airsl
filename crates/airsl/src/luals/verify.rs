//! The drift check: installs one host module into a scratch Lua state and compares the keys it
//! set with the keys its description lists.
//!
//! Its own module because a host calls it alone, in its own tests, to learn that a module's
//! description and its `install` still agree. A description nobody checks goes stale the first time
//! a function is added, and the type file derived from it then lies to the editor.
//!
//! Responsibilities:
//!
//! - [`verify`], which describes a module, installs it, and reports any disagreement.
//! - [`Drift`], the structured list of disagreements and its one-line display.
//! - `describe_module`, the wrapper that turns a failed `describe` into
//!   [`Error::DescribeFailed`], shared with every caller that needs a module's description.
//!
//! Non-responsibilities: checking types. Only names and kinds are compared; that a parameter is
//! really a string is outside what a scratch install can show.

#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use core::fmt;
use std::collections::HashSet;

use mlua::Value;

use crate::description::{Member, ModuleDescription};
use crate::error::{Error, Result};
use crate::modules::{HostModule, InstallContext};
use crate::sandbox::Policy;
use crate::types::RootTable;

/// How a module's installed keys differ from its description.
///
/// Every list is sorted by the text it shows, in byte order. A [`Drift`] is only ever carried by
/// [`Error::DescriptionDrift`], so at least one list is not empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift {
    undescribed: Vec<String>,
    missing: Vec<String>,
    not_a_function: Vec<(String, String)>,
    not_a_field: Vec<String>,
}

impl Drift {
    /// Builds a drift from unsorted lists, sorting each.
    pub(crate) fn from_parts(
        mut undescribed: Vec<String>,
        mut missing: Vec<String>,
        mut not_a_function: Vec<(String, String)>,
        mut not_a_field: Vec<String>,
    ) -> Self {
        undescribed.sort();
        missing.sort();
        not_a_function.sort();
        not_a_field.sort();
        Self {
            undescribed,
            missing,
            not_a_function,
            not_a_field,
        }
    }

    const fn is_empty(&self) -> bool {
        self.undescribed.is_empty()
            && self.missing.is_empty()
            && self.not_a_function.is_empty()
            && self.not_a_field.is_empty()
    }

    /// Keys the module installs that its description does not list, as displayed text.
    ///
    /// A string key shows as itself, a number as its number and any other key as `<type>`.
    #[must_use]
    pub fn undescribed(&self) -> &[String] {
        &self.undescribed
    }

    /// Names the description lists that the module does not install.
    #[must_use]
    pub fn missing(&self) -> &[String] {
        &self.missing
    }

    /// Functions the description lists that the module installs as another kind of value, as
    /// `(name, installed type name)` pairs.
    #[must_use]
    pub fn not_a_function(&self) -> &[(String, String)] {
        &self.not_a_function
    }

    /// Fields the description lists that the module installs as a function.
    #[must_use]
    pub fn not_a_field(&self) -> &[String] {
        &self.not_a_field
    }
}

/// Backticks each name and joins them with `, `.
fn quoted(names: &[String]) -> String {
    names
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Picks the singular or plural wording for a group of `count` names.
const fn pronoun(count: usize) -> &'static str {
    if count == 1 { "it" } else { "them" }
}

impl fmt::Display for Drift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if !self.undescribed.is_empty() {
            parts.push(format!(
                "installs {} without describing {}",
                quoted(&self.undescribed),
                pronoun(self.undescribed.len())
            ));
        }
        if !self.missing.is_empty() {
            parts.push(format!(
                "describes {} but does not install {}",
                quoted(&self.missing),
                pronoun(self.missing.len())
            ));
        }
        for (name, ty) in &self.not_a_function {
            parts.push(format!(
                "describes `{name}` as a function but installs a value of type `{ty}`"
            ));
        }
        for name in &self.not_a_field {
            parts.push(format!(
                "describes `{name}` as a field but installs a function"
            ));
        }
        f.write_str(&parts.join("; "))
    }
}

/// Calls `module.describe()` and names the module when it fails.
///
/// # Errors
///
/// Returns [`Error::DescribeFailed`] wrapping whatever `describe` returned.
pub(crate) fn describe_module(module: &dyn HostModule) -> Result<ModuleDescription> {
    module.describe().map_err(|source| Error::DescribeFailed {
        module: module.name().to_string(),
        source: Box::new(source),
    })
}

/// Checks that `module` installs exactly the keys its description lists.
///
/// Installs the module into a fresh Lua state under the trusted policy and `root`, then compares
/// the table it filled with [`HostModule::describe`]. Names and kinds are compared, never types:
/// a described function must be installed as a function, a described field as something that is
/// not a function, and nothing may be installed that is not described. A record the description
/// declares is a type, not a key, so it is not expected on the table.
///
/// The table is walked raw, so keys a metatable's `__pairs` would add are not seen. A described
/// name is read the way Lua reads it, so a member served through `__index` counts as installed.
///
/// A host calls this from its own tests to keep a module's description honest.
///
/// # Errors
///
/// - [`Error::DescribeFailed`] when `describe` fails.
/// - Whatever `install` returns, unchanged.
/// - [`Error::VerifyFailed`] when the check's own use of the Lua state fails: creating the table,
///   walking it, reading a described name, or turning a key into text.
/// - [`Error::DescriptionDrift`] when the installed keys and the description disagree.
///
/// # Examples
///
/// ```
/// use airsl::description::{Function, ModuleDescription};
/// use airsl::luals::verify;
/// use airsl::{HostModule, InstallContext, ModuleName, Result, RootTable};
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
///             .create_function(|_, ()| Ok("hello"))
///             .map_err(|e| airsl::Error::lua("greeter", e))?;
///         table.set("hello", hello).map_err(|e| airsl::Error::lua("greeter", e))
///     }
///
///     fn describe(&self) -> Result<ModuleDescription> {
///         ModuleDescription::builder("Greetings.")
///             .function(Function::new("hello", "Says hello."))
///             .build()
///     }
/// }
///
/// verify(&Greeter(ModuleName::new("greeter")?), &RootTable::default())?;
/// # Ok::<(), airsl::Error>(())
/// ```
pub fn verify(module: &dyn HostModule, root: &RootTable) -> Result<()> {
    let name = module.name().to_string();
    let description = describe_module(module)?;

    let lua = mlua::Lua::new();
    let table = lua.create_table().map_err(|e| verify_failed(&name, e))?;
    let policy = Policy::trusted();
    module.install(&lua, &table, &InstallContext::new(&policy, root))?;

    let members = described_members(&description);
    let described: HashSet<&str> = members.iter().map(|member| member.name).collect();
    let mut undescribed = Vec::new();
    for pair in table.pairs::<Value, Value>() {
        let (key, _) = pair.map_err(|e| verify_failed(&name, e))?;
        let text = key_text(&key).map_err(|e| verify_failed(&name, e))?;
        if !described.contains(text.as_str()) {
            undescribed.push(text);
        }
    }

    let mut missing = Vec::new();
    let mut not_a_function = Vec::new();
    let mut not_a_field = Vec::new();
    for member in &members {
        let value: Value = table
            .get(member.name)
            .map_err(|e| verify_failed(&name, e))?;
        match (&value, member.is_function) {
            (Value::Nil, _) => missing.push(member.name.to_owned()),
            (Value::Function(_), false) => not_a_field.push(member.name.to_owned()),
            (other, true) if !matches!(other, Value::Function(_)) => {
                not_a_function.push((member.name.to_owned(), other.type_name().to_owned()));
            }
            _ => {}
        }
    }

    let drift = Drift::from_parts(undescribed, missing, not_a_function, not_a_field);
    if drift.is_empty() {
        Ok(())
    } else {
        Err(Error::DescriptionDrift {
            module: name,
            drift,
        })
    }
}

/// One key a description lists, and whether it is described as a function.
struct DescribedMember<'a> {
    name: &'a str,
    is_function: bool,
}

/// The keys `description` lists, in the order they were added. Records are types, not keys, so
/// none appear here.
fn described_members(description: &ModuleDescription) -> Vec<DescribedMember<'_>> {
    description
        .members()
        .iter()
        .map(|member| DescribedMember {
            name: member.name(),
            is_function: matches!(member, Member::Function(_)),
        })
        .collect()
}

fn verify_failed(module: &str, source: mlua::Error) -> Error {
    Error::VerifyFailed {
        module: module.to_owned(),
        source: Box::new(source),
    }
}

/// The text a table key is shown as in a drift message.
///
/// A string shows as itself, lossily when it is not UTF-8. A number shows as its number. Any other
/// key shows as `<type>`, because its own text is an address that would change between runs.
fn key_text(key: &Value) -> mlua::Result<String> {
    match key {
        Value::String(text) => Ok(String::from_utf8_lossy(&text.as_bytes()).into_owned()),
        Value::Integer(_) | Value::Number(_) => key.to_string(),
        other => Ok(format!("<{}>", other.type_name())),
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{Drift, verify};
    use crate::description::{Function, LuaType, ModuleDescription, Record};
    use crate::error::{Error, Result};
    use crate::modules::{HostModule, InstallContext};
    use crate::types::{ModuleName, RootTable};

    type Install = fn(&mlua::Lua, &mlua::Table) -> mlua::Result<()>;

    /// A module whose description and installed keys are chosen independently.
    struct Stub {
        name: ModuleName,
        description: Option<ModuleDescription>,
        install: Install,
    }

    impl Stub {
        fn new(description: Option<ModuleDescription>, install: Install) -> Self {
            Self {
                name: ModuleName::new("stub").unwrap(),
                description,
                install,
            }
        }
    }

    impl HostModule for Stub {
        fn name(&self) -> &ModuleName {
            &self.name
        }

        fn install(
            &self,
            lua: &mlua::Lua,
            table: &mlua::Table,
            _context: &InstallContext<'_>,
        ) -> Result<()> {
            (self.install)(lua, table).map_err(|e| Error::lua("stub", e))
        }

        fn describe(&self) -> Result<ModuleDescription> {
            self.description
                .clone()
                .ok_or_else(|| Error::InvalidDescription { reason: "x".into() })
        }
    }

    fn function(name: &str) -> Function {
        Function::new(name, "A function.")
    }

    fn describing(functions: &[&str]) -> ModuleDescription {
        let mut builder = ModuleDescription::builder("A stub.");
        for name in functions {
            builder = builder.function(function(name));
        }
        builder.build().unwrap()
    }

    fn noop(lua: &mlua::Lua) -> mlua::Result<mlua::Function> {
        lua.create_function(|_, ()| Ok(()))
    }

    fn install_f(lua: &mlua::Lua, table: &mlua::Table) -> mlua::Result<()> {
        table.set("f", noop(lua)?)
    }

    fn install_f_and_g(lua: &mlua::Lua, table: &mlua::Table) -> mlua::Result<()> {
        table.set("f", noop(lua)?)?;
        table.set("g", noop(lua)?)
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "it stands in for an install function, whose type returns a Result"
    )]
    fn install_nothing(_: &mlua::Lua, _: &mlua::Table) -> mlua::Result<()> {
        Ok(())
    }

    fn check(stub: &Stub) -> Result<()> {
        verify(stub, &RootTable::default())
    }

    fn drift_text(stub: &Stub) -> String {
        check(stub).unwrap_err().to_string()
    }

    #[test]
    fn a_matching_module_passes() {
        let stub = Stub::new(Some(describing(&["f"])), install_f);
        check(&stub).unwrap();
    }

    #[test]
    fn an_undescribed_key_is_drift() {
        let stub = Stub::new(Some(describing(&["f"])), install_f_and_g);
        assert_eq!(
            drift_text(&stub),
            "host module `stub` does not match its description: installs `g` without describing it"
        );
    }

    #[test]
    fn a_missing_key_is_drift() {
        let stub = Stub::new(Some(describing(&["f", "g"])), install_f);
        assert_eq!(
            drift_text(&stub),
            "host module `stub` does not match its description: describes `g` but does not install it"
        );
    }

    #[test]
    fn a_member_served_through_index_is_not_missing() {
        fn install(lua: &mlua::Lua, table: &mlua::Table) -> mlua::Result<()> {
            let served = noop(lua)?;
            let index = lua
                .create_function(move |_, (_, _): (mlua::Value, mlua::Value)| Ok(served.clone()))?;
            let meta = lua.create_table()?;
            meta.set("__index", index)?;
            table.set_metatable(Some(meta))
        }
        let stub = Stub::new(Some(describing(&["f"])), install);
        check(&stub).unwrap();
    }

    #[test]
    fn keys_seen_only_through_pairs_are_not_walked() {
        fn install(lua: &mlua::Lua, table: &mlua::Table) -> mlua::Result<()> {
            table.set("f", noop(lua)?)?;
            let pairs = lua.create_function(|lua, _: mlua::Value| {
                let fake = lua.create_table()?;
                fake.set("extra", 1)?;
                let next = lua.globals().get::<mlua::Function>("next")?;
                Ok((next, fake, mlua::Value::Nil))
            })?;
            let meta = lua.create_table()?;
            meta.set("__pairs", pairs)?;
            table.set_metatable(Some(meta))
        }
        let stub = Stub::new(Some(describing(&["f"])), install);
        check(&stub).unwrap();
    }

    #[test]
    fn a_field_installed_as_a_function_is_drift() {
        fn install(lua: &mlua::Lua, table: &mlua::Table) -> mlua::Result<()> {
            table.set("x", noop(lua)?)
        }
        let description = ModuleDescription::builder("A stub.")
            .field("x", LuaType::STRING, "A value.")
            .build()
            .unwrap();
        let stub = Stub::new(Some(description), install);
        assert!(
            drift_text(&stub).contains("describes `x` as a field but installs a function"),
            "{}",
            drift_text(&stub)
        );
    }

    #[test]
    fn a_function_installed_as_a_value_is_drift() {
        fn install(_: &mlua::Lua, table: &mlua::Table) -> mlua::Result<()> {
            table.set("f", 1)
        }
        let stub = Stub::new(Some(describing(&["f"])), install);
        assert!(
            drift_text(&stub)
                .contains("describes `f` as a function but installs a value of type `integer`"),
            "{}",
            drift_text(&stub)
        );
    }

    #[test]
    fn a_table_key_is_shown_by_its_type() {
        fn install(lua: &mlua::Lua, table: &mlua::Table) -> mlua::Result<()> {
            table.set(lua.create_table()?, 1)
        }
        let stub = Stub::new(Some(describing(&[])), install);
        assert!(
            drift_text(&stub).contains("installs `<table>` without describing it"),
            "{}",
            drift_text(&stub)
        );
    }

    #[test]
    fn a_function_key_is_shown_by_its_type() {
        fn install(lua: &mlua::Lua, table: &mlua::Table) -> mlua::Result<()> {
            table.set(noop(lua)?, 1)
        }
        let stub = Stub::new(Some(describing(&[])), install);
        assert!(
            drift_text(&stub).contains("installs `<function>` without describing it"),
            "{}",
            drift_text(&stub)
        );
    }

    #[test]
    fn a_non_utf8_key_is_shown_lossily() {
        fn install(lua: &mlua::Lua, table: &mlua::Table) -> mlua::Result<()> {
            table.set(lua.create_string(b"a\xFFb")?, 1)
        }
        let stub = Stub::new(Some(describing(&[])), install);
        assert!(
            drift_text(&stub).contains("installs `a\u{FFFD}b` without describing it"),
            "{}",
            drift_text(&stub)
        );
    }

    #[test]
    fn a_number_key_is_shown_as_its_number() {
        fn install(_: &mlua::Lua, table: &mlua::Table) -> mlua::Result<()> {
            table.set(7, 1)
        }
        let stub = Stub::new(Some(describing(&[])), install);
        assert!(
            drift_text(&stub).contains("installs `7` without describing it"),
            "{}",
            drift_text(&stub)
        );
    }

    #[test]
    fn a_vm_failure_during_verify_names_the_module() {
        fn install(lua: &mlua::Lua, table: &mlua::Table) -> mlua::Result<()> {
            let index = lua.create_function(|_, (_, _): (mlua::Value, mlua::Value)| {
                Err::<(), _>(mlua::Error::runtime("boom"))
            })?;
            let meta = lua.create_table()?;
            meta.set("__index", index)?;
            table.set_metatable(Some(meta))
        }
        let stub = Stub::new(Some(describing(&["f"])), install);
        let err = check(&stub).unwrap_err();
        assert!(matches!(err, Error::VerifyFailed { .. }), "{err}");
        assert!(
            err.to_string()
                .starts_with("cannot verify host module `stub`: "),
            "{err}"
        );
    }

    #[test]
    fn a_module_that_installs_nothing_passes() {
        let stub = Stub::new(Some(describing(&[])), install_nothing);
        check(&stub).unwrap();
    }

    #[test]
    fn drift_display_orders_and_joins_parts() {
        let drift = Drift::from_parts(
            vec!["b".into(), "a".into()],
            vec!["m".into()],
            vec![("f".into(), "integer".into())],
            vec!["x".into()],
        );
        assert_eq!(
            drift.to_string(),
            "installs `a`, `b` without describing them; \
             describes `m` but does not install it; \
             describes `f` as a function but installs a value of type `integer`; \
             describes `x` as a field but installs a function"
        );
    }

    #[test]
    fn drift_display_pluralises_the_missing_group() {
        let drift = Drift::from_parts(vec![], vec!["z".into(), "y".into()], vec![], vec![]);
        assert_eq!(
            drift.to_string(),
            "describes `y`, `z` but does not install them"
        );
    }

    #[test]
    fn drift_accessors_return_each_list_sorted() {
        let drift = Drift::from_parts(
            vec!["b".into(), "a".into()],
            vec!["d".into(), "c".into()],
            vec![("g".into(), "nil".into()), ("f".into(), "table".into())],
            vec!["y".into(), "x".into()],
        );
        assert_eq!(drift.undescribed(), ["a", "b"]);
        assert_eq!(drift.missing(), ["c", "d"]);
        assert_eq!(
            drift.not_a_function(),
            [
                ("f".to_owned(), "table".to_owned()),
                ("g".to_owned(), "nil".to_owned())
            ]
        );
        assert_eq!(drift.not_a_field(), ["x", "y"]);
    }

    #[test]
    fn a_describe_error_names_the_module() {
        let stub = Stub::new(None, install_nothing);
        assert_eq!(
            drift_text(&stub),
            "cannot describe host module `stub`: invalid module description: x"
        );
    }

    #[test]
    fn a_described_record_is_not_reported_missing() {
        let description = ModuleDescription::builder("A stub.")
            .function(function("f").returns(LuaType::record("R")))
            .record(Record::new("R", "A record.").field("n", LuaType::INTEGER, "A count."))
            .build()
            .unwrap();
        let stub = Stub::new(Some(description), install_f);
        check(&stub).unwrap();
    }

    #[test]
    fn an_install_error_is_returned_unchanged() {
        fn install(_: &mlua::Lua, _: &mlua::Table) -> mlua::Result<()> {
            Err(mlua::Error::runtime("nope"))
        }
        let stub = Stub::new(Some(describing(&[])), install);
        let err = check(&stub).unwrap_err();
        assert!(matches!(err, Error::Lua { .. }), "{err}");
    }
}
