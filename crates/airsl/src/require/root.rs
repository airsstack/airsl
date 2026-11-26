//! Where the engine keeps its root table for `require`, and how a requested name is matched to it.
//!
//! Its own module because the registry key and the name comparison are one fact about how `require`
//! reaches the root, and the three `require` installers and the engine builder must agree on it
//! without any of them owning it.
//!
//! Responsibilities:
//!
//! - [`register`], keeping the table under a registry key a script cannot reach, and seeding Lua's
//!   own `package.loaded` where the language surface has one.
//! - [`answer`], returning that table when a `require` asks for the root's exact name.
//!
//! Non-responsibilities: choosing a `require`, which is [`super::RequireDisposition`], installing
//! one, which is [`super::RequireLoader`], and creating the table, which is the engine builder.
#![expect(
    clippy::redundant_pub_crate,
    reason = "explicit pub(crate) documents the crate-wide visibility intent at each item"
)]

use crate::error::{Error, Result};
use crate::sandbox::LanguageSurface;
use crate::types::RootTable;

/// Registry key the engine's own root table is stored under.
const ROOT_KEY: &str = "airsl.require.root";

/// Keeps `table` as the root table `require(name)` answers, on every surface.
///
/// The registry holds it so the answer survives a script replacing the global. On
/// [`LanguageSurface::Full`] it is also written to `package.loaded[name]`, so Lua's own `require`,
/// which that surface leaves alone, answers the same table; a name already present there is
/// overwritten. The match is exhaustive: a fourth surface must decide whether it has a
/// `package.loaded` before this compiles.
///
/// # Errors
///
/// Returns [`Error::EngineSetup`] when the registry value or `package.loaded` entry cannot be
/// written.
pub(crate) fn register(
    lua: &mlua::Lua,
    name: &RootTable,
    table: &mlua::Table,
    surface: LanguageSurface,
) -> Result<()> {
    let fail = |source: mlua::Error| Error::EngineSetup {
        stage: "registering the root table",
        source: Box::new(source),
    };

    lua.set_named_registry_value(ROOT_KEY, table.clone())
        .map_err(fail)?;

    match surface {
        LanguageSurface::Full => {
            let package: mlua::Table = lua.globals().get("package").map_err(fail)?;
            let loaded: mlua::Table = package.get("loaded").map_err(fail)?;
            loaded.set(name.as_str(), table.clone()).map_err(fail)
        }
        LanguageSurface::Restricted | LanguageSurface::Minimal => Ok(()),
    }
}

/// The registered root table when `requested` is exactly `name`, byte for byte.
///
/// Byte-wise and exact: a case change, a dotted sub-name or a stray space is an ordinary module
/// name, not the root. The table comes from the registry rather than the global, so a script that
/// reassigned the global still gets the engine's own table.
///
/// # Errors
///
/// Returns the `mlua` error when the registry holds no table under the root key, which means
/// [`register`] never ran on this state.
pub(crate) fn answer(
    lua: &mlua::Lua,
    name: &RootTable,
    requested: &[u8],
) -> mlua::Result<Option<mlua::Table>> {
    if requested != name.as_str().as_bytes() {
        return Ok(None);
    }
    lua.named_registry_value(ROOT_KEY).map(Some)
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{ROOT_KEY, answer, register};
    use crate::error::Error;
    use crate::sandbox::LanguageSurface;
    use crate::types::RootTable;

    fn state(surface: LanguageSurface) -> mlua::Lua {
        mlua::Lua::new_with(surface.libraries(), mlua::LuaOptions::default()).unwrap()
    }

    #[test]
    fn register_keeps_the_engine_s_table_in_the_registry() {
        let lua = state(LanguageSurface::Full);
        let name = RootTable::default();
        let table = lua.create_table().unwrap();

        register(&lua, &name, &table, LanguageSurface::Full).unwrap();

        let kept: mlua::Table = lua.named_registry_value(ROOT_KEY).unwrap();
        assert_eq!(kept.to_pointer(), table.to_pointer());
    }

    #[test]
    fn register_seeds_package_loaded_on_the_full_surface_only() {
        let name = RootTable::default();

        let lua = state(LanguageSurface::Full);
        let table = lua.create_table().unwrap();
        register(&lua, &name, &table, LanguageSurface::Full).unwrap();
        let seeded: mlua::Table = lua
            .load("return package.loaded[...]")
            .call(name.as_str())
            .unwrap();
        assert_eq!(seeded.to_pointer(), table.to_pointer());

        for surface in [LanguageSurface::Restricted, LanguageSurface::Minimal] {
            let lua = state(surface);
            let table = lua.create_table().unwrap();
            register(&lua, &name, &table, surface).unwrap();
            let package: mlua::Value = lua.globals().get("package").unwrap();
            assert!(package.is_nil(), "{surface}");
        }
    }

    #[test]
    fn register_overwrites_a_package_loaded_entry_of_the_same_name() {
        let lua = state(LanguageSurface::Full);
        let name = RootTable::new("utf8").unwrap();
        let table = lua.create_table().unwrap();

        register(&lua, &name, &table, LanguageSurface::Full).unwrap();

        let seeded: mlua::Table = lua.load("return package.loaded.utf8").eval().unwrap();
        assert_eq!(seeded.to_pointer(), table.to_pointer());
    }

    #[test]
    fn register_names_its_stage_when_package_loaded_is_not_a_table() {
        let lua = state(LanguageSurface::Full);
        lua.load("package.loaded = 1").exec().unwrap();
        let table = lua.create_table().unwrap();

        let err = register(&lua, &RootTable::default(), &table, LanguageSurface::Full).unwrap_err();

        assert!(
            matches!(
                err,
                Error::EngineSetup {
                    stage: "registering the root table",
                    ..
                }
            ),
            "{err}"
        );
        assert!(
            err.to_string()
                .contains("cannot configure the Lua state (registering the root table)"),
            "{err}"
        );
    }

    #[test]
    fn answer_matches_the_exact_root_name_only() {
        let lua = state(LanguageSurface::Minimal);
        let name = RootTable::default();
        let table = lua.create_table().unwrap();
        register(&lua, &name, &table, LanguageSurface::Minimal).unwrap();

        let found = answer(&lua, &name, b"airsstack").unwrap().unwrap();
        assert_eq!(found.to_pointer(), table.to_pointer());
        for other in [
            &b"Airsstack"[..],
            b"airsstack.fs",
            b" airsstack",
            b"airsstack ",
            b"airsstac",
        ] {
            assert!(
                answer(&lua, &name, other).unwrap().is_none(),
                "{}",
                String::from_utf8_lossy(other)
            );
        }
    }

    #[test]
    fn answer_returns_the_registry_table_after_the_global_is_replaced() {
        let lua = state(LanguageSurface::Minimal);
        let name = RootTable::default();
        let table = lua.create_table().unwrap();
        lua.globals().set(name.as_str(), table.clone()).unwrap();
        register(&lua, &name, &table, LanguageSurface::Minimal).unwrap();

        lua.globals().set(name.as_str(), mlua::Value::Nil).unwrap();

        let found = answer(&lua, &name, b"airsstack").unwrap().unwrap();
        assert_eq!(found.to_pointer(), table.to_pointer());
    }
}
