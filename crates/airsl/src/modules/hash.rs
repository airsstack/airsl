//! The `airsstack.hash` host module.
//!
//! Hashing a string needs no authority; hashing a *file* needs to read one, so `hash_file` goes
//! through the same guard `fs` uses and inherits the policy's read grants. That split is the rule
//! about modules being capabilities applied inside one module rather than between two.
//!
//! Responsibilities: [`struct@Hash`], installing `sha256`, `sha1`, `hash_file` and `hex`.
//!
//! Non-responsibilities: choosing an algorithm for the caller. Both are offered because the plugin
//! suite needs one for compatibility and the other for everything new.

use std::sync::Arc;

use sha1::Digest as _;

use crate::error::{Error, Result};
use crate::modules::guard::PathGuard;
use crate::modules::{HostModule, InstallContext};
use crate::types::ModuleName;

/// Installs `airsstack.hash`.
#[derive(Debug)]
pub struct Hash {
    name: ModuleName,
}

impl Hash {
    /// Builds the module.
    ///
    /// # Panics
    ///
    /// Never in practice: the name is a literal that satisfies [`ModuleName`]'s rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: ModuleName::new("hash")
                .unwrap_or_else(|_| unreachable!("`hash` is a valid module name")),
        }
    }
}

impl Default for Hash {
    fn default() -> Self {
        Self::new()
    }
}

/// Lowercase hexadecimal, which is what every tool the plugin suite shells out to produces.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        use core::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
        out
    })
}

impl HostModule for Hash {
    fn name(&self) -> &ModuleName {
        &self.name
    }

