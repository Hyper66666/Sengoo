//! External watchdog makes a nonwaiting stdin regression fail within 25s.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
fn clang() -> PathBuf {
    if let Some(value) = std::env::var_os("SENGOO_CLANG") {
        return value.into();
    }
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let p = PathBuf::from(local)
                .join("SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe");
            if p.is_file() {
                return p;
            }
        }
    }
    "clang".into()
}
#[test]
fn piped_stdin_remains_responsive_under_backpressure_and_cancel() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let work = std::env::temp_dir().join(format!(
        "sengoo-stdin-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&work).unwrap();
    let binary = work.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let output = Command::new(clang())
        .args(["-std=c11", "-O1", "-Wno-unused-function"])
        .arg(root.join("tools/stdlib/tests/process_stdin_probe.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("clang required");
    assert!(
        output.status.success(),
        "compile stdin probe: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut child = Command::new(&binary)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(25);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!("stdin probe exceeded 25s watchdog; owned job handles close on process exit\n{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stdin probe: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
