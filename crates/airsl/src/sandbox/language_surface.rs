//! Which of Lua's own standard libraries a script can see.
//!
//! Its own type because this is one of three independent axes a policy composes, and the only one
//! whose enforcement happens entirely at engine construction: the libraries are chosen before the
//! state exists, and the globals that survive that choice are removed immediately afterwards.
//! Nothing later can widen it.
//!
//! Responsibilities:
//!
//! - [`LanguageSurface`] and the library set each variant selects.
//! - The globals withheld after loading, and the reasoning for each group.
//!
//! Non-responsibilities: host modules. What arrives under the `airsstack` table is
//! [`crate::ModuleSet`]'s business, and a module is installed the same way whatever the surface.

use mlua::StdLib;

/// Base-library globals that hand a script a second way to load code, bypassing the surface and
/// the confined `require`. Withheld everywhere except [`LanguageSurface::Full`].
pub(crate) const CHUNK_LOADERS: [&str; 4] = ["load", "loadstring", "dofile", "loadfile"];

/// `os` functions that reach outside the process or change how the process behaves. Every
/// capability they provide is available through a host module instead, where the host controls it.
///
/// `setlocale` is on the list for a subtler reason than the rest: Lua compares strings with
/// `strcoll`, so a script that changes the locale changes the sort order of every subsequent
/// `table.sort` — which would silently break output the tooling asserts byte-for-byte.
pub(crate) const UNSAFE_OS_FUNCTIONS: [&str; 7] = [
    "execute",
    "exit",
    "getenv",
    "remove",
    "rename",
    "tmpname",
    "setlocale",
];

/// Which Lua standard libraries a script can reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum LanguageSurface {
    /// Every Lua standard library except `debug`, including `io`, `os`, `package` and `require`.
    ///
    /// For trusted first-party scripts. A script running on this surface reads and writes
    /// arbitrary files and spawns processes without going through a host module, so none of the
    /// containment the host modules provide applies to it.
    Full,

    /// The default: `string`, `table`, `math`, `coroutine`, and the pure-computation parts of `os`.
    ///
    /// Withholds `io`, `debug`, `package`, the chunk loaders, and the `os` functions that reach
    /// outside the process. Every side effect a script needs arrives through the `airsstack` host
    /// modules instead, so the host stays in control of it.
    #[default]
    Restricted,

    /// `string`, `table` and `math`, and nothing else — no `os`, no `coroutine`.
    ///
    /// For evaluating configuration, expressions and generated snippets, where the guarantees
    /// should be as strong and as easy to state as possible. Dropping `os` removes the last source
    /// of nondeterminism a script could reach without a host module: `os.time` and `os.clock`
    /// return something different on every run.
    Minimal,
}

impl LanguageSurface {
    /// The standard libraries loaded into a fresh state on this surface.
    #[must_use]
    pub(crate) fn libraries(self) -> StdLib {
        match self {
            Self::Full => StdLib::ALL_SAFE,
            Self::Restricted => {
                StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::COROUTINE | StdLib::OS
            }
            Self::Minimal => StdLib::STRING | StdLib::TABLE | StdLib::MATH,
        }
    }

    /// Whether the chunk loaders and the unsafe `os` functions are removed after loading.
    #[must_use]
    pub const fn withholds_unsafe_globals(self) -> bool {
        !matches!(self, Self::Full)
    }

    /// The name this surface reports itself under.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Restricted => "restricted",
            Self::Minimal => "minimal",
        }
    }
}

impl core::fmt::Display for LanguageSurface {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{CHUNK_LOADERS, LanguageSurface, UNSAFE_OS_FUNCTIONS};
    use mlua::StdLib;

    #[test]
    fn restricted_is_the_default_surface() {
        assert_eq!(LanguageSurface::default(), LanguageSurface::Restricted);
    }

    #[test]
    fn full_loads_every_safe_library() {
        assert_eq!(LanguageSurface::Full.libraries(), StdLib::ALL_SAFE);
    }

    #[test]
    fn restricted_loads_coroutine_and_os_but_minimal_does_not() {
        let restricted = LanguageSurface::Restricted.libraries();
        assert!(restricted.contains(StdLib::COROUTINE));
        assert!(restricted.contains(StdLib::OS));

        let minimal = LanguageSurface::Minimal.libraries();
        assert!(!minimal.contains(StdLib::COROUTINE));
        assert!(!minimal.contains(StdLib::OS));
    }

    #[test]
    fn every_surface_loads_the_pure_computation_libraries() {
        for surface in [
            LanguageSurface::Full,
            LanguageSurface::Restricted,
            LanguageSurface::Minimal,
        ] {
            let libs = surface.libraries();
            assert!(libs.contains(StdLib::STRING), "{surface}");
            assert!(libs.contains(StdLib::TABLE), "{surface}");
            assert!(libs.contains(StdLib::MATH), "{surface}");
        }
    }

    #[test]
    fn no_surface_below_full_loads_io_or_package() {
        for surface in [LanguageSurface::Restricted, LanguageSurface::Minimal] {
            let libs = surface.libraries();
            assert!(!libs.contains(StdLib::IO), "{surface}");
            assert!(!libs.contains(StdLib::PACKAGE), "{surface}");
        }
    }

    #[test]
    fn only_full_keeps_the_unsafe_globals() {
        assert!(!LanguageSurface::Full.withholds_unsafe_globals());
        assert!(LanguageSurface::Restricted.withholds_unsafe_globals());
        assert!(LanguageSurface::Minimal.withholds_unsafe_globals());
    }

    #[test]
    fn each_surface_reports_a_distinct_name() {
        assert_eq!(LanguageSurface::Full.to_string(), "full");
        assert_eq!(LanguageSurface::Restricted.to_string(), "restricted");
        assert_eq!(LanguageSurface::Minimal.to_string(), "minimal");
    }

    #[test]
    fn the_withheld_lists_name_every_route_back_to_arbitrary_code() {
        assert!(CHUNK_LOADERS.contains(&"load"));
        assert!(CHUNK_LOADERS.contains(&"loadfile"));
        assert!(UNSAFE_OS_FUNCTIONS.contains(&"execute"));
        assert!(UNSAFE_OS_FUNCTIONS.contains(&"setlocale"));
    }
}
