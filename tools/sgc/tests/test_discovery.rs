use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn sgc() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sgc"))
}

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("sgc_test_discovery_{name}_{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn native_toolchain_available() -> bool {
    which::which("clang").is_ok() || which::which("clang.exe").is_ok()
}

#[test]
fn sgc_test_discovers_and_runs_zero_arg_test_functions() {
    if !native_toolchain_available() {
        eprintln!("skip: native clang toolchain unavailable for test discovery e2e");
        return;
    }

    let root = temp_dir("functions");
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("tests/math.sg"),
        r#"
def helper(value: i64) -> i64 {
    value + 1
}

def test_first() -> i64 {
    if helper(40) == 41 { 0 } else { 1 }
}

def test_second() -> i64 {
    if helper(1) == 2 { 0 } else { 1 }
}
"#,
    )
    .unwrap();

    let output = Command::new(sgc())
        .current_dir(&root)
        .args(["test", "--format", "json"])
        .output()
        .expect("run sgc test");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("test output should be valid JSON");
    assert_eq!(report["passed"], 2);
    assert_eq!(report["failed"], 0);
    assert_eq!(report["total"], 2);
    assert_eq!(report["tests"][0]["name"], "tests/math.sg::test_first");
    assert_eq!(report["tests"][0]["function"], "test_first");
    assert_eq!(report["tests"][1]["name"], "tests/math.sg::test_second");
    assert_eq!(report["tests"][1]["function"], "test_second");

    let generated = fs::read_dir(root.join("tests"))
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".sengoo-test-")
        });
    assert!(!generated, "generated test wrappers should be cleaned up");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn sgc_test_function_assertion_reports_original_source_path() {
    if !native_toolchain_available() {
        eprintln!("skip: native clang toolchain unavailable for test discovery e2e");
        return;
    }

    let root = temp_dir("assertion_source");
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("tests/assertions.sg"),
        r#"import std::assert;

def test_failure() -> i64 {
    if assert_eq_i64(7, 9) { 0 } else { 1 }
}
"#,
    )
    .unwrap();

    let output = Command::new(sgc())
        .current_dir(&root)
        .args([
            "test",
            "--exact",
            "tests/assertions.sg::test_failure",
            "--format",
            "json",
        ])
        .output()
        .expect("run sgc test");

    assert!(
        !output.status.success(),
        "failing function test should fail"
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("test output should be valid JSON");
    let assertion = &report["tests"][0]["assertion"];
    assert!(
        assertion["file"]
            .as_str()
            .unwrap_or_default()
            .replace('\\', "/")
            .ends_with("/tests/assertions.sg"),
        "function assertion should report the original source: {assertion}"
    );
    assert_eq!(assertion["line"], 4);
    assert_eq!(report["tests"][0]["function"], "test_failure");

    let _ = fs::remove_dir_all(root);
}
