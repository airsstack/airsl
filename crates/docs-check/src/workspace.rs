//! The workspace the guards read: where its root is, and which crates and documents it holds.
//!
//! Its own module because every guard starts from the same facts, and a path rule written twice
//! drifts: the examples guard and each citation guard must agree on what the root is, what counts
//! as a crate, and which documents are in scope.
//!
//! Responsibilities:
//!
//! - The repository root, derived from this crate's own manifest directory.
//! - The crate directories under `crates/`, and the crate a given document belongs to.
//! - The documents whose prose is checked: the repository `CLAUDE.md` and `README.md`, and every
//!   `README.md` under `crates/`.
//! - Folding CRLF to LF, so a checkout that converted line endings reads the same as one that did
//!   not.
//!
//! Non-responsibilities: what any document says. Reading citations or output blocks belongs to the
//! guards.
#![expect(
    clippy::redundant_pub_crate,
    reason = "the guards are sibling modules and read these crate-wide; pub(crate) names that intent at each item"
)]

use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

/// The repository root: two directories above this crate's manifest, which sits in `crates/`.
pub(crate) fn root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .unwrap_or(manifest)
        .to_path_buf()
}

/// Folds CRLF to LF, so a checkout that converted line endings compares equal to one that did not.
pub(crate) fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// The crate directories directly under `<root>/crates`, sorted.
pub(crate) fn crate_dirs(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root.join("crates")) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// The crate directory a document sits in, or `None` for a document outside every crate (the
/// repository `CLAUDE.md` and `README.md`).
pub(crate) fn citing_crate(doc: &Path, root: &Path) -> Option<PathBuf> {
    let under_crates = doc.strip_prefix(root.join("crates")).ok()?;
    let name = under_crates.components().next()?;
    Some(root.join("crates").join(name))
}

/// The documents whose citations are checked: the repository `CLAUDE.md` and `README.md`, then
/// every `README.md` under `crates/` (the crate READMEs and each example's), sorted.
pub(crate) fn documents(root: &Path) -> Vec<PathBuf> {
    let mut docs = vec![root.join("CLAUDE.md"), root.join("README.md")];
    let mut readmes: Vec<PathBuf> = WalkDir::new(root.join("crates"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && entry.file_name() == "README.md")
        .map(walkdir::DirEntry::into_path)
        .collect();
    readmes.sort();
    docs.extend(readmes);
    docs
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "fixtures are known to be creatable; a panic is the failure signal"
    )]

    use super::*;

    #[test]
    fn the_root_holds_the_workspace_manifest_and_claude_md() {
        let root = root();
        assert!(root.join("Cargo.toml").is_file(), "{}", root.display());
        assert!(root.join("CLAUDE.md").is_file(), "{}", root.display());
        assert!(root.join("crates").join("docs-check").is_dir());
    }

    #[test]
    fn crlf_folds_to_lf_and_lone_cr_is_left_alone() {
        assert_eq!(normalize("a\r\nb\rc\n"), "a\nb\rc\n");
    }

    #[test]
    fn a_document_belongs_to_the_crate_directory_it_sits_under() {
        let root = Path::new("/ws");
        let nested = root.join("crates/a/examples/demo/README.md");
        assert_eq!(
            citing_crate(&nested, root),
            Some(root.join("crates").join("a"))
        );
        assert_eq!(citing_crate(&root.join("CLAUDE.md"), root), None);
        assert_eq!(citing_crate(&root.join("book/src/x.md"), root), None);
    }

    #[test]
    fn documents_are_the_root_pair_then_every_readme_under_crates() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("crates/b/examples/z")).unwrap();
        fs::create_dir_all(root.join("crates/a")).unwrap();
        for rel in [
            "crates/b/examples/z/README.md",
            "crates/a/README.md",
            "crates/a/NOTES.md",
        ] {
            fs::write(root.join(rel), "").unwrap();
        }
        let shown: Vec<_> = documents(root)
            .iter()
            .map(|doc| doc.strip_prefix(root).unwrap().to_path_buf())
            .collect();
        assert_eq!(
            shown,
            [
                "CLAUDE.md",
                "README.md",
                "crates/a/README.md",
                "crates/b/examples/z/README.md"
            ]
            .map(PathBuf::from)
        );
        assert_eq!(
            crate_dirs(root),
            vec![root.join("crates/a"), root.join("crates/b")]
        );
    }
}
