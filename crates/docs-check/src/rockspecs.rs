//! The rockspec names and contents, and the published members' versions, held to the workspace
//! version.
//!
//! Its own module because it is the only guard that reads the `rockspecs/` directory of
//! `airsl-lua` and parses the root and member manifests as TOML to get a version. A version that is
//! bumped in one place and not in the others publishes a rock that luarocks refuses or one that
//! names the wrong tag, and nothing in the build notices until a release.
//!
//! Responsibilities:
//!
//! - Requiring `airsl`, `airsl-cli` and `airsl-lua` to inherit the workspace version
//!   (`version.workspace = true`).
//! - Reading the workspace version from the root manifest.
//! - Requiring `crates/airsl-lua/rockspecs/` to hold exactly the two current rockspecs, named
//!   `<package>-<workspace version>-1.rockspec`, and ignoring hidden entries.
//! - Refusing a current rockspec that is not a regular file, before opening it, and one that cannot
//!   be read as text.
//! - Requiring the `package`, `version` and `tag` lines of each current rockspec to agree with its
//!   file name and the release tag `airsl-cli-v<version>`.
//!
//! Non-responsibilities: whether a rockspec builds, which is `cargo make rocks` and the rock
//! workflow; and the keys of a member manifest other than `version`, which cargo checks.

use std::fs;
use std::path::Path;

use toml::Value;

use crate::workspace::toml_error_line;

/// The published members' manifests, relative to the repository root.
const MEMBERS: [&str; 3] = [
    "crates/airsl/Cargo.toml",
    "crates/airsl-cli/Cargo.toml",
    "crates/airsl-lua/Cargo.toml",
];

/// The directory of the shipped rockspecs, relative to the repository root.
const ROCKSPECS_DIR: &str = "crates/airsl-lua/rockspecs";

/// The two packages a release publishes, in the order a missing rockspec is reported.
const PACKAGES: [&str; 2] = ["airsstack", "airsstack-luajit"];

/// What a `.rockspec` name that does not split into its three parts is told.
const NOT_SPLIT: &str = "is not named `<package>-<version>-<revision>.rockspec`";

/// The sentence every member-version message ends with, so the three shapes read alike.
const SHARE_ONE_NUMBER: &str = "so airsl, airsl-cli and airsl-lua share one number";

/// Every way the tree under `root` drifts from the workspace version, one line each, empty when
/// it does not.
///
/// A root manifest that gives no workspace version skips the rockspec checks, which have nothing to
/// compare with; the member checks run regardless.
fn problems(root: &Path) -> Vec<String> {
    let mut found: Vec<String> = MEMBERS
        .iter()
        .filter_map(|member| member_problem(root, member))
        .collect();
    match workspace_version(root) {
        Ok(version) => found.extend(rockspec_problems(root, &version)),
        Err(line) => found.push(line),
    }
    found
}

/// `[workspace.package].version` of the root manifest, or the problem line saying why it cannot be
/// had.
fn workspace_version(root: &Path) -> Result<String, String> {
    let text = fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|err| format!("Cargo.toml: could not be read: {err}"))?;
    let manifest: Value = toml::from_str(&text).map_err(|err| {
        format!(
            "Cargo.toml: is not valid TOML: {}",
            toml_error_line(&err.to_string())
        )
    })?;
    manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "Cargo.toml: has no string `version` under `[workspace.package]`, which the \
             rockspecs follow"
                .to_owned()
        })
}

