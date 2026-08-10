//! The parameterised authorities a policy extends to individual host modules.
//!
//! Three types rather than one because the three questions have different shapes: filesystem
//! authority is a set of directory roots split by direction, environment authority is a set of
//! variable names, and process authority is a set of executable names. Collapsing them into a
//! generic "capability with strings" would make every module re-derive what its own strings mean.
//!
//! Responsibilities:
//!
//! - [`FsGrant`], [`EnvGrant`] and [`ProcGrant`], each an allowlist with the containment rule that
//!   fits it.
//! - The containment checks themselves, which are the only place the rules are written down.
//!
//! Non-responsibilities: resolving a path against the filesystem. A grant is asked about a path
//! that has already been made absolute and canonical as far as it exists; deciding what that means
//! is the `fs` module's job, because only it knows whether the target is being read or created.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Which directories a script may read and write.
///
/// Read and write are separate lists rather than one list with a mode, because the overwhelmingly
/// common shape is a wide read root and a narrow write root — a tool that scans a repository and
/// writes one index file. One list would force the write authority up to the read authority.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FsGrant {
    read: Vec<PathBuf>,
    write: Vec<PathBuf>,
}

impl FsGrant {
    /// Grants nothing.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            read: Vec::new(),
            write: Vec::new(),
        }
    }

    /// Adds a directory the script may read under.
    #[must_use]
    pub fn read(mut self, root: impl Into<PathBuf>) -> Self {
        self.read.push(resolve_root(root.into()));
        self
    }

    /// Adds a directory the script may write under.
    ///
    /// A write root is not implicitly readable. Saying so explicitly is a little more typing and
    /// removes the question of what a write-only grant means when something reads the file back.
    #[must_use]
    pub fn write(mut self, root: impl Into<PathBuf>) -> Self {
        self.write.push(resolve_root(root.into()));
        self
    }

    /// Whether `path` lies under a granted read root.
    #[must_use]
    pub fn allows_read(&self, path: &Path) -> bool {
        contains_any(&self.read, path)
    }

    /// Whether `path` lies under a granted write root.
    #[must_use]
    pub fn allows_write(&self, path: &Path) -> bool {
        contains_any(&self.write, path)
    }

    /// The granted read roots, in the order they were added.
    #[must_use]
    pub fn read_roots(&self) -> &[PathBuf] {
        &self.read
    }

    /// The granted write roots, in the order they were added.
    #[must_use]
    pub fn write_roots(&self) -> &[PathBuf] {
        &self.write
    }

    /// Whether this grant permits nothing at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.read.is_empty() && self.write.is_empty()
    }
}

/// Puts a grant root into the form the paths checked against it will be in.
///
/// A path is canonicalised before it is checked, so a root that is not itself canonical can never
/// contain one. Two ways to write a root that silently grants nothing, both easy to hit:
///
/// - **A relative root.** `--allow-read .` stores `.`, and no absolute path begins with it.
/// - **A root reached through a symlink.** Granting `/srv/app` when it links to `/mnt/app` fails
///   every check, because the target resolves past the link to `/mnt/app/...`.
///
/// Neither produces an error at the point the grant is written — the policy simply refuses
/// everything afterwards, with a message naming a root that looks correct. Resolving here means the
/// two sides are always comparable.
///
/// A root that does not exist yet is made absolute but not canonical, since there is nothing to
/// resolve; that is the ordinary case for a write root the script is about to create.
fn resolve_root(root: PathBuf) -> PathBuf {
    root.canonicalize()
        .or_else(|_| std::path::absolute(&root))
        .unwrap_or(root)
}

/// Whether `path` is inside any of `roots`.
///
/// Compared component-wise via [`Path::starts_with`] rather than as strings, so `/repo-extra` is
/// not inside `/repo`. A string prefix test accepts that, and it is the classic way a containment
/// check turns out to have never contained anything.
fn contains_any(roots: &[PathBuf], path: &Path) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

/// Which environment variables a script may read.
///
/// An allowlist of names rather than a boolean, because the environment routinely carries
/// credentials that have nothing to do with the script holding the grant. "May read the
/// environment" is almost never the authority anyone means.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnvGrant {
    names: BTreeSet<String>,
}

impl EnvGrant {
    /// Grants nothing.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            names: BTreeSet::new(),
        }
    }

    /// Adds the variable names the script may read.
    #[must_use]
    pub fn read<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.names.extend(names.into_iter().map(Into::into));
        self
    }

    /// Whether `name` is on the allowlist.
    #[must_use]
    pub fn allows(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    /// The allowed names, in sorted order.
    ///
    /// Sorted because this is what `airsl doctor` prints and what a script sees from `env.all`,
    /// and a set that enumerated differently between runs would make both non-deterministic.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    /// Whether this grant permits nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// Which executables a script may run.
///
/// Matched on the program name as written, not on a resolved path. That is a real limitation, and
/// the crate's sandbox documentation records it: an allowlist of names is defeated by anything that
/// can put its own `git` earlier on `PATH`, so this grant is only as strong as the write grants
/// sitting alongside it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProcGrant {
    executables: BTreeSet<String>,
}

