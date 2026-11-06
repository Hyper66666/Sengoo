//! Windows ready input must not wait behind irrelevant console records.
#![cfg(windows)]
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn clang() -> PathBuf {
    if let Some(value) = std::env::var_os("SENGOO_CLANG") {
        return value.into();
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let candidate = PathBuf::from(local)
            .join("SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe");
        if candidate.is_file() {
            return candidate;
        }
    }
    "clang".into()
}
fn passed(output: Output, what: &str) {
    assert!(
        output.status.success(),
        "{what}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn ready_keys_surrogate_pairs_and_bounded_ignored_records() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = std::env::temp_dir().join(format!(
        "sengoo-terminal-poll-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&directory).unwrap();
    let compiler = clang();
    let native = directory.join("poll.exe");
    passed(
        Command::new(&compiler)
            .args(["-std=c11", "-O1", "-Wno-unused-function"])
            .arg(repo.join("tools/stdlib/tests/terminal_poll_probe.c"))
            .arg("-o")
            .arg(&native)
            .output()
            .expect("clang required"),
        "compile native polling probe",
    );
    let source = directory.join("main.sg");
    fs::write(
        &source,
        r#"
import std::terminal;
def main() -> i64 {
    let opened = terminal_open();
    if !opened.is_ok || !opened.value.is_tty() { return 40; }
    if !opened.value.enter_raw().is_ok { return 41; }
    let first = opened.value.read_event(0);
    if !first.is_ok || first.value.kind != 1 || first.value.code != 128512 { return 42; }
    if first.value.data.as_str() != "😀" { return 43; }
    let second = opened.value.read_event(0);
    if !second.is_ok || second.value.kind != 1 || second.value.data.as_str() != "中" { return 44; }
    let third = opened.value.read_event(0);
    if !third.is_ok || third.value.kind != 1 || third.value.data.as_str() != "z" { return 45; }
    let idle = opened.value.read_event(0);
    if !idle.is_ok || idle.value.kind != 0 { return 46; }
    if first.value.data.as_str() != "😀" { return 47; }
    return 0;
}
"#,
    )
    .unwrap();
    let public = directory.join("public.exe");
    let mut paths = compiler
        .parent()
        .into_iter()
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    passed(
        Command::new(env!("CARGO_BIN_EXE_sgc"))
            .args([
                "--runtime-mode",
                "source-development",
                "build",
                "--force-rebuild",
            ])
            .arg(&source)
            .arg("--output")
            .arg(&public)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .output()
            .unwrap(),
        "compile public Sengoo polling consumer",
    );
    passed(
        Command::new(&native)
            .arg(&public)
            .stdin(Stdio::null())
            .output()
            .unwrap(),
        "real hidden-console native and public Sengoo polling",
    );
}
