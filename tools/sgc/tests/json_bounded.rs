//! Explicit bounds are independent of legacy JSON admission.
mod common;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
fn scratch(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "sengoo-json-bounded-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    root
}
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
#[test]
fn native_explicit_json_bounds_precede_reads_and_release_admission() {
    let work = scratch("native");
    let source = repository().join("tools/stdlib/tests/json_bounded_probe.c");
    let clang = std::env::var_os("SENGOO_CLANG").unwrap_or_else(|| "clang".into());
    let binary = work.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let output = Command::new(clang)
        .args(["-std=c11", "-O1", "-Wall", "-Wextra"])
        .arg(source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    fs::write(work.join("build-stderr.txt"), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "{}: {}",
        work.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(binary).output().unwrap();
    fs::write(work.join("run-stdout.txt"), &output.stdout).unwrap();
    fs::write(work.join("run-stderr.txt"), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "{}: {}",
        work.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn public_json_limits_large_unicode_copy_and_ownership() {
    let work = scratch("public");
    let source = work.join("main.sg");
    fs::copy(
        repository().join("tools/stdlib/tests/json_bounded.sg"),
        &source,
    )
    .unwrap();
    let mut payload = String::from("\"");
    payload.push_str(&"a".repeat(1024 * 1024));
    payload.push_str("世界😀\\u0000end\"");
    let data = work.join("large.json");
    fs::write(&data, payload).unwrap();
    let binary = work.join(if cfg!(windows) {
        "public.exe"
    } else {
        "public"
    });
    let output = common::source_sgc_command()
        .arg("build")
        .arg(&source)
        .args(["--force-rebuild", "-O", "1", "-o"])
        .arg(&binary)
        .output()
        .unwrap();
    fs::write(work.join("build-stdout.txt"), &output.stdout).unwrap();
    fs::write(work.join("build-stderr.txt"), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "{}: {}",
        work.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(binary).arg(data).output().unwrap();
    fs::write(work.join("run-stdout.txt"), &output.stdout).unwrap();
    fs::write(work.join("run-stderr.txt"), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "{}: {} {}",
        work.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
