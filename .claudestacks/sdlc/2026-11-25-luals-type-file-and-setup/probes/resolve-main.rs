// What symlink_metadata returns for a target whose parent path is a file, or is unreadable.
use std::os::unix::fs::PermissionsExt;
fn main() {
    let w = std::env::temp_dir().join(format!("resolveprobe-{}", std::process::id()));
    std::fs::create_dir_all(&w).unwrap();
    // .airsl is a regular file
    std::fs::write(w.join(".airsl"), "x").unwrap();
    let r = std::fs::symlink_metadata(w.join(".airsl/types/airsstack.lua"));
    println!("parent is a file: {:?}", r.err().map(|e| (e.kind(), e.to_string())));
    // .airsl/types exists but has mode 000
    let q = w.join("q");
    std::fs::create_dir_all(q.join(".airsl/types")).unwrap();
    std::fs::set_permissions(q.join(".airsl/types"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let r = std::fs::symlink_metadata(q.join(".airsl/types/airsstack.lua"));
    println!("parent dir mode 000: {:?}", r.err().map(|e| (e.kind(), e.to_string())));
    std::fs::set_permissions(q.join(".airsl/types"), std::fs::Permissions::from_mode(0o755)).unwrap();
    // missing parent
    let r = std::fs::symlink_metadata(w.join("nope/.airsl/types/airsstack.lua"));
    println!("parent missing: {:?}", r.err().map(|e| (e.kind(), e.to_string())));
    // a symlink loop
    std::os::unix::fs::symlink(w.join("b.json"), w.join("a.json")).unwrap();
    std::os::unix::fs::symlink(w.join("a.json"), w.join("b.json")).unwrap();
    println!("loop symlink_metadata ok: {}", std::fs::symlink_metadata(w.join("a.json")).is_ok());
    println!("loop canonicalize: {:?}", std::fs::canonicalize(w.join("a.json")).err().map(|e| (e.kind(), e.to_string())));
    std::fs::remove_dir_all(&w).unwrap();
}
