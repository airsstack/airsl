//! The two result shapes a host gets back from operations that touch several extensions.
//!
//! Their own module because both exist for one reason: `load_dir` and `broadcast` must not
//! short-circuit. One broken extension in a directory should not stop the others loading, and
//! one failing handler should not hide the results of the rest — so neither operation can
//! return a plain `Result`, and the per-item outcomes need a home that is not the host itself.
//!
//! Responsibilities: [`LoadReport`] (what `load_dir` loaded and what it could not) and
//! [`Dispatch`] (one extension's answer to one broadcast).
//!
//! Non-responsibilities: loading or dispatching anything — see [`super::host`].

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::types::ExtensionName;

/// What `load_dir` managed to load, and every directory it could not, in directory order.
#[derive(Debug, Default)]
pub struct LoadReport {
    loaded: Vec<ExtensionName>,
    failed: Vec<(PathBuf, Error)>,
}

impl LoadReport {
    /// Records an extension that loaded successfully.
    pub(crate) fn record_loaded(&mut self, name: ExtensionName) {
        self.loaded.push(name);
    }

    /// Records a directory that failed to load, with the error that stopped it.
    pub(crate) fn record_failed(&mut self, dir: PathBuf, error: Error) {
        self.failed.push((dir, error));
    }

    /// Names of the extensions now loaded, in load order.
    #[must_use]
    pub fn loaded(&self) -> &[ExtensionName] {
        &self.loaded
    }

    /// Each directory that did not load, with the error that stopped it.
    #[must_use]
    pub fn failed(&self) -> &[(PathBuf, Error)] {
        &self.failed
    }

    /// `true` when every candidate directory loaded.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.failed.is_empty()
    }
}

/// One extension's outcome for one broadcast event.
#[derive(Debug)]
pub struct Dispatch {
    name: ExtensionName,
    result: Result<Option<serde_json::Value>>,
}

impl Dispatch {
    /// Pairs `name` with the outcome its handler produced.
    pub(crate) const fn new(
        name: ExtensionName,
        result: Result<Option<serde_json::Value>>,
    ) -> Self {
        Self { name, result }
    }

    /// The extension that produced this outcome.
    #[must_use]
    pub const fn name(&self) -> &ExtensionName {
        &self.name
    }

    /// The handler's return value, `Ok(None)` when no handler was registered, or the error.
    #[must_use = "check the handler's outcome rather than discarding it"]
    pub const fn result(&self) -> &Result<Option<serde_json::Value>> {
        &self.result
    }

    /// Consumes the outcome.
    ///
    /// # Errors
    ///
    /// Returns whatever error the handler that produced this outcome returned, verbatim.
    pub fn into_result(self) -> Result<Option<serde_json::Value>> {
        self.result
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use std::path::PathBuf;

    use serde_json::json;

    use super::{Dispatch, LoadReport};
    use crate::error::Error;
    use crate::types::ExtensionName;

    #[test]
    fn is_clean_on_an_empty_report() {
        let report = LoadReport::default();
        assert!(report.is_clean());
        assert!(report.loaded().is_empty());
        assert!(report.failed().is_empty());
    }

    #[test]
    fn record_failed_flips_is_clean() {
        let mut report = LoadReport::default();
        report.record_failed(
            PathBuf::from("/extensions/broken"),
            Error::ManifestVariable {
                name: "MISSING".to_owned(),
            },
        );

        assert!(!report.is_clean());
        assert_eq!(report.failed().len(), 1);
        assert_eq!(report.failed()[0].0, PathBuf::from("/extensions/broken"));
    }

    #[test]
    fn dispatch_into_result_round_trips_an_ok_value() {
        let name = ExtensionName::new("journal-indexer").unwrap();
        let dispatch = Dispatch::new(name, Ok(Some(json!(1))));

        assert_eq!(dispatch.into_result().unwrap(), Some(json!(1)));
    }

    #[test]
    fn result_on_an_err_exposes_the_error_message() {
        let name = ExtensionName::new("journal-indexer").unwrap();
        let dispatch = Dispatch::new(
            name,
            Err(Error::ManifestVariable {
                name: "MISSING".to_owned(),
            }),
        );

        let err = dispatch.result().as_ref().unwrap_err();
        assert!(err.to_string().contains("MISSING"), "{err}");
    }
}