/// Classifies every entry of the rockspecs directory by its name, then reports each package that
/// has no current entry.
fn rockspec_problems(root: &Path, workspace: &str) -> Vec<String> {
    let listed =
        fs::read_dir(root.join(ROCKSPECS_DIR)).and_then(Iterator::collect::<Result<Vec<_>, _>>);
    let entries = match listed {
        Ok(entries) => entries,
        Err(err) => return vec![format!("{ROCKSPECS_DIR}: could not be listed: {err}")],
    };
    let mut names: Vec<String> = entries
        .iter()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort();

    let mut found = Vec::new();
    let mut present: Vec<&str> = Vec::new();
    for name in &names {
        match classify(name, workspace) {
            Ok(package) => {
                present.push(package);
                let path = root.join(ROCKSPECS_DIR).join(name);
                let shown = format!("{ROCKSPECS_DIR}/{name}");
                match read_rockspec(&path) {
                    Ok(text) => found.extend(line_problems(&shown, &text, package, workspace)),
                    Err(reason) => found.push(format!("{shown}: {reason}")),
                }
            }
            Err(reason) => found.push(format!("{ROCKSPECS_DIR}/{name}: {reason}")),
        }
    }
    for package in PACKAGES {
        if !present.contains(&package) {
            found.push(format!(
                "{ROCKSPECS_DIR}: has no `{package}-{workspace}-1.rockspec`, which the workspace \
                 version {workspace} needs; a version bump renames both rockspecs"
            ));
        }
    }
    found
}

/// Checks the `package`, `version` and `tag` lines of a current rockspec against its file name and
/// the workspace version, in that order.
///
/// Each key is read from the first line that starts with its prefix, text as the shipped rockspecs
/// lay it out. A key with no such line is reported by name; one whose value differs is reported at
/// its line, and a line of the same key further down is never consulted.
fn line_problems(shown: &str, text: &str, package: &str, workspace: &str) -> Vec<String> {
    let version = format!("{workspace}-1");
    let tag = format!("airsl-cli-v{workspace}");
    let mismatch = "does not match its file name; luarocks refuses such a rockspec";
    let wrong_tag = format!("is not the release tag `{tag}`");
    let keys = [
        ("package", "package = ", package, mismatch),
        ("version", "version = ", version.as_str(), mismatch),
        ("tag", "   tag = ", tag.as_str(), wrong_tag.as_str()),
    ];
    keys.iter()
        .filter_map(|(key, prefix, expected, why)| {
            let found = text
                .lines()
                .enumerate()
                .find_map(|(index, line)| line.strip_prefix(prefix).map(|rest| (index + 1, rest)));
            match found {
                None => Some(format!("{shown}: declares no `{key} = \"…\"` line")),
                Some((line, rest)) => {
                    let declared = declared_value(rest);
                    (declared != *expected).then(|| {
                        format!("{shown}:{line}: declares `{key} = \"{declared}\"`, which {why}")
                    })
                }
            }
        })
        .collect()
}

/// The value a key line declares: the rest of the line without its trailing comma and the quotes
/// around it. A value that is not quoted is returned as written, so it never equals an expected
/// value and is reported as it reads.
fn declared_value(rest: &str) -> &str {
    let value = rest.trim_end();
    let value = value.strip_suffix(',').unwrap_or(value);
    value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(value)
}

/// The text of a current rockspec, or why it cannot be read as one.
///
/// The file type is checked before the file is opened: a directory, a FIFO or a socket (a symlink
/// to one included) is refused unopened, so a FIFO with no writer cannot block the run.
fn read_rockspec(path: &Path) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|err| format!("could not be read: {err}"))?;
    if !metadata.is_file() {
        return Err("is not a regular file, so it cannot be a rockspec".to_owned());
    }
    fs::read_to_string(path).map_err(|err| format!("could not be read: {err}"))
}

/// The package a current rockspec name belongs to, or why the name is not one. The first rule
/// that applies decides.
fn classify(name: &str, workspace: &str) -> Result<&'static str, String> {
    let Some(stem) = name.strip_suffix(".rockspec") else {
        return Err("is not a rockspec; only the two rockspecs live here".to_owned());
    };
    let mut parts = stem.rsplitn(3, '-');
    let (revision, version, package) = (parts.next(), parts.next(), parts.next());
    let (Some(revision), Some(version), Some(package)) = (revision, version, package) else {
        return Err(NOT_SPLIT.to_owned());
    };
    let is_revision = !revision.is_empty() && revision.bytes().all(|b| b.is_ascii_digit());
    if package.is_empty() || version.is_empty() || !is_revision {
        return Err(NOT_SPLIT.to_owned());
    }
    let Some(known) = PACKAGES.iter().find(|known| **known == package) else {
        return Err(format!(
            "names package {package}, which is neither airsstack nor airsstack-luajit"
        ));
    };
    if version != workspace {
        return Err(format!(
            "names version {version}, which is not the workspace version {workspace}; rename it \
             rather than keep a copy"
        ));
    }
    if revision != "1" {
        return Err(format!(
            "has revision {revision}; a release publishes revision 1 only"
        ));
    }
    Ok(known)
}

