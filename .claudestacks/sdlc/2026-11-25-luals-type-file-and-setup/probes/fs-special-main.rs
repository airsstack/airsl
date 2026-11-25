use std::fs;
use std::os::unix::fs::symlink;
fn main() {
    let w = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("w");
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(w.join("dotfiles")).unwrap();
    fs::create_dir_all(w.join("proj")).unwrap();
    fs::write(w.join("dotfiles/luarc.json"), "{}").unwrap();
    symlink("../dotfiles/luarc.json", w.join("proj/.luarc.json")).unwrap();
    symlink("../dotfiles/missing.json", w.join("proj/dangling.json")).unwrap();
    let link = w.join("proj/.luarc.json");
    println!("link exists(): {}", link.exists());
    println!("link symlink_metadata is_symlink: {}", fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    println!("link canonicalize ends with: {:?}", fs::canonicalize(&link).unwrap().strip_prefix(fs::canonicalize(&w).unwrap()).unwrap());
    println!("link metadata is_file: {}", fs::metadata(&link).unwrap().is_file());
    let dang = w.join("proj/dangling.json");
    println!("dangling exists(): {}", dang.exists());
    println!("dangling symlink_metadata ok: {}", fs::symlink_metadata(&dang).is_ok());
    println!("dangling canonicalize: {:?}", fs::canonicalize(&dang).err().map(|e| (e.kind(), e.to_string())));
    // write through: temp next to target then rename over target
    let target = fs::canonicalize(&link).unwrap();
    let tmp = target.with_file_name(".luarc.json.tmp");
    fs::write(&tmp, "{\"x\":1}").unwrap();
    fs::rename(&tmp, &target).unwrap();
    println!("after write-through: link still symlink: {}, read via link: {}", fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), fs::read_to_string(&link).unwrap());
    // fifo
    let fifo = w.join("proj/fifo");
    std::process::Command::new("mkfifo").arg(&fifo).status().unwrap();
    let md = fs::metadata(&fifo).unwrap();
    println!("fifo is_file: {}, is_dir: {}", md.is_file(), md.is_dir());
}
