// Probe 239: the docs-check split rule for an entry of crates/airsl-lua/rockspecs/, in std Rust.
// One question: does "strip `.rockspec`, then split the stem from the right at its last two `-`;
// malformed unless package and version are non-empty and the revision is one or more ASCII digits"
// classify each name below as the spec's table says?
const PACKAGES: [&str; 2] = ["airsstack", "airsstack-luajit"];

fn classify(name: &str, workspace: &str) -> String {
    let Some(stem) = name.strip_suffix(".rockspec") else {
        return "not a rockspec".to_owned();
    };
    let mut parts = stem.rsplitn(3, '-');
    let (revision, version, package) = (parts.next(), parts.next(), parts.next());
    let (Some(revision), Some(version), Some(package)) = (revision, version, package) else {
        return "malformed (fewer than two `-`)".to_owned();
    };
    if package.is_empty() || version.is_empty() || revision.is_empty()
        || !revision.bytes().all(|b| b.is_ascii_digit())
    {
        return format!("malformed (package={package:?} version={version:?} revision={revision:?})");
    }
    if !PACKAGES.contains(&package) {
        return format!("names package {package}");
    }
    if version != workspace {
        return format!("names version {version}");
    }
    if revision != "1" {
        return format!("has revision {revision}");
    }
    format!("current: package {package}, line checks")
}

fn main() {
    let names = [
        "airsstack-0.2.0-1.rockspec",
        "airsstack-luajit-0.2.0-1.rockspec",
        "airsstack.rockspec",
        "airsstack-0.2.0.rockspec",
        "airsstack-luajit-0.2.0.rockspec",
        "airsstack-0.2.0-x.rockspec",
        "airsstack--1.rockspec",
        "-0.2.0-1.rockspec",
        ".rockspec",
        "airsstack-0.2.0-.rockspec",
        "airsstack-0.2.0-rc.1-1.rockspec",
        "airsstack-0.1.0-1.rockspec",
        "airsstack-luajit-0.1.0-1.rockspec",
        "airsstack-0.2.0-2.rockspec",
        "airsstack-0.2.0-01.rockspec",
        "other-0.2.0-1.rockspec",
        "AIRSSTACK-0.2.0-1.rockspec",
        "airsstack-0.2.0-1.ROCKSPEC",
        "airsstack-0.2.0-1.rockspec.bak",
        "notes.txt",
    ];
    for name in names {
        println!("{name:<36} -> {}", classify(name, "0.2.0"));
    }
}
