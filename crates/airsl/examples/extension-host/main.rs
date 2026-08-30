//! Loading a directory of third-party extensions and broadcasting one event to all of them.
//!
//! Exists as its own example because [`ExtensionHost`] is the seam that turns the crate from "a
//! host embeds one script" into "a host runs a plugin directory it never audited line by line" —
//! `load_dir` negotiates each manifest against a ceiling the host chose, never against what the
//! manifest asked for, and one denial does not stop the rest from loading. `extensions/broken`
//! exists to prove that: it asks for `fs.read = ["$OUTSIDE"]`, which this host expands to the
//! filesystem root and which is outside every ceiling this example builds, so it is refused before
//! its `main.lua` ever runs, while `word-count` loads and answers the broadcast normally.
//!
//! Responsibilities: building a host with a [`Ceiling`], loading a directory, reading a
//! [`LoadReport`] that never short-circuits, and rendering a [`Dispatch`] as byte-stable JSON.
//!
//! Non-responsibilities: the manifest format and the negotiation it goes through — see
//! `crates/airsl/docs/extensions.md`.

use std::error::Error as StdError;
use std::path::Path;

use airsl::extension::Ceiling;
use airsl::{EventName, ExtensionHost, Policy};
use serde_json::json;

/// The value `extensions/broken/extension.toml`'s `$OUTSIDE` expands to: the filesystem root,
/// spelled the way each platform requires an absolute path to be spelled. A bare `/` is rooted but
/// driveless on Windows, so the manifest validator would refuse it before negotiation ever ran —
/// this keeps the extension demonstrating a ceiling denial on both platforms, rather than turning
/// into a manifest error on one of them.
#[cfg(unix)]
const OUTSIDE_ROOT: &str = "/";
#[cfg(windows)]
const OUTSIDE_ROOT: &str = "C:\\";

fn main() -> Result<(), Box<dyn StdError>> {
    let extensions =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/extension-host/extensions");

    let mut host = ExtensionHost::builder()
        .ceiling(Ceiling::new(Policy::confined())?)
        .events(["count"])?
        .variables([("OUTSIDE", OUTSIDE_ROOT)])
        .build()?;

    let report = host.load_dir(&extensions)?;

    for name in report.loaded() {
        println!("loaded: {name}");
    }
    for (dir, error) in report.failed() {
        // The directory name, not the absolute path underneath it — the point of this line is
        // which extension failed, and that name is the one thing about the path that is not the
        // reader's own machine's business.
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("<unknown>");
        println!("failed: {name} — {error}");
    }
    println!();

    let payload = json!({ "text": "the quick brown fox jumps over the lazy dog" });
    for dispatch in host.broadcast(&EventName::new("count")?, &payload) {
        // `serde_json::Value`'s map is a `BTreeMap` in this crate's build (the `preserve_order`
        // feature is off), so `to_string` already renders keys sorted — there is no ordering left
        // for a Lua table's own construction order to leak into the output.
        let rendered = match dispatch.result() {
            Ok(Some(value)) => serde_json::to_string(value)?,
            Ok(None) => "null".to_owned(),
            Err(error) => format!("error: {error}"),
        };
        println!("{}: {rendered}", dispatch.name());
    }

    Ok(())
}
