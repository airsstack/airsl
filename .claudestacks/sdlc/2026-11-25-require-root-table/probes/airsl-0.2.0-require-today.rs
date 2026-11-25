//! Today's `require("<root>")` on checkout 4620389, through airsl's public API.
use airsl::{Engine, GrantSet, HostAccess, Policy, RootTable, Script};

const PROBE: &str = r#"
local ok, v = pcall(require, ROOT)
if ok then
  return "ok " .. type(v) .. (rawequal(v, _G[ROOT]) and " (the root table)" or " (not the root table)")
    .. (type(v) == "table" and v.from_user_file and " from_user_file" or "")
end
return "err " .. tostring(v)
"#;

fn run(label: &str, policy: Policy, root: &str, on_disk: bool, user_file: bool) {
    let dir = tempfile::tempdir().unwrap();
    let source = format!(
        "local ROOT = {root:?}\n{}{PROBE}",
        // Lua's own require searches package.path relative to the process cwd; point it at the
        // script's folder so a user file there is visible to it, as a script beside it would be.
        if on_disk { format!("if package then package.path = {:?} .. '/?.lua;' .. package.path end\n", dir.path().display().to_string()) } else { String::new() }
    );
    if user_file {
        std::fs::write(dir.path().join(format!("{root}.lua")), "return { from_user_file = true }").unwrap();
    }
    let script = if on_disk {
        let path = dir.path().join("main.lua");
        std::fs::write(&path, &source).unwrap();
        Script::from_file(&path).unwrap()
    } else {
        Script::from_source(source, "main").unwrap()
    };
    let engine = Engine::builder()
        .policy(policy)
        .root_table(RootTable::new(root).unwrap())
        .build()
        .unwrap();
    let out = engine.eval_to::<String>(&script).unwrap_or_else(|e| format!("eval error: {e}"));
    let out = out.replace(&dir.path().canonicalize().unwrap().display().to_string(), "<dir>")
        .replace(&dir.path().display().to_string(), "<dir>");
    println!("{label:<58} | {out}");
}

fn main() {
    println!("airsl {} (path dependency, checkout 4620389)", airsl::VERSION);
    let pure_req = || Policy::pure().with_grants(GrantSet::sealed().with_access(HostAccess::Require));
    for root in ["airsstack", "agentstacks"] {
        run(&format!("trusted  file   no user file   root={root}"), Policy::trusted(), root, true, false);
        run(&format!("trusted  file   user {root}.lua"), Policy::trusted(), root, true, true);
        run(&format!("confined file   no user file   root={root}"), Policy::confined(), root, true, false);
        run(&format!("confined file   user {root}.lua"), Policy::confined(), root, true, true);
        run(&format!("pure     file   user {root}.lua"), Policy::pure(), root, true, true);
        run(&format!("pure+req file   user {root}.lua"), pure_req(), root, true, true);
        run(&format!("trusted  source root={root}"), Policy::trusted(), root, false, false);
        run(&format!("confined source root={root}"), Policy::confined(), root, false, false);
        run(&format!("pure     source root={root}"), Policy::pure(), root, false, false);
        run(&format!("pure+req source root={root}"), pure_req(), root, false, false);
    }
}
