// The `notes` host module that the "Writing Your Own Module" chapter builds, shared by the two
// programs that show it being used.
//
// It lives in its own file, pulled in with `include!`, so the module the chapter prints is the
// module both programs run, not two copies that can drift apart. Installing it into an engine is
// `main.rs`; setting up an editor for it is `setup.rs`.

// ANCHOR: module
use std::sync::{Arc, Mutex};

use airsl::description::{Function, LuaType, ModuleDescription};
use airsl::mlua::{Lua, Table};
use airsl::{HostModule, InstallContext, ModuleName};

struct Notes {
    name: ModuleName,
    collected: Arc<Mutex<Vec<String>>>,
}

impl HostModule for Notes {
    fn name(&self) -> &ModuleName {
        &self.name
    }

    fn install(
        &self,
        lua: &Lua,
        table: &Table,
        _context: &InstallContext<'_>,
    ) -> airsl::Result<()> {
        let fail = |e| airsl::Error::lua("notes", e);

        let collected = Arc::clone(&self.collected);
        let add = lua
            .create_function(move |_, text: String| {
                collected.lock().expect("notes lock poisoned").push(text);
                Ok(())
            })
            .map_err(fail)?;

        table.set("add", add).map_err(fail)
    }

    fn describe(&self) -> airsl::Result<ModuleDescription> {
        ModuleDescription::builder("Notes a script leaves for the host program.")
            .function(Function::new("add", "Records one note.").param(
                "text",
                LuaType::STRING,
                "The note to record.",
            ))
            .build()
    }
}
// ANCHOR_END: module