    fn install(
        &self,
        lua: &mlua::Lua,
        table: &mlua::Table,
        context: &InstallContext<'_>,
    ) -> Result<()> {
        let fail = |e: mlua::Error| Error::ModuleInstall {
            module: String::from("hash"),
            reason: e.to_string(),
        };
        let guard = PathGuard::new(Arc::new(context.grants().clone()), "hash");

        let sha256 = lua
            .create_function(|_, body: mlua::LuaString| {
                Ok(hex(&sha2::Sha256::digest(body.as_bytes())))
            })
            .map_err(fail)?;
        table.set("sha256", sha256).map_err(fail)?;

        // SHA-1 is present for compatibility, not because it is a good choice. The plugin suite
        // derives its per-repository project key with `hashlib.sha1(...)[:8]` and
        // `shasum | cut -c1-8`; shipping only SHA-256 would silently re-key every project and
        // orphan the SDD specs, plans and snapshots already stored under the old key.
        let sha1 = lua
            .create_function(|_, body: mlua::LuaString| {
                Ok(hex(&sha1::Sha1::digest(body.as_bytes())))
            })
            .map_err(fail)?;
        table.set("sha1", sha1).map_err(fail)?;

        let g = guard;
        let hash_file = lua
            .create_function(
                move |_, (path, algorithm): (mlua::LuaString, Option<mlua::LuaString>)| {
                    let target = g.read("hash_file", &path.to_str()?)?;
                    let body = std::fs::read(&target).map_err(|source| Error::Io {
                        operation: "hash_file",
                        path: target.display().to_string(),
                        source,
                    })?;
                    let algorithm = match algorithm.as_ref() {
                        Some(name) => name.to_str()?.to_owned(),
                        None => String::from("sha256"),
                    };
                    match algorithm.as_str() {
                        "sha256" => Ok(hex(&sha2::Sha256::digest(&body))),
                        "sha1" => Ok(hex(&sha1::Sha1::digest(&body))),
                        other => Err(mlua::Error::from(Error::Denied {
                            module: "hash",
                            operation: "hash_file",
                            detail: format!(
                                "`{other}` is not a known algorithm — use sha256 or sha1"
                            ),
                        })),
                    }
                },
            )
            .map_err(fail)?;
        table.set("hash_file", hash_file).map_err(fail)?;

        let hex_fn = lua
            .create_function(|_, body: mlua::LuaString| Ok(hex(&body.as_bytes())))
            .map_err(fail)?;
        table.set("hex", hex_fn).map_err(fail)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::Hash;
    use crate::{Engine, GrantSet, HostModule as _, Policy, Script};

    fn eval(source: &str) -> String {
        let engine = Engine::builder().policy(Policy::pure()).build().unwrap();
        engine
            .eval_to::<String>(&Script::from_source(source, "test").unwrap())
            .unwrap()
    }

    #[test]
    fn the_module_is_named_hash() {
        assert_eq!(Hash::new().name().as_str(), "hash");
    }

    #[test]
    fn sha256_matches_the_known_digest_of_abc() {
        assert_eq!(
            eval("return airsstack.hash.sha256('abc')"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha1_matches_the_known_digest_of_abc() {
        // The value `shasum` and `hashlib.sha1` produce, which is the whole point of shipping it.
        assert_eq!(
            eval("return airsstack.hash.sha1('abc')"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn the_empty_string_hashes_to_the_documented_value() {
        assert_eq!(
            eval("return airsstack.hash.sha256('')"),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn a_truncated_sha1_reproduces_the_plugin_project_key() {
        // The dispatcher's project key is `sha1(path)[:8]`; a script computing it must get the
        // same eight characters or it will not find last week's plan.
        assert_eq!(
            eval("return airsstack.hash.sha1('abc'):sub(1, 8)"),
            "a9993e36"
        );
    }

    #[test]
    fn digests_are_lowercase_hexadecimal() {
        assert_eq!(
            eval("return tostring(airsstack.hash.sha256('x'):match('^[0-9a-f]+$') ~= nil)"),
            "true"
        );
    }

    #[test]
    fn hex_encodes_arbitrary_bytes() {
        assert_eq!(eval("return airsstack.hash.hex('AB')"), "4142");
    }

    #[test]
    fn hash_file_defaults_to_sha256_and_matches_the_string_form() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "abc").unwrap();

        let engine = Engine::builder()
            .policy(
                Policy::confined()
                    .with_grants(GrantSet::declared().with_fs(|fs| fs.read(root.clone()))),
            )
            .build()
            .unwrap();
        let script = Script::from_source("return airsstack.hash.hash_file(arg[1])", "t")
            .unwrap()
            .with_args([root.join("a.txt").to_string_lossy().into_owned()]);
        assert_eq!(
            engine.eval_to::<String>(&script).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn hash_file_can_be_asked_for_sha1() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "abc").unwrap();

        let engine = Engine::builder()
            .policy(
                Policy::confined()
                    .with_grants(GrantSet::declared().with_fs(|fs| fs.read(root.clone()))),
            )
            .build()
            .unwrap();
        let script = Script::from_source("return airsstack.hash.hash_file(arg[1], 'sha1')", "t")
            .unwrap()
            .with_args([root.join("a.txt").to_string_lossy().into_owned()]);
        assert_eq!(
            engine.eval_to::<String>(&script).unwrap(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn hash_file_needs_the_read_grant_that_reading_the_file_would() {
        // Otherwise `hash` would be a way to learn a file's contents without an `fs` grant.
        let engine = Engine::builder()
            .policy(Policy::confined())
            .build()
            .unwrap();
        let script =
            Script::from_source("return airsstack.hash.hash_file('/etc/hostname')", "t").unwrap();
        let err = engine.eval_to::<String>(&script).unwrap_err();
        assert!(err.to_string().contains("hash.hash_file denied"), "{err}");
    }

    #[test]
    fn an_unknown_algorithm_is_named_in_the_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.txt"), "abc").unwrap();

        let engine = Engine::builder()
            .policy(
                Policy::confined()
                    .with_grants(GrantSet::declared().with_fs(|fs| fs.read(root.clone()))),
            )
            .build()
            .unwrap();
        let script = Script::from_source("return airsstack.hash.hash_file(arg[1], 'md5')", "t")
            .unwrap()
            .with_args([root.join("a.txt").to_string_lossy().into_owned()]);
        let err = engine.eval_to::<String>(&script).unwrap_err();
        assert!(err.to_string().contains("md5"), "{err}");
    }
}
