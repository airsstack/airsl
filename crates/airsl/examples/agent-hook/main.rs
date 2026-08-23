//! An agent hook end to end: take the JSON payload on stdin, decide, write the envelope on stdout.
//!
//! Exists as its own example because the hook contract is the one place where the *bytes on
//! stdout* are the interface. Everywhere else in this crate a script's output is for a person to
//! read; here it is parsed by the process that invoked the hook, so a stray newline, a hand-rolled
//! nesting or a well-meant progress line is a bug rather than a blemish. That is a whole shape,
//! not a function call, which is why it does not fit in a doc comment on [`airsl::modules::Hook`].
//!
//! Responsibilities: `hook.payload`, `hook.context` and `hook.emit` as the three halves of the
//! contract, `stdio.isatty` as the way a script tells a reader from a parser, and the reason only
//! the exit status can block the tool call a hook fired on.
//!
//! Non-responsibilities: choosing that exit status. This example always returns zero;
//! [`airsl::FailurePolicy`] and [`failure-policy`](../failure-policy/) are where that decision is
//! made.

use std::error::Error;
use std::path::Path;

use airsl::{Engine, GrantSet, Policy, Script};

fn main() -> Result<(), Box<dyn Error>> {
    // The script falls back to a bundled payload through `airsstack.fs`, so the host has to say
    // where that may come from. Canonicalised because the containment check compares the path the
    // operation would touch against the granted root, and the two have to be spelled the same way.
    let here = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/agent-hook")
        .canonicalize()?;

    let policy = Policy::confined().with_grants(GrantSet::declared().with_fs(|fs| fs.read(&here)));
    let engine = Engine::builder().policy(policy).build()?;

    // `with_name` because the path above is absolute and `from_file` would take the chunk name
    // from it: a raised error would then carry the developer's home directory into the traceback,
    // and a hook's stderr routinely lands in someone else's log.
    //
    // The directory travels as an argument rather than being spelled in the Lua, so the script
    // holds no path that is true only on the machine that built it.
    let script = Script::from_file(here.join("hook.lua"))?
        .with_name("hook.lua")?
        .with_args([here.to_string_lossy().into_owned()]);

    // Every byte below is written by the script. A host that printed its own framing around a
    // hook's output would be corrupting the document the caller is waiting to parse, so this one
    // says nothing at all.
    engine.eval(&script)?;

    Ok(())
}
