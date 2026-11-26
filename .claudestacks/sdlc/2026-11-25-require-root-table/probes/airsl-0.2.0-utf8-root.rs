//! A root named `utf8` on checkout 4620389: accepted, and what `package.loaded.utf8` holds under trusted.
use airsl::{Engine, Policy, RootTable, Script};

fn main() {
    println!("airsl {} (path dependency, checkout 4620389)", airsl::VERSION);
    println!("RootTable::new(\"utf8\") -> {:?}", RootTable::new("utf8").map(|r| r.as_str().to_owned()));
    let engine = Engine::builder().policy(Policy::trusted()).root_table(RootTable::new("utf8").unwrap()).build().unwrap();
    let out = engine.eval_to::<String>(&Script::from_source(
        "return 'global utf8 has json: ' .. tostring(utf8.json ~= nil)
          .. '; package.loaded.utf8 has charpattern: ' .. tostring(package.loaded.utf8.charpattern ~= nil)
          .. '; require(\"utf8\") is the global: ' .. tostring(rawequal(require('utf8'), utf8))", "probe").unwrap()).unwrap();
    println!("{out}");
}
