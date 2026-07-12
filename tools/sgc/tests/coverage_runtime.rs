use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn sgc() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sgc"))
}

fn temp_project() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after the Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "sgc_coverage_runtime_{}_{}",
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(root.join("tests")).expect("create coverage test project");
    root
}

fn native_toolchain_available() -> bool {
    which::which("clang").is_ok() || which::which("clang.exe").is_ok()
}

#[test]
fn coverage_counts_runtime_hits_instead_of_marking_the_source_fully_covered() {
    if !native_toolchain_available() {
        eprintln!("SKIP coverage_runtime: native clang toolchain unavailable");
        return;
    }

    let root = temp_project();
    let source = root.join("tests/branches.sg");
    fs::write(
        &source,
        r#"#[test]
def visits_only_the_live_path() -> i64 {
    let take_branch = false;
    if take_branch {
        let never_reached = 41;
        return never_reached;
    }
    0
}

def never_called() -> i64 {
    let hidden = 41;
    hidden + 1
}
"#,
    )
    .expect("write coverage fixture");

    let output = Command::new(sgc())
        .current_dir(&root)
        .args(["test", "--coverage", "--format", "json"])
        .output()
        .expect("run sgc test with coverage");

    assert!(
        output.status.success(),
        "coverage fixture should pass:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let report: Value = serde_json::from_str(stdout.trim()).expect("valid JSON test report");
    let coverage = &report["coverage"];
    let covered = coverage["covered_lines"]
        .as_u64()
        .expect("covered_lines should be numeric");
    let executable = coverage["executable_lines"]
        .as_u64()
        .expect("executable_lines should be numeric");
    let percent = coverage["percent"]
        .as_u64()
        .expect("percent should be numeric");

    assert!(
        covered < executable,
        "an unvisited branch and uncalled function must leave uncovered lines: {coverage}"
    );
    assert!(
        covered > 0,
        "the live test path must produce runtime hits: {coverage}"
    );
    assert!(
        percent < 100,
        "runtime coverage must not report 100%: {coverage}"
    );

    let _ = fs::remove_dir_all(root);
}
