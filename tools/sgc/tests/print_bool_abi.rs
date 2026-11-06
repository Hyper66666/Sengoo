//! Network diagnostics exposed bool i1 passed to the ABI-1 i64 print bridge.
mod common;
use std::{fs, process::Command, time::{SystemTime, UNIX_EPOCH}};

#[test]
fn runtime_boolean_print_uses_zero_extended_i64_for_both_sinks() {
    let scratch = std::env::temp_dir().join(format!("sengoo-print-bool-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&scratch).unwrap();
    let source = scratch.join("main.sg");
    fs::write(&source, r#"
struct Flags { ready: bool, eof: bool, count: i64, error: i64 }
def flags(code: i64) -> Flags { Flags { ready: code != -100, eof: code == 0, count: 0, error: if code < 0 { 0-code } else { 0 } } }
def main() -> i64 {
    let failed = flags(-11);
    println(failed.error); println(failed.ready); println(failed.eof);
    eprintln(failed.ready); eprintln(failed.eof);
    println(flags(0).eof); println(flags(-100).ready);
    if failed.eof { return 1; }; 0
}
"#).unwrap();
    for level in ["0", "2"] {
        let binary = scratch.join(if cfg!(windows) { format!("main-{level}.exe") } else { format!("main-{level}") });
        let build = common::source_sgc_command().args(["build"]).arg(&source).args(["--force-rebuild", "-O", level, "-o"]).arg(&binary).output().unwrap();
        assert!(build.status.success(), "{}\n{}", String::from_utf8_lossy(&build.stdout), String::from_utf8_lossy(&build.stderr));
        let run = Command::new(&binary).output().unwrap();
        assert!(run.status.success(), "{run:?}");
        assert_eq!(String::from_utf8_lossy(&run.stdout).replace("\r\n", "\n"), "11\ntrue\nfalse\ntrue\nfalse\n");
        assert_eq!(String::from_utf8_lossy(&run.stderr).replace("\r\n", "\n"), "true\nfalse\n");
    }
}
