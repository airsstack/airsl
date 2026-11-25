// File modes when writing through tempfile 3.27.0 and renaming over a target.
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
fn mode(p: &std::path::Path) -> String { format!("{:o}", std::fs::metadata(p).unwrap().permissions().mode() & 0o777) }
fn main() {
    let d = tempfile::tempdir().unwrap();
    let t = tempfile::Builder::new().tempfile_in(d.path()).unwrap();
    println!("tempfile_in default mode: {}", mode(t.path()));
    // created: set 0644 explicitly, then persist
    let mut t = tempfile::Builder::new().tempfile_in(d.path()).unwrap();
    t.write_all(b"new").unwrap();
    t.as_file().set_permissions(std::fs::Permissions::from_mode(0o644)).unwrap();
    let new = d.path().join("new.json");
    t.persist(&new).unwrap();
    println!("created with explicit 0644: {}", mode(&new));
    // updated: copy the old file's permissions (0640), then persist over it
    let old = d.path().join("old.json");
    std::fs::write(&old, "old").unwrap();
    std::fs::set_permissions(&old, std::fs::Permissions::from_mode(0o640)).unwrap();
    let mut t = tempfile::Builder::new().tempfile_in(d.path()).unwrap();
    t.write_all(b"updated").unwrap();
    t.as_file().set_permissions(std::fs::metadata(&old).unwrap().permissions()).unwrap();
    t.persist(&old).unwrap();
    println!("updated keeps old 0640: {}, content: {}", mode(&old), std::fs::read_to_string(&old).unwrap());
    // read-only target
    let ro = d.path().join("ro.json");
    std::fs::write(&ro, "ro").unwrap();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o444)).unwrap();
    println!("0444 permissions().readonly(): {}", std::fs::metadata(&ro).unwrap().permissions().readonly());
    let t = tempfile::Builder::new().tempfile_in(d.path()).unwrap();
    println!("persist over 0444 file succeeds: {}", t.persist(&ro).is_ok());
}
