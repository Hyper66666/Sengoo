mod common;

use common::source_sgc_command;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_source(name: &str, source: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sengoo-cranelift-{name}-{}-{nonce}.sg",
        std::process::id()
    ));
    fs::write(&path, source).expect("temporary Sengoo source should be writable");
    path
}

fn run_cranelift(name: &str, source: &str, opt_level: &str) -> std::process::Output {
    let source = temp_source(name, source);
    let output = source_sgc_command()
        .args([
            "run",
            source.to_str().expect("temporary path should be UTF-8"),
            "--cranelift-fast-jit",
            opt_level,
        ])
        .output()
        .expect("sgc should launch");
    let _ = fs::remove_file(source);
    output
}

#[test]
fn cranelift_cli_executes_primitive_numeric_program() {
    let output = run_cranelift(
        "primitive",
        r#"
def main() -> i64 {
    let left: i16 = 20i16;
    let right: i16 = 2i16;
    (left * right + right) as i64
}
"#,
        "-O2",
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
}

#[test]
fn cranelift_cli_traps_debug_overflow_and_wraps_release_overflow() {
    let source = r#"
def main() -> i64 {
    let max: i32 = 2147483647i32;
    let one: i32 = 1i32;
    (max + one) as i64
}
"#;

    let debug = run_cranelift("debug-overflow", source, "-O0");
    assert!(
        !debug.status.success(),
        "debug Cranelift overflow should trap; stdout: {}; stderr: {}",
        String::from_utf8_lossy(&debug.stdout),
        String::from_utf8_lossy(&debug.stderr)
    );

    let release = run_cranelift("release-overflow", source, "-O2");
    assert!(
        release.status.success(),
        "release Cranelift overflow should wrap; stderr: {}",
        String::from_utf8_lossy(&release.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&release.stdout).trim(),
        "-2147483648"
    );
}

#[test]
fn cranelift_cli_traps_integer_division_by_zero() {
    let output = run_cranelift(
        "division-by-zero",
        r#"
def main() -> i64 {
    let value: i64 = 42;
    let zero: i64 = 0;
    value / zero
}
"#,
        "-O0",
    );

    assert!(
        !output.status.success(),
        "Cranelift division by zero should trap; stdout: {}; stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
