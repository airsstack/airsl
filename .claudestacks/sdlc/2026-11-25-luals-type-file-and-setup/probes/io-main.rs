use std::fs;
fn main() {
    let w = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("w");
    fs::write(w.join("afile"), "x").unwrap();
    println!("read_dir missing: {:?}", fs::read_dir(w.join("missing")).err().map(|e| (e.kind(), e.to_string())));
    println!("read_dir file: {:?}", fs::read_dir(w.join("afile")).err().map(|e| (e.kind(), e.to_string())));
    fs::create_dir_all(w.join("p1")).unwrap(); fs::write(w.join("p1/.airsl"), "x").unwrap();
    println!("create_dir_all under file: {:?}", fs::create_dir_all(w.join("p1/.airsl/types")).err().map(|e| (e.kind(), e.to_string())));
    fs::create_dir_all(w.join("p2/.airsl/types/airsstack.lua")).unwrap();
    println!("write onto dir: {:?}", fs::write(w.join("p2/.airsl/types/airsstack.lua"), "x").err().map(|e| (e.kind(), e.to_string())));
    println!("read dir as file: {:?}", fs::read_to_string(w.join("p2/.airsl/types")).err().map(|e| (e.kind(), e.to_string())));
    println!("rename file over dir: {:?}", { fs::write(w.join("p2/tmp"), "x").unwrap(); fs::rename(w.join("p2/tmp"), w.join("p2/.airsl/types/airsstack.lua")).err().map(|e| (e.kind(), e.to_string())) });
}
