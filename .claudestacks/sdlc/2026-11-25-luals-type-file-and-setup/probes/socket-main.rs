use std::os::unix::fs::FileTypeExt;
fn main() {
    let dir = std::env::temp_dir().join(format!("sockprobe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(".luarc.json");
    let _l = std::os::unix::net::UnixListener::bind(&p).unwrap();
    let m = std::fs::symlink_metadata(&p).unwrap();
    println!("socket: is_file: {}, is_dir: {}, is_socket: {}", m.file_type().is_file(), m.file_type().is_dir(), m.file_type().is_socket());
    println!("canonicalize ok: {}", std::fs::canonicalize(&p).is_ok());
    match std::fs::File::open(&p) { Ok(_) => println!("File::open: ok"), Err(e) => println!("File::open: err kind={:?} msg={}", e.kind(), e) }
    let link = dir.join("link.json");
    std::os::unix::fs::symlink(&p, &link).unwrap();
    let lm = std::fs::symlink_metadata(&link).unwrap();
    let c = std::fs::canonicalize(&link).unwrap();
    let cm = std::fs::metadata(&c).unwrap();
    println!("link to socket: is_symlink: {}, canonical is_file: {}, is_socket: {}", lm.file_type().is_symlink(), cm.file_type().is_file(), cm.file_type().is_socket());
    std::fs::remove_dir_all(&dir).unwrap();
}
