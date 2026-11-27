//! The version of this crate, readable at run time.
//!
//! Its own module because `lib.rs` holds only declarations and re-exports. An embedder needs the
//! version to record which runtime it ran a script on, or to compare the runtime it embeds with an
//! installed `airsl` binary, and without this constant it would have to parse its own
//! `Cargo.lock`.
//!
//! Responsibilities: [`VERSION`].
//!
//! Non-responsibilities: the binary's `--version` output. `airsl-cli` reports its own package
//! version.

/// The version of the `airsl` crate this program was built against, such as `"0.3.0"`.
///
/// Taken from Cargo at compile time, so it always names the library actually linked in, never the
/// one a manifest asked for.
///
/// # Examples
///
/// ```
/// let mut parts = airsl::VERSION.split('.');
/// assert!(parts.next().is_some_and(|major| major.parse::<u64>().is_ok()));
/// ```
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn the_version_has_three_numeric_components() {
        let parts: Vec<_> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "{VERSION}");
        assert!(
            parts.iter().all(|part| part.parse::<u64>().is_ok()),
            "{VERSION}"
        );
    }
}
