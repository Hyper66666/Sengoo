use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
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
    let dir = std::env::temp_dir().join(format!("sgc_assert_transport_{name}_{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_failing_assert_test(root: &Path) -> PathBuf {
    fs::create_dir_all(root.join("tests")).unwrap();
    let test = root.join("tests/assert_fail.sg");
    fs::write(
        &test,
        r#"import std::assert;

def main() -> i64 {
    if assert_eq_i64(7, 9) { 0 } else { 1 }
}
"#,
    )
    .unwrap();
    test
}

fn native_toolchain_available() -> bool {
    which::which("clang").is_ok() || which::which("clang.exe").is_ok()
}

#[test]
fn sgc_test_reports_structured_assertion_failure_in_json_mode() {
    if !native_toolchain_available() {
        eprintln!("skip: native clang toolchain unavailable for assertion transport e2e");
        return;
    }

    let root = temp_dir("json_assertion");
    write_failing_assert_test(&root);

    let output = Command::new(sgc())
        .current_dir(&root)
        .args(["test", "--format", "json"])
        .output()
        .expect("run sgc test");

    assert!(!output.status.success(), "failing assert test should fail");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let report: Value = serde_json::from_str(stdout.trim()).expect("valid json report");

    assert_eq!(report["failed"], 1);
    let assertion = &report["tests"][0]["assertion"];
    assert_eq!(assertion["schema_version"], 1);
    assert_eq!(assertion["kind"], "assertion_failure");
    assert_eq!(assertion["helper"], "assert_eq_i64");
    assert!(assertion["message"]
        .as_str()
        .unwrap_or_default()
        .contains("expected 7, got 9"));
    assert_eq!(assertion["expected"], "7");
    assert_eq!(assertion["actual"], "9");
    assert!(
        assertion["file"]
            .as_str()
            .unwrap_or_default()
            .replace('\\', "/")
            .ends_with("/tests/assert_fail.sg"),
        "assertion file should point at the failing source: {assertion}"
    );
    assert_eq!(assertion["line"], 4);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn sgc_test_reports_assertion_message_in_text_mode_with_nocapture() {
    if !native_toolchain_available() {
        eprintln!("skip: native clang toolchain unavailable for assertion transport e2e");
        return;
    }

    let root = temp_dir("text_assertion");
    write_failing_assert_test(&root);

    let output = Command::new(sgc())
        .current_dir(&root)
        .args(["test", "--nocapture"])
        .output()
        .expect("run sgc test nocapture");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("assertion: expected 7, got 9"));

    let _ = fs::remove_dir_all(root);
}
