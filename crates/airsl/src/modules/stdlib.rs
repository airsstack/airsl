//! The default set of host modules the engine installs.
//!
//! A function rather than a constant because [`ModuleSet`] owns boxed trait objects and cannot be
//! built at compile time. Keeping the default set in one place means the engine, the `doctor`
//! output of the CLI, and the tests all agree on what "the standard library" is.
//!
//! Responsibilities: [`stdlib`], which builds the default [`ModuleSet`].
//!
//! Non-responsibilities: installing the modules. The engine does that.

use crate::error::Result;
use crate::modules::{Json, ModuleSet};

/// Builds the default host-module set.
///
/// # Errors
///
/// Returns [`crate::Error::DuplicateModule`] if two entries ever claim the same name, which would
/// be a bug in this function rather than in caller input.
pub fn stdlib() -> Result<ModuleSet> {
    let mut set = ModuleSet::new();
    set.insert(Box::new(Json::new()))?;
    Ok(set)
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::stdlib;

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
}
