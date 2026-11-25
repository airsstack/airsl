#![forbid(unsafe_code)]
use std::os::unix::fs::{MetadataExt, fchown};
fn main() {
    let f = std::fs::File::create("owned.txt").unwrap();
    let m = f.metadata().unwrap();
    println!("running uid={} gid={}", m.uid(), m.gid());
    println!("fchown(file, Some(0), None) as uid {} -> {:?}", m.uid(), fchown(&f, Some(0), None).map_err(|e| e.to_string()));
}
