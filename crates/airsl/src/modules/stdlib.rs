//! The default set of host modules the engine installs.
//!
//! A function rather than a constant because [`ModuleSet`] owns boxed trait objects and cannot be
//! built at compile time. Keeping the default set in one place means the engine, the `doctor`
//! output of the CLI, and the tests all agree on what "the standard library" is.
//!
//! Responsibilities: [`standalone`], the built-ins that need nothing from the engine, and
//! [`stdlib`], which is [`standalone`] plus `ext`. One list, so the two cannot drift.
//!
//! Non-responsibilities: installing the modules. [`ModuleSet`] does that, for an engine and for
//! [`ModuleSet::open`] alike.

use crate::error::Result;
use crate::modules::{
    Env, Ext, Fs, Glob, Hash, Hook, Json, ModuleSet, Path, Proc, Regex, Stdio, Time,
};

/// Builds the built-in host modules that work without an engine: every one of [`stdlib`] except
/// `ext`.
///
/// `ext` registers handlers in a registry table that only an airsl engine's dispatcher reads
/// back, so it exists only inside airsl. This is the set to give to a Lua state airsl did not
/// create, through [`ModuleSet::open`].
///
/// # Errors
///
/// Returns [`crate::Error::DuplicateModule`] if two entries ever claim the same name, which would
/// be a bug in this function rather than in caller input.
///
/// # Examples
///
/// ```
/// let set = airsl::modules::standalone()?;
/// let names: Vec<String> = set.names().iter().map(ToString::to_string).collect();
/// assert!(names.contains(&String::from("json")));
/// assert!(!names.contains(&String::from("ext")));
/// # Ok::<(), airsl::Error>(())
/// ```
pub fn standalone() -> Result<ModuleSet> {
    let mut set = ModuleSet::new();
    set.insert(Box::new(Json::new()))?;
    set.insert(Box::new(Path::new()))?;
    set.insert(Box::new(Fs::new()))?;
    set.insert(Box::new(Env::new()))?;
    set.insert(Box::new(Proc::new()))?;
    set.insert(Box::new(Regex::new()))?;
    set.insert(Box::new(Hash::new()))?;
    set.insert(Box::new(Time::new()))?;
    set.insert(Box::new(Glob::new()))?;
    set.insert(Box::new(Stdio::new()))?;
    set.insert(Box::new(Hook::new()))?;
    Ok(set)
}

/// Builds the default host-module set: [`standalone`] followed by `ext`.
///
/// # Errors
///
/// Returns [`crate::Error::DuplicateModule`] if two entries ever claim the same name, which would
/// be a bug in this function rather than in caller input.
pub fn stdlib() -> Result<ModuleSet> {
    let mut set = standalone()?;
    set.insert(Box::new(Ext::new()))?;
    Ok(set)
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{standalone, stdlib};

    fn names_of(set: &crate::modules::ModuleSet) -> Vec<String> {
        set.names().iter().map(ToString::to_string).collect()
    }

    #[test]
    fn the_default_set_builds_without_a_name_collision() {
        assert!(stdlib().is_ok());
    }

    #[test]
    fn the_default_set_includes_json() {
        let set = stdlib().unwrap();
        let names: Vec<_> = set.names().iter().map(ToString::to_string).collect();
        assert!(names.contains(&String::from("json")), "{names:?}");
    }

    #[test]
    fn the_default_set_is_not_empty() {
        assert!(!stdlib().unwrap().is_empty());
    }

    #[test]
    fn standalone_is_stdlib_without_ext_in_the_same_order() {
        let mut expected = names_of(&stdlib().unwrap());
        expected.retain(|n| n != "ext");
        assert_eq!(names_of(&standalone().unwrap()), expected);
    }

    #[test]
    fn stdlib_is_standalone_with_ext_last() {
        let all = names_of(&stdlib().unwrap());
        let (last, before) = all.split_last().unwrap();
        assert_eq!(last, "ext");
        assert_eq!(before, names_of(&standalone().unwrap()));
    }
}
