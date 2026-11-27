// Probe 245 control: what `fs::read_to_string` alone does with a directory named like a rockspec (the reading the
// round-1 text relied on), so the explicit metadata check's effect is visible. Usage: control <path>
fn main() {
    let path = std::env::args().nth(1).expect("path");
    println!("{:?}", std::fs::read_to_string(&path).map(|s| s.len()));
}
