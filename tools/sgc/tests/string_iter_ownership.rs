//! Scope-exit ownership for strings' snapshot iterators; explicit free survives.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};
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
fn work() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "sengoo-iter-owner-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}
fn passed(o: Output, label: &str) {
    assert!(
        o.status.success(),
        "{label} ({}):\n{}\n{}",
        o.status,
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
}
#[test]
fn native_string_iterators_validate_tokens_and_reuse_metadata() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = work();
    let binary = directory.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    passed(
        Command::new(clang())
            .args([
                "-std=c11",
                "-O1",
                "-Wno-unused-function",
                "-Wno-deprecated-declarations",
            ])
            .arg(root.join("tools/stdlib/tests/string_iter_probe.c"))
            .arg("-o")
            .arg(&binary)
            .output()
            .expect("clang required"),
        "compile native iterator probe",
    );
    passed(
        Command::new(&binary).output().unwrap(),
        "native iterator generations and cleanup",
    );
}
#[test]
fn string_iterators_drop_on_early_return_and_exhaustion() {
    let directory = work();
    let source = directory.join("main.sg");
    fs::write(
        &source,
        r#"
import std::string;
def early() -> i64 {
    let text = string_from_str("中文,a,b").value;
    let chars = text.chars(); let bytes = text.bytes(); let split = text.split(",");
    let first = chars.next(); let byte = bytes.next(); let segment = split.next();
    if !first.is_some || !byte.is_some || !segment.is_some { return 10; }
    return 0;
}
def exhaust() -> i64 {
    let text = string_from_str("中文,a,b").value;
    let chars = text.chars(); let bytes = text.bytes(); let split = text.split(",");
    loop { if !chars.next().is_some { break; } }
    loop { if !bytes.next().is_some { break; } }
    loop { let segment = split.next(); if !segment.is_some { break; } }
    return 0;
}
def explicit() -> i64 {
    let text = string_from_str("中文,a,b").value;
    let chars = text.chars(); let bytes = text.bytes(); let split = text.split(",");
    if !chars.free() || !chars.free() || chars.next().is_some { return 20; }
    if !bytes.free() || !bytes.free() || bytes.next().is_some { return 21; }
    if !split.free() || !split.free() || split.next().is_some { return 22; }
    return 0;
}
def main() -> i64 {
    if early() != 0 { return 10; }
    if sengoo_string_iter_live_handle_count() != 0 { return 1; }
    if exhaust() != 0 || sengoo_string_iter_live_handle_count() != 0 { return 2; }
    if explicit() != 0 || sengoo_string_iter_live_handle_count() != 0 { return 3; }
    let mut index = 0;
    while index < 10000 {
        if early() != 0 || exhaust() != 0 || explicit() != 0 { return 4; }
        if sengoo_string_iter_live_handle_count() != 0 { return 5; }
        index = index + 1;
    }
    if sengoo_string_iter_slot_count() > 4 { return 6; }
    return 0;
}
"#,
    )
    .unwrap();
    let compiler = clang();
    let mut paths = compiler
        .parent()
        .into_iter()
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let binary = directory.join(if cfg!(windows) { "main.exe" } else { "main" });
    passed(
        Command::new(env!("CARGO_BIN_EXE_sgc"))
            .args(["--runtime-mode", "source-development", "build"])
            .arg(&source)
            .arg("--output")
            .arg(&binary)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .output()
            .unwrap(),
        "compile string iterator ownership",
    );
    passed(
        Command::new(&binary).output().unwrap(),
        "string iterator scope ownership",
    );
}
