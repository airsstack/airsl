// Throwaway probe: Path::is_absolute for the inputs spec §5.4 names, on this host.
fn main() {
    for p in ["", "x", "/a", r"\a", "C:a", r"C:\a"] {
        println!("{p:?} is_absolute={}", std::path::Path::new(p).is_absolute());
    }
}