/// Reads one member manifest and says how its version fails to inherit the workspace's, if it does.
fn member_problem(root: &Path, member: &str) -> Option<String> {
    let text = match fs::read_to_string(root.join(member)) {
        Ok(text) => text,
        Err(err) => return Some(format!("{member}: could not be read: {err}")),
    };
    let manifest: Value = match toml::from_str(&text) {
        Ok(manifest) => manifest,
        Err(err) => {
            return Some(format!(
                "{member}: is not valid TOML: {}",
                toml_error_line(&err.to_string())
            ));
        }
    };
    version_problem(member, &manifest)
}

/// The first of the member-manifest rules that applies to `manifest`, as a message.
fn version_problem(member: &str, manifest: &Value) -> Option<String> {
    let Some(package) = manifest.get("package").and_then(Value::as_table) else {
        return Some(format!(
            "{member}: has no `[package]` table, so it cannot inherit the workspace version \
             (`version.workspace = true`)"
        ));
    };
    let Some(version) = package.get("version") else {
        return Some(format!(
            "{member}: declares no version; it must inherit the workspace version \
             (`version.workspace = true`) {SHARE_ONE_NUMBER}"
        ));
    };
    let kind = match version {
        Value::Table(table)
            if table.len() == 1
                && table.get("workspace").and_then(Value::as_bool) == Some(true) =>
        {
            return None;
        }
        Value::String(_) => {
            return Some(format!(
                "{member}: sets its own version, which must come from the workspace \
                 (`version.workspace = true`) {SHARE_ONE_NUMBER}"
            ));
        }
        Value::Integer(_) => "an integer",
        Value::Float(_) => "a float",
        Value::Boolean(_) => "a boolean",
        Value::Datetime(_) => "a date-time",
        Value::Array(_) => "an array",
        Value::Table(_) => "a table other than `{ workspace = true }`",
    };
    Some(format!(
        "{member}: has a version that is {kind}; it must be `version.workspace = true` \
         {SHARE_ONE_NUMBER}"
    ))
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "fixtures are known to be creatable; a panic is the failure signal"
    )]

    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::{MEMBERS, ROCKSPECS_DIR as DIR, problems};
    use crate::workspace;

    const CURRENT: &str = "airsstack-0.2.0-1.rockspec";
    const CURRENT_LUAJIT: &str = "airsstack-luajit-0.2.0-1.rockspec";

    const NO_STRING_VERSION: &str = "Cargo.toml: has no string `version` under \
                                     `[workspace.package]`, which the rockspecs follow";

    const MEMBER_MANIFEST: &str = "[package]\nname = \"member\"\nversion.workspace = true\n";

    /// A rockspec laid out as the shipped ones are: `package` on line 2, `version` on line 3 and
    /// `tag` on line 6.
    fn rockspec(package: &str, version: &str) -> String {
        format!(
            "rockspec_format = \"3.0\"\n\
             package = \"{package}\"\n\
             version = \"{version}-1\"\n\
             source = {{\n   url = \"git+https://github.com/airsstack/airsl\",\n   \
             tag = \"airsl-cli-v{version}\",\n}}\n"
        )
    }

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// A whole valid tree: workspace version 0.2.0, three inheriting members, both current
    /// rockspecs. Every test changes one input of it.
    fn fixture() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        write(
            root,
            "Cargo.toml",
            "[workspace.package]\nversion = \"0.2.0\"\n",
        );
        for member in MEMBERS {
            write(root, member, MEMBER_MANIFEST);
        }
        write(
            root,
            "crates/airsl-lua/rockspecs/airsstack-0.2.0-1.rockspec",
            &rockspec("airsstack", "0.2.0"),
        );
        write(
            root,
            "crates/airsl-lua/rockspecs/airsstack-luajit-0.2.0-1.rockspec",
            &rockspec("airsstack-luajit", "0.2.0"),
        );
        dir
    }

    #[test]
    fn the_fixture_tree_has_no_problem() {
        let dir = fixture();
        assert_eq!(problems(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn a_member_that_does_not_inherit_the_workspace_version_is_reported() {
        let dir = fixture();
        write(
            dir.path(),
            "crates/airsl/Cargo.toml",
            "[package]\nname = \"airsl\"\nversion = \"0.2.0\"\n",
        );
        assert_eq!(
            problems(dir.path()),
            vec![
                "crates/airsl/Cargo.toml: sets its own version, which must come from the workspace \
                 (`version.workspace = true`) so airsl, airsl-cli and airsl-lua share one number"
            ]
        );
    }

    #[test]
    fn a_member_without_a_package_table_is_reported() {
        let dir = fixture();
        write(
            dir.path(),
            "crates/airsl/Cargo.toml",
            "[lib]\nname = \"x\"\n",
        );
        assert_eq!(
            problems(dir.path()),
            vec![
                "crates/airsl/Cargo.toml: has no `[package]` table, so it cannot inherit the \
                 workspace version (`version.workspace = true`)"
            ]
        );
    }

    #[test]
    fn a_member_without_a_version_is_reported() {
        let dir = fixture();
        write(
            dir.path(),
            "crates/airsl-cli/Cargo.toml",
            "[package]\nname = \"airsl-cli\"\n",
        );
        assert_eq!(
            problems(dir.path()),
            vec![
                "crates/airsl-cli/Cargo.toml: declares no version; it must inherit the workspace \
                 version (`version.workspace = true`) so airsl, airsl-cli and airsl-lua share one \
                 number"
            ]
        );
    }

    #[test]
    fn a_member_version_of_another_type_is_named_by_its_type() {
        let dir = fixture();
        write(
            dir.path(),
            "crates/airsl/Cargo.toml",
            "[package]\nname = \"airsl\"\nversion = 1\n",
        );
        write(
            dir.path(),
            "crates/airsl-cli/Cargo.toml",
            "[package]\nname = \"airsl-cli\"\nversion.workspace = false\n",
        );
        assert_eq!(
            problems(dir.path()),
            vec![
                "crates/airsl/Cargo.toml: has a version that is an integer; it must be \
                 `version.workspace = true` so airsl, airsl-cli and airsl-lua share one number",
                "crates/airsl-cli/Cargo.toml: has a version that is a table other than \
                 `{ workspace = true }`; it must be `version.workspace = true` so airsl, airsl-cli \
                 and airsl-lua share one number",
            ]
        );
    }

    #[test]
    fn an_inline_workspace_table_counts_as_inheriting() {
        let dir = fixture();
        write(
            dir.path(),
            "crates/airsl/Cargo.toml",
            "[package]\nname = \"airsl\"\nversion = { workspace = true }\n",
        );
        assert_eq!(problems(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn a_member_that_is_not_valid_toml_shows_the_position_and_the_reason() {
        let dir = fixture();
        write(dir.path(), "crates/airsl/Cargo.toml", "[package");
        assert_eq!(
            problems(dir.path()),
            vec![
                "crates/airsl/Cargo.toml: is not valid TOML: TOML parse error at line 1, column 9: \
                 unclosed table, expected `]`"
            ]
        );
    }

    #[test]
    fn a_toml_error_without_a_position_is_shown_alone() {
        let dir = fixture();
        let deep = vec!["a"; 100].join(".");
        write(
            dir.path(),
            "crates/airsl/Cargo.toml",
            &format!("[package]\n{deep} = 1\n"),
        );
        assert_eq!(
            problems(dir.path()),
            vec!["crates/airsl/Cargo.toml: is not valid TOML: recursion limit"]
        );
    }

    fn set_workspace_version(root: &Path, version: &str) {
        write(
            root,
            "Cargo.toml",
            &format!("[workspace.package]\nversion = \"{version}\"\n"),
        );
    }

    fn remove_rockspec(root: &Path, name: &str) {
        fs::remove_file(root.join(DIR).join(name)).unwrap();
    }

    fn has_no(package: &str, version: &str) -> String {
        format!(
            "{DIR}: has no `{package}-{version}-1.rockspec`, which the workspace version \
             {version} needs; a version bump renames both rockspecs"
        )
    }

    #[test]
    fn a_workspace_version_that_is_missing_or_not_a_string_is_reported() {
        for root_manifest in [
            "[workspace.package]\nedition = \"2024\"\n",
            "[workspace.package]\nversion = 2\n",
        ] {
            let dir = fixture();
            write(dir.path(), "Cargo.toml", root_manifest);
            assert_eq!(problems(dir.path()), vec![NO_STRING_VERSION]);
        }
    }

    #[test]
    fn a_root_manifest_that_is_not_valid_toml_is_reported_and_the_members_still_run() {
        let dir = fixture();
        write(dir.path(), "Cargo.toml", "[workspace.package");
        write(
            dir.path(),
            "crates/airsl/Cargo.toml",
            "[package]\nname = \"airsl\"\nversion = \"0.2.0\"\n",
        );
        let found = problems(dir.path());
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].starts_with("crates/airsl/Cargo.toml: sets its own version"));
        assert_eq!(
            found[1],
            "Cargo.toml: is not valid TOML: TOML parse error at line 1, column 19: unclosed \
             table, expected `]`"
        );
    }

    #[test]
    fn a_root_manifest_that_cannot_be_read_is_reported() {
        let dir = fixture();
        fs::remove_file(dir.path().join("Cargo.toml")).unwrap();
        let found = problems(dir.path());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("Cargo.toml: could not be read: "));
    }

    #[test]
    fn a_missing_rockspec_is_reported() {
        let dir = fixture();
        set_workspace_version(dir.path(), "0.3.0");
        remove_rockspec(dir.path(), "airsstack-luajit-0.2.0-1.rockspec");
        let found = problems(dir.path());
        assert!(found.contains(&has_no("airsstack", "0.3.0")), "{found:?}");
        assert!(
            found.contains(&format!(
                "{DIR}/airsstack-0.2.0-1.rockspec: names version 0.2.0, which is not the \
                 workspace version 0.3.0; rename it rather than keep a copy"
            )),
            "{found:?}"
        );
    }

    #[test]
    fn a_leftover_rockspec_of_another_version_is_reported() {
        let dir = fixture();
        write(
            dir.path(),
            "crates/airsl-lua/rockspecs/airsstack-0.1.0-1.rockspec",
            &rockspec("airsstack", "0.1.0"),
        );
        assert_eq!(
            problems(dir.path()),
            vec![format!(
                "{DIR}/airsstack-0.1.0-1.rockspec: names version 0.1.0, which is not the \
                 workspace version 0.2.0; rename it rather than keep a copy"
            )]
        );
    }

    #[test]
    fn a_revision_other_than_one_is_reported() {
        let dir = fixture();
        write(
            dir.path(),
            "crates/airsl-lua/rockspecs/airsstack-0.2.0-2.rockspec",
            "",
        );
        assert_eq!(
            problems(dir.path()),
            vec![format!(
                "{DIR}/airsstack-0.2.0-2.rockspec: has revision 2; a release publishes \
                 revision 1 only"
            )]
        );
    }

    #[test]
    fn an_unknown_package_is_reported() {
        let dir = fixture();
        write(
            dir.path(),
            "crates/airsl-lua/rockspecs/other-0.2.0-1.rockspec",
            "",
        );
        assert_eq!(
            problems(dir.path()),
            vec![format!(
                "{DIR}/other-0.2.0-1.rockspec: names package other, which is neither airsstack \
                 nor airsstack-luajit"
            )]
        );
    }

    #[test]
    fn a_file_that_is_not_a_rockspec_is_reported() {
        let dir = fixture();
        write(dir.path(), "crates/airsl-lua/rockspecs/notes.txt", "");
        assert_eq!(
            problems(dir.path()),
            vec![format!(
                "{DIR}/notes.txt: is not a rockspec; only the two rockspecs live here"
            )]
        );
    }

    #[test]
    fn a_rockspec_name_that_does_not_split_is_reported() {
        for name in [
            "airsstack.rockspec",
            "airsstack-0.2.0.rockspec",
            "airsstack-luajit-0.2.0.rockspec",
            "airsstack--1.rockspec",
            "airsstack-0.2.0-x.rockspec",
        ] {
            let dir = fixture();
            write(dir.path(), &format!("{DIR}/{name}"), "");
            assert_eq!(
                problems(dir.path()),
                vec![format!(
                    "{DIR}/{name}: is not named `<package>-<version>-<revision>.rockspec`"
                )],
                "{name}"
            );
        }
    }

    #[test]
    fn the_luajit_rockspec_name_splits_as_package_airsstack_luajit() {
        let dir = fixture();
        remove_rockspec(dir.path(), "airsstack-0.2.0-1.rockspec");
        assert_eq!(problems(dir.path()), vec![has_no("airsstack", "0.2.0")]);
    }

    #[test]
    fn a_hidden_entry_is_skipped() {
        let dir = fixture();
        for hidden in [".DS_Store", ".gitkeep", ".rockspec"] {
            write(dir.path(), &format!("{DIR}/{hidden}"), "");
        }
        fs::create_dir(dir.path().join(DIR).join(".hidden")).unwrap();
        assert_eq!(problems(dir.path()), Vec::<String>::new());

        remove_rockspec(dir.path(), "airsstack-0.2.0-1.rockspec");
        remove_rockspec(dir.path(), "airsstack-luajit-0.2.0-1.rockspec");
        assert_eq!(
            problems(dir.path()),
            vec![
                has_no("airsstack", "0.2.0"),
                has_no("airsstack-luajit", "0.2.0")
            ]
        );
    }

    #[test]
    fn a_current_name_that_is_a_directory_is_not_a_regular_file() {
        let dir = fixture();
        remove_rockspec(dir.path(), CURRENT);
        fs::create_dir(dir.path().join(DIR).join(CURRENT)).unwrap();
        assert_eq!(
            problems(dir.path()),
            vec![format!(
                "{DIR}/{CURRENT}: is not a regular file, so it cannot be a rockspec"
            )]
        );
    }

    /// How long the FIFO test waits for the check before it calls it blocked.
    #[cfg(unix)]
    const BLOCKED_AFTER: std::time::Duration = std::time::Duration::from_secs(5);

    #[cfg(unix)]
    #[test]
    fn a_current_name_that_is_a_fifo_is_reported_without_being_opened() {
        let dir = fixture();
        remove_rockspec(dir.path(), CURRENT_LUAJIT);
        let fifo = dir.path().join(DIR).join(CURRENT_LUAJIT);
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(made.success());
        // An open of a FIFO with no writer blocks for good, so the check runs on a worker thread
        // and a check that has not returned in time fails the test instead of hanging the run.
        // The blocked thread is left behind; it ends with the test process.
        let root = dir.path().to_path_buf();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(problems(&root));
        });
        let found = receiver.recv_timeout(BLOCKED_AFTER);
        assert!(
            found.is_ok(),
            "the check blocked on the FIFO: it opened the entry before checking its type"
        );
        assert_eq!(
            found.unwrap(),
            vec![format!(
                "{DIR}/{CURRENT_LUAJIT}: is not a regular file, so it cannot be a rockspec"
            )]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_is_reported_as_unreadable() {
        let dir = fixture();
        remove_rockspec(dir.path(), CURRENT);
        std::os::unix::fs::symlink(
            dir.path().join("nowhere"),
            dir.path().join(DIR).join(CURRENT),
        )
        .unwrap();
        let found = problems(dir.path());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with(&format!("{DIR}/{CURRENT}: could not be read: ")),
            "{found:?}"
        );
    }

    #[test]
    fn a_rockspec_that_is_not_utf8_is_reported_as_unreadable() {
        let dir = fixture();
        fs::write(
            dir.path().join(DIR).join(CURRENT),
            b"package = \"airsstack\"\n\xff\xfe\n",
        )
        .unwrap();
        assert_eq!(
            problems(dir.path()),
            vec![format!(
                "{DIR}/{CURRENT}: could not be read: stream did not contain valid UTF-8"
            )]
        );
    }

    fn replace_in(root: &Path, name: &str, from: &str, to: &str) {
        let path = root.join(DIR).join(name);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{from} is not in the fixture");
        fs::write(path, text.replace(from, to)).unwrap();
    }

    #[test]
    fn a_package_line_that_disagrees_with_the_file_name_is_reported() {
        let dir = fixture();
        replace_in(
            dir.path(),
            CURRENT,
            "package = \"airsstack\"",
            "package = \"airsstack-lua\"",
        );
        assert_eq!(
            problems(dir.path()),
            vec![format!(
                "{DIR}/{CURRENT}:2: declares `package = \"airsstack-lua\"`, which does not match \
                 its file name; luarocks refuses such a rockspec"
            )]
        );
    }

    #[test]
    fn a_version_line_that_disagrees_with_the_file_name_is_reported() {
        let dir = fixture();
        replace_in(
            dir.path(),
            CURRENT_LUAJIT,
            "version = \"0.2.0-1\"",
            "version = \"0.1.0-1\"",
        );
        assert_eq!(
            problems(dir.path()),
            vec![format!(
                "{DIR}/{CURRENT_LUAJIT}:3: declares `version = \"0.1.0-1\"`, which does not match \
                 its file name; luarocks refuses such a rockspec"
            )]
        );
    }

    #[test]
    fn a_tag_that_is_not_the_release_tag_is_reported() {
        let dir = fixture();
        replace_in(
            dir.path(),
            CURRENT,
            "tag = \"airsl-cli-v0.2.0\"",
            "tag = \"airsl-cli-v0.1.0\"",
        );
        assert_eq!(
            problems(dir.path()),
            vec![format!(
                "{DIR}/{CURRENT}:6: declares `tag = \"airsl-cli-v0.1.0\"`, which is not the \
                 release tag `airsl-cli-v0.2.0`"
            )]
        );
    }

    #[test]
    fn a_missing_line_is_reported() {
        for (key, from) in [
            ("package", "package = \"airsstack\"\n"),
            ("version", "version = \"0.2.0-1\"\n"),
            ("tag", "   tag = \"airsl-cli-v0.2.0\",\n"),
        ] {
            let dir = fixture();
            replace_in(dir.path(), CURRENT, from, "");
            assert_eq!(
                problems(dir.path()),
                vec![format!("{DIR}/{CURRENT}: declares no `{key} = \"…\"` line")],
                "{key}"
            );
        }
    }

    #[test]
    fn a_file_missing_several_lines_gets_one_message_each_in_key_order() {
        let dir = fixture();
        fs::write(
            dir.path().join(DIR).join(CURRENT),
            "rockspec_format = \"3.0\"\n",
        )
        .unwrap();
        assert_eq!(
            problems(dir.path()),
            vec![
                format!("{DIR}/{CURRENT}: declares no `package = \"…\"` line"),
                format!("{DIR}/{CURRENT}: declares no `version = \"…\"` line"),
                format!("{DIR}/{CURRENT}: declares no `tag = \"…\"` line"),
            ]
        );
    }

    #[test]
    fn a_leftover_rockspec_gets_one_message_and_no_line_checks() {
        let dir = fixture();
        write(
            dir.path(),
            "crates/airsl-lua/rockspecs/airsstack-0.1.0-1.rockspec",
            &rockspec("other", "9.9.9"),
        );
        assert_eq!(
            problems(dir.path()),
            vec![format!(
                "{DIR}/airsstack-0.1.0-1.rockspec: names version 0.1.0, which is not the \
                 workspace version 0.2.0; rename it rather than keep a copy"
            )]
        );
    }

    #[test]
    fn an_unreadable_rockspec_is_reported() {
        let dir = fixture();
        fs::write(dir.path().join(DIR).join(CURRENT), [0xff_u8]).unwrap();
        assert_eq!(
            problems(dir.path()),
            vec![format!(
                "{DIR}/{CURRENT}: could not be read: stream did not contain valid UTF-8"
            )]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_a_rockspec_file_is_read() {
        let dir = fixture();
        remove_rockspec(dir.path(), CURRENT);
        let target = dir.path().join("target.rockspec");
        fs::write(&target, rockspec("airsstack-lua", "0.2.0")).unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join(DIR).join(CURRENT)).unwrap();
        assert_eq!(
            problems(dir.path()),
            vec![format!(
                "{DIR}/{CURRENT}:2: declares `package = \"airsstack-lua\"`, which does not match \
                 its file name; luarocks refuses such a rockspec"
            )]
        );
    }

    #[test]
    fn a_rockspecs_directory_that_cannot_be_listed_is_reported() {
        let dir = fixture();
        fs::remove_dir_all(dir.path().join(DIR)).unwrap();
        let found = problems(dir.path());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with(&format!("{DIR}: could not be listed: ")),
            "{found:?}"
        );
    }

    #[test]
    fn a_member_manifest_that_cannot_be_read_is_reported() {
        let dir = fixture();
        fs::remove_file(dir.path().join("crates/airsl-cli/Cargo.toml")).unwrap();
        let found = problems(dir.path());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("crates/airsl-cli/Cargo.toml: could not be read: "),
            "{found:?}"
        );
    }

    #[test]
    fn an_empty_string_version_counts_as_a_version_of_its_own() {
        let dir = fixture();
        write(
            dir.path(),
            "crates/airsl/Cargo.toml",
            "[package]\nname = \"airsl\"\nversion = \"\"\n",
        );
        assert_eq!(
            problems(dir.path()),
            vec![
                "crates/airsl/Cargo.toml: sets its own version, which must come from the workspace \
                 (`version.workspace = true`) so airsl, airsl-cli and airsl-lua share one number"
            ]
        );
    }

    #[test]
    fn every_other_type_of_member_version_is_named_by_its_type() {
        for (value, kind) in [
            ("1", "an integer"),
            ("1.5", "a float"),
            ("true", "a boolean"),
            ("[1]", "an array"),
            ("2024-01-01", "a date-time"),
            (
                "{ workspace = false }",
                "a table other than `{ workspace = true }`",
            ),
            (
                "{ workspace = true, extra = 1 }",
                "a table other than `{ workspace = true }`",
            ),
            (
                "{ workspace = \"yes\" }",
                "a table other than `{ workspace = true }`",
            ),
        ] {
            let dir = fixture();
            write(
                dir.path(),
                "crates/airsl/Cargo.toml",
                &format!("[package]\nname = \"airsl\"\nversion = {value}\n"),
            );
            assert_eq!(
                problems(dir.path()),
                vec![format!(
                    "crates/airsl/Cargo.toml: has a version that is {kind}; it must be \
                     `version.workspace = true` so airsl, airsl-cli and airsl-lua share one number"
                )],
                "{value}"
            );
        }
    }

    #[test]
    fn the_rockspec_suffix_is_matched_case_sensitively() {
        // A name of another version than the current ones, so that a case-insensitive file system
        // does not fold it onto a current rockspec.
        let dir = fixture();
        write(
            dir.path(),
            "crates/airsl-lua/rockspecs/airsstack-0.1.0-1.ROCKSPEC",
            "",
        );
        assert_eq!(
            problems(dir.path()),
            vec![format!(
                "{DIR}/airsstack-0.1.0-1.ROCKSPEC: is not a rockspec; only the two rockspecs live \
                 here"
            )]
        );
    }

    #[test]
    fn rockspecs_match_the_workspace_version() {
        assert_eq!(problems(&workspace::root()), Vec::<String>::new());
    }
}