impl ProcGrant {
    /// Grants nothing.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            executables: BTreeSet::new(),
        }
    }

    /// Adds the executables the script may run.
    #[must_use]
    pub fn allow<I, S>(mut self, executables: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.executables
            .extend(executables.into_iter().map(Into::into));
        self
    }

    /// Whether `program` may be run.
    #[must_use]
    pub fn allows(&self, program: &str) -> bool {
        self.executables.contains(program)
    }

    /// The allowed executables, in sorted order.
    pub fn executables(&self) -> impl Iterator<Item = &str> {
        self.executables.iter().map(String::as_str)
    }

    /// Whether this grant permits nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.executables.is_empty()
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::{EnvGrant, FsGrant, ProcGrant};
    use std::path::Path;

    #[test]
    fn an_empty_fs_grant_allows_nothing() {
        let grant = FsGrant::none();
        assert!(!grant.allows_read(Path::new("/anything")));
        assert!(!grant.allows_write(Path::new("/anything")));
        assert!(grant.is_empty());
    }

    #[test]
    fn a_read_root_covers_itself_and_its_descendants() {
        let grant = FsGrant::none().read("/repo");
        assert!(grant.allows_read(Path::new("/repo")));
        assert!(grant.allows_read(Path::new("/repo/crates/airsl")));
    }

    #[test]
    fn a_read_root_does_not_cover_its_parent_or_a_sibling() {
        let grant = FsGrant::none().read("/repo");
        assert!(!grant.allows_read(Path::new("/")));
        assert!(!grant.allows_read(Path::new("/elsewhere")));
    }

    #[test]
    fn a_sibling_sharing_a_name_prefix_is_not_inside_the_root() {
        // The string test `"/repo-extra".starts_with("/repo")` is true; the component test is not.
        let grant = FsGrant::none().read("/repo");
        assert!(!grant.allows_read(Path::new("/repo-extra")));
        assert!(!grant.allows_read(Path::new("/repo-extra/src")));
    }

    #[test]
    fn read_and_write_are_independent_authorities() {
        let grant = FsGrant::none().read("/repo").write("/repo/.index");
        assert!(grant.allows_read(Path::new("/repo/src")));
        assert!(!grant.allows_write(Path::new("/repo/src")));
        assert!(grant.allows_write(Path::new("/repo/.index/a.json")));
        // A write root is not implicitly readable, but here it happens to sit under a read root.
        assert!(grant.allows_read(Path::new("/repo/.index/a.json")));
    }

    #[test]
    fn a_write_root_outside_every_read_root_is_not_readable() {
        let grant = FsGrant::none().read("/repo").write("/var/state");
        assert!(grant.allows_write(Path::new("/var/state/a")));
        assert!(!grant.allows_read(Path::new("/var/state/a")));
    }

    #[test]
    fn a_relative_root_resolves_against_the_working_directory() {
        // `--allow-read .` is the obvious thing to type. Stored verbatim it grants nothing at all,
        // because every path it is checked against is absolute.
        let here = std::env::current_dir().unwrap().canonicalize().unwrap();
        let grant = FsGrant::none().read(".");
        assert_eq!(grant.read_roots(), std::slice::from_ref(&here));
        assert!(grant.allows_read(&here.join("Cargo.toml")));
    }

    #[test]
    fn a_symlinked_root_resolves_to_what_it_points_at() {
        // Otherwise granting the link grants nothing: a path under it canonicalises past the link,
        // and the refusal names a root that looks exactly right.
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        std::fs::create_dir(base.join("real")).unwrap();
        std::os::unix::fs::symlink(base.join("real"), base.join("link")).unwrap();

        let grant = FsGrant::none().read(base.join("link"));
        assert_eq!(grant.read_roots(), [base.join("real")]);
        assert!(grant.allows_read(&base.join("real/a.txt")));
    }

    #[test]
    fn a_root_that_does_not_exist_yet_is_made_absolute_but_kept() {
        // The ordinary case for a write root: the script is about to create it.
        let grant = FsGrant::none().write("/definitely/not/here");
        assert_eq!(
            grant.write_roots(),
            [std::path::PathBuf::from("/definitely/not/here")]
        );
    }

    #[test]
    fn several_roots_are_all_honoured() {
        let grant = FsGrant::none().read("/a").read("/b");
        assert!(grant.allows_read(Path::new("/a/x")));
        assert!(grant.allows_read(Path::new("/b/x")));
        assert!(!grant.allows_read(Path::new("/c/x")));
    }

    #[test]
    fn an_env_grant_admits_only_the_names_it_lists() {
        let grant = EnvGrant::none().read(["HOME", "AIRSSTACK_HOME"]);
        assert!(grant.allows("HOME"));
        assert!(grant.allows("AIRSSTACK_HOME"));
        assert!(!grant.allows("AWS_SECRET_ACCESS_KEY"));
    }

    #[test]
    fn env_names_are_matched_exactly_rather_than_by_prefix() {
        let grant = EnvGrant::none().read(["HOME"]);
        assert!(!grant.allows("HOMEBREW_PREFIX"));
        assert!(!grant.allows("home"));
    }

    #[test]
    fn env_names_enumerate_in_sorted_order() {
        let grant = EnvGrant::none().read(["ZED", "ALPHA", "MID"]);
        let names: Vec<_> = grant.names().collect();
        assert_eq!(names, ["ALPHA", "MID", "ZED"]);
    }

    #[test]
    fn a_proc_grant_admits_only_the_executables_it_lists() {
        let grant = ProcGrant::none().allow(["git", "tar"]);
        assert!(grant.allows("git"));
        assert!(grant.allows("tar"));
        assert!(!grant.allows("curl"));
    }

    #[test]
    fn a_proc_grant_does_not_admit_a_path_to_a_granted_name() {
        // The check is on the program as written. Anything that resolves a path has to do so
        // before asking, or the grant means nothing.
        let grant = ProcGrant::none().allow(["git"]);
        assert!(!grant.allows("/usr/bin/git"));
        assert!(!grant.allows("./git"));
    }

    #[test]
    fn every_grant_reports_emptiness() {
        assert!(EnvGrant::none().is_empty());
        assert!(ProcGrant::none().is_empty());
        assert!(!EnvGrant::none().read(["A"]).is_empty());
        assert!(!ProcGrant::none().allow(["a"]).is_empty());
    }
}
