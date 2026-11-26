//! Y4 probe: how std::process::Command resolves a luarocks launcher on Windows.
use std::process::Command;

fn main() {
    for program in std::env::args().skip(1) {
        match Command::new(&program).arg("--version").output() {
            Ok(out) => println!(
                "Command::new({program:?}) -> Ok status={:?} stdout_first_line={:?}",
                out.status.code(),
                String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or("")
            ),
            Err(e) => println!("Command::new({program:?}) -> Err kind={:?} msg={e}", e.kind()),
        }
    }
}
