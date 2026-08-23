//! Contributing a module this crate does not ship, under a root table of the host's choosing.
//!
//! Exists as its own example because it exercises the seam that makes `airsl` a shared Lua
//! integration point rather than a fixed script runner: a downstream crate's module goes through
//! the same [`HostModule`] trait, the same uniqueness check and the same [`InstallContext`] as the
//! twelve built-ins, so nothing here is a special case the library had to anticipate.
//!
//! Responsibilities: implementing [`HostModule`], reading authority from
//! [`InstallContext::grants`], inserting into [`airsl::modules::stdlib`], and naming a custom
//! [`RootTable`].
//!
//! Non-responsibilities: what the module measures. `record` returns a string rather than keeping a
//! counter, because the subject is the seam and a real metrics sink would bury it.

use std::error::Error;
use std::path::{Path, PathBuf};

use airsl::{Engine, GrantSet, HostModule, InstallContext, ModuleName, Policy, RootTable, Script};

/// A host module contributed the way a downstream crate would contribute one.
///
/// Holds only its name and whatever authority it was granted, which is what lets it satisfy
/// `HostModule`'s `Send + Sync` bound without thinking about it. A module reaching for an `Rc` or a
/// `RefCell` here would not compile, and that is deliberate: an [`Engine`] is shareable between
/// threads, so everything installed into one has to be too.
#[derive(Debug)]
struct Metrics {
    /// The key this module is installed under in the root table.
    name: ModuleName,
}

impl Metrics {
    /// Builds the module.
    fn new() -> Result<Self, airsl::Error> {
        Ok(Self {
            name: ModuleName::new("metrics")?,
        })
    }
}

impl HostModule for Metrics {
    fn name(&self) -> &ModuleName {
        &self.name
    }

    fn install(
        &self,
        lua: &airsl::mlua::Lua,
        table: &airsl::mlua::Table,
        context: &InstallContext<'_>,
    ) -> Result<(), airsl::Error> {
        let fail = |e: airsl::mlua::Error| airsl::Error::lua("metrics", e);

        // Asking the context rather than assuming `airsstack`. A module contributed by a third
        // party has no business hard-coding the namespace the host chose for it, and a message
        // naming the wrong path is worse than one naming none.
        let root = context.root_table().as_str().to_owned();

        // The authority is read here and captured into the function, but *checked* at the call.
        // Installation is the wrong place for the decision — a module that refused to install
        // would disappear from the table, and `if myapp.metrics then` would start meaning "am I
        // allowed" instead of "does this runtime have it".
        let may_reset = !context.grants().fs().write_roots().is_empty();
        let granted = describe(context.grants());

        let record = lua
            .create_function(move |_, name: String| Ok(format!("{root}.metrics recorded {name}")))
            .map_err(fail)?;
        table.set("record", record).map_err(fail)?;

        let reset = lua
            .create_function(move |_, ()| {
                if may_reset {
                    return Ok("metrics reset");
                }
                // The refusal names what *was* granted. A bare "denied" sends the reader to the
                // source; this sends them to their own policy, which is where the answer is.
                Err(airsl::mlua::Error::RuntimeError(format!(
                    "metrics.reset denied: it needs a filesystem write grant — granted: {granted}"
                )))
            })
            .map_err(fail)?;
        table.set("reset", reset).map_err(fail)?;

        Ok(())
    }
}

/// Renders a grant set for a refusal message.
fn describe(grants: &GrantSet) -> String {
    if grants.is_empty() {
        String::from("none")
    } else {
        grants.to_string()
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // The contributed module joins the twelve built-ins in one set. `insert` enforces name
    // uniqueness, so colliding with `json` is an error at build time rather than a module that
    // silently replaces another.
    let mut modules = airsl::modules::stdlib()?;
    modules.insert(Box::new(Metrics::new()?))?;

    let engine = Engine::builder()
        .policy(Policy::confined())
        // Without this the module would land in a namespace called `airsstack`, which belongs to
        // this crate rather than to the embedding application.
        .root_table(RootTable::new("myapp")?)
        .stdlib(modules)
        .build()?;

    println!("root table: {}", engine.root_table().as_str());
    println!(
        "modules: {}",
        engine
            .module_names()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!();

    let path: PathBuf =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/custom-module/metrics.lua");
    // Renamed rather than left as the absolute path `CARGO_MANIFEST_DIR` builds: the refusal this
    // script prints carries its chunk name, and that name is not the reader's machine's business.
    let script = Script::from_file(&path)?.with_name("metrics.lua")?;
    engine.eval(&script)?;

    Ok(())
}
