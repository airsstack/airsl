//! A7 probe: what std::process::Command reports for a missing binary, a non-executable file and a
//! signal-killed child.

use std::process::Command;

fn report(label: &str, result: std::io::Result<std::process::Output>) {
    match result {
        Ok(out) => println!(
            "{label}: Ok; status.code()={:?}; status Display=\"{}\"; status.success()={}",
            out.status.code(),
            out.status,
            out.status.success()
        ),
        Err(e) => println!(
            "{label}: Err; kind={:?}; Display=\"{e}\"; raw_os_error={:?}",
            e.kind(),
            e.raw_os_error()
        ),
    }
}

fn main() -> std::io::Result<()> {
    report("missing", Command::new("airsl-probe-no-such-binary").output());

    let dir = std::env::temp_dir().join("airsl-a7-probe");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("not-executable");
    std::fs::write(&file, "#!/bin/sh\necho hi\n")?;
    report("non-executable (absolute path, mode 0644)", Command::new(&file).output());

    report("killed", Command::new("sh").args(["-c", "kill -9 $$"]).output());
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}
