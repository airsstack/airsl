// Probe 251 (Windows): fixtures.sh's directories A..D built with std's Windows calls, because Git Bash's `ln -s` copies by
// default and Windows has no FIFO. The same layout as fixtures.sh, except that A has no entry under the LuaJIT name (the
// FIFO there). Symlinks are made with std::os::windows::fs::symlink_file / symlink_dir (a dangling one included: Windows
// does not need the target to exist). Usage: fixtures_windows <F>
#[cfg(windows)]
fn main() {
    use std::fs;
    use std::os::windows::fs::{symlink_dir, symlink_file};
    let f = std::path::PathBuf::from(std::env::args().nth(1).expect("fixture dir"));
    let _ = fs::remove_dir_all(&f);
    for d in ["A", "B", "C", "D", "targetdir"] {
        fs::create_dir_all(f.join(d)).expect("mkdir");
    }
    fs::write(f.join("real.rockspec"), "package = \"airsstack\"\n").expect("real");
    for name in [".DS_Store", ".rockspec", ".airsstack-0.2.0-1.rockspec", "notes.txt"] {
        fs::write(f.join("A").join(name), "").expect("touch");
    }
    for name in [".hidden-dir", "airsstack-0.2.0-1.rockspec", "notes"] {
        fs::create_dir(f.join("A").join(name)).expect("mkdir A");
    }
    symlink_file("..\\real.rockspec", f.join("B").join("airsstack-0.2.0-1.rockspec")).expect("symlink B file");
    symlink_dir("..\\targetdir", f.join("B").join("airsstack-luajit-0.2.0-1.rockspec")).expect("symlink B dir");
    symlink_file("..\\no-such.rockspec", f.join("C").join("airsstack-0.2.0-1.rockspec")).expect("symlink C dangling");
    fs::write(f.join("C").join("airsstack-luajit-0.2.0-1.rockspec"), b"package = \"\xff\"\n").expect("non-utf8");
    for name in [".DS_Store", ".gitkeep"] {
        fs::write(f.join("D").join(name), "").expect("touch D");
    }
    println!("fixtures built under {}", f.display());
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only; fixtures.sh builds the unix fixtures");
    std::process::exit(2);
}
