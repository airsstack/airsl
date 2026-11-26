// Probe 245: how docs-check's fifth check treats each kind of entry in crates/airsl-lua/rockspecs/, in std Rust.
// The rule probed: list the directory; skip an entry whose name (to_string_lossy) starts with `.`; classify the rest
// by name (steps 1-5, as probe 239's split.rs); for a current name, take `fs::metadata` (which follows a symlink)
// BEFORE opening it: an error -> "could not be read: {io error}"; not a regular file -> its own message; a regular file
// -> `fs::read_to_string`, whose error -> "could not be read: {io error}". Then one "has no" line per absent package.
// Usage: entries <fixture dir> ; paths are shown as crates/airsl-lua/rockspecs/<name>.
use std::fs;
use std::path::Path;

const SHOWN: &str = "crates/airsl-lua/rockspecs";
const PACKAGES: [&str; 2] = ["airsstack", "airsstack-luajit"];
const WORKSPACE: &str = "0.2.0";

fn by_name(name: &str) -> Result<&'static str, String> {
    let Some(stem) = name.strip_suffix(".rockspec") else {
        return Err("is not a rockspec; only the two rockspecs live here".to_owned());
    };
    let mut parts = stem.rsplitn(3, '-');
    let (Some(revision), Some(version), Some(package)) = (parts.next(), parts.next(), parts.next()) else {
        return Err("is not named `<package>-<version>-<revision>.rockspec`".to_owned());
    };
    if package.is_empty() || version.is_empty() || revision.is_empty() || !revision.bytes().all(|b| b.is_ascii_digit()) {
        return Err("is not named `<package>-<version>-<revision>.rockspec`".to_owned());
    }
    let Some(known) = PACKAGES.into_iter().find(|p| *p == package) else {
        return Err(format!("names package {package}, which is neither airsstack nor airsstack-luajit"));
    };
    if version != WORKSPACE {
        return Err(format!("names version {version}, which is not the workspace version {WORKSPACE}; rename it rather than keep a copy"));
    }
    if revision != "1" {
        return Err(format!("has revision {revision}; a release publishes revision 1 only"));
    }
    Ok(known)
}

fn main() {
    let dir = std::env::args().nth(1).expect("fixture dir");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("list")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut seen: Vec<&str> = Vec::new();
    for name in &names {
        let shown = format!("{SHOWN}/{name}");
        if name.starts_with('.') {
            println!("trace: skipped {name:?} (hidden)");
            continue;
        }
        let package = match by_name(name) {
            Ok(p) => p,
            Err(m) => { println!("{shown}: {m}"); continue; }
        };
        seen.push(package);
        let path = Path::new(&dir).join(name);
        match fs::metadata(&path) {
            Err(e) => println!("{shown}: could not be read: {e}"),
            Ok(m) if !m.is_file() => println!("{shown}: is not a regular file, so it cannot be a rockspec"),
            Ok(_) => match fs::read_to_string(&path) {
                Err(e) => println!("{shown}: could not be read: {e}"),
                Ok(text) => println!("trace: {shown} read, {} bytes; the per-line checks run", text.len()),
            },
        }
    }
    for package in PACKAGES {
        if !seen.contains(&package) {
            println!("{SHOWN}: has no `{package}-{WORKSPACE}-1.rockspec`, which the workspace version {WORKSPACE} needs; a version bump renames both rockspecs");
        }
    }
}
