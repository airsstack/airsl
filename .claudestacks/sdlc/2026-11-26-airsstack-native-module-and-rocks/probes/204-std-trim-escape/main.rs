//! What Rust std (1.94) does to `luarocks config lua_version` stdout bytes before the spec's rule
//! classifies them: `<[u8]>::trim_ascii`, `String::from_utf8` (strict) vs `from_utf8_lossy`, and
//! `str::escape_debug` for showing the value inside a message.
fn main() {
    let inputs: [&[u8]; 16] = [
        b"5.4\n", b"5.4", b" 5.4 \r\n", b"5.1\n", b"5.5\n", b"", b"\n", b"  \t ",
        b"5.4.8\n", b"5.4\n5.1\n", "5.4\u{a0}".as_bytes(), "\u{feff}5.4".as_bytes(),
        b"5.04\n", b"\xff5.4\n", "\u{ff15}.\u{ff14}".as_bytes(), b"10.0\n",
    ];
    for raw in inputs {
        let trimmed = raw.trim_ascii();
        let strict = std::str::from_utf8(trimmed).is_ok();
        let lossy = String::from_utf8_lossy(trimmed);
        let shown: String = lossy.escape_debug().collect();
        let shape = !trimmed.is_empty()
            && strict
            && {
                let s = std::str::from_utf8(trimmed).unwrap_or("");
                match s.split_once('.') {
                    Some((a, b)) => !a.is_empty() && !b.is_empty()
                        && a.bytes().all(|c| c.is_ascii_digit())
                        && b.bytes().all(|c| c.is_ascii_digit()),
                    None => false,
                }
            };
        println!(
            "raw={:?} trimmed_bytes={:?} empty={} utf8={} shown=`{}` digits.digits={}",
            raw.escape_ascii().to_string(),
            trimmed.escape_ascii().to_string(),
            trimmed.is_empty(),
            strict,
            shown,
            shape
        );
    }
}
