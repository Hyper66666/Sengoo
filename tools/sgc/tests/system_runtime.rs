use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn passed(output: Output, what: &str) {
    assert!(
        output.status.success(),
        "{what} ({:?}):\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn clang() -> PathBuf {
    if let Some(path) = std::env::var_os("SENGOO_CLANG") {
        return path.into();
    }
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let path = PathBuf::from(local)
                .join("SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe");
            if path.is_file() {
                return path;
            }
        }
    }
    "clang".into()
}
fn work() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "sengoo-system-中文-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn native_locks_paths_and_disposable_credentials() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = work();
    let actual = directory.join("Actual-路径");
    fs::create_dir(&actual).unwrap();
    let alias = directory.join("alias");
    #[cfg(windows)]
    passed(
        Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&alias)
            .arg(&actual)
            .output()
            .unwrap(),
        "create test junction",
    );
    #[cfg(unix)]
    std::os::unix::fs::symlink(&actual, &alias).unwrap();
    let binary = directory.join(if cfg!(windows) {
        "system.exe"
    } else {
        "system"
    });
    let mut compile = Command::new(clang());
    compile
        .args(["-std=c11", "-O1", "-Wno-unused-function"])
        .arg(repo.join("tools/stdlib/tests/system_probe.c"))
        .arg("-o")
        .arg(&binary);
    if cfg!(windows) {
        compile.arg("-lAdvapi32");
    }
    passed(compile.output().unwrap(), "compile system probe");
    let lock = directory.join("permanent.lock");
    let canonical_alias = if cfg!(windows) {
        PathBuf::from(alias.to_string_lossy().to_lowercase())
    } else {
        alias
    };
    passed(
        Command::new(&binary)
            .arg("--all")
            .arg(&lock)
            .arg(&actual)
            .arg(&canonical_alias)
            .output()
            .unwrap(),
        "system OS probe",
    );
    let mut owner = Command::new(&binary)
        .arg("--hold")
        .arg(&lock)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(owner.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert_eq!(line.trim(), "held");
    let contended = Command::new(&binary)
        .arg("--check")
        .arg(&lock)
        .output()
        .unwrap();
    assert_eq!(
        contended.status.code(),
        Some(11),
        "competing process must fail nonblocking with timeout"
    );
    owner.stdin.take().unwrap().write_all(b"release\n").unwrap();
    assert!(owner.wait().unwrap().success());
    assert!(lock.is_file(), "closing never unlinks a lock identity");
    passed(
        Command::new(&binary)
            .arg("--check")
            .arg(&lock)
            .output()
            .unwrap(),
        "lock released across processes",
    );
}

#[test]
fn public_sengoo_file_lock_and_string_ownership() {
    let directory = work();
    let path = directory
        .join("public.lock")
        .to_string_lossy()
        .replace('\\', "/");
    let source = directory.join("main.sg");
    fs::write(
        &source,
        r#"
import std::system;
extern "C" {
    fn sengoo_string_live_handle_count() -> i64;
    fn sengoo_opaque_live_handle_count() -> i64;
}
def own() -> i64 {
    let mut lock = file_lock("LOCK_PATH");
    if !lock.is_ok { return 1; }
    let other = file_lock("LOCK_PATH");
    if other.is_ok || other.error != 11 { return 2; }
    let path = canonical_path_string("LOCK_PATH");
    if !path.is_ok || path.value.len() == 0 { return 3; }
    if !lock.value.close().is_ok { return 4; }
    let reacquired = file_lock("LOCK_PATH");
    if !reacquired.is_ok { return 5; }
    return 0;
}
def main() -> i64 {
    let strings = sengoo_string_live_handle_count();
    let opaque = sengoo_opaque_live_handle_count();
    let result = own();
    if result != 0 { return result; }
    if sengoo_file_lock_live_count() != 0 { return 6; }
    if sengoo_string_live_handle_count() != strings { return 7; }
    if sengoo_opaque_live_handle_count() != opaque { return 8; }
    return 0;
}
"#
        .replace("LOCK_PATH", &path),
    )
    .unwrap();
    let binary = directory.join(if cfg!(windows) { "main.exe" } else { "main" });
    passed(
        Command::new(env!("CARGO_BIN_EXE_sgc"))
            .args(["--runtime-mode", "source-development", "build"])
            .arg(&source)
            .arg("--output")
            .arg(&binary)
            .env("SENGOO_CLANG", clang())
            .output()
            .unwrap(),
        "compile public system API",
    );
    passed(
        Command::new(&binary).output().unwrap(),
        "run public system API",
    );
}
