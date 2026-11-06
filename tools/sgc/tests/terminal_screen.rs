//! Additive fullscreen and mouse boundary; real Windows console plus SG API.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn clang() -> PathBuf {
    std::env::var_os("SENGOO_CLANG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                    let p = PathBuf::from(local).join(
                        "SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe",
                    );
                    if p.is_file() {
                        return p;
                    }
                }
            }
            "clang".into()
        })
}
fn passed(output: Output, what: &str) {
    assert!(
        output.status.success(),
        "{what}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn work() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "sengoo-screen-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn native_fullscreen_mouse_and_restoration() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let binary = work().join(if cfg!(windows) { "probe.exe" } else { "probe" });
    passed(
        Command::new(clang())
            .args(["-std=c11", "-O1", "-Wno-unused-function"])
            .arg(repo.join("tools/stdlib/tests/terminal_screen_probe.c"))
            .arg("-o")
            .arg(&binary)
            .output()
            .expect("clang required"),
        "compile terminal screen probe",
    );
    passed(
        Command::new(&binary).stdin(Stdio::null()).output().unwrap(),
        "terminal fullscreen/mouse probe",
    );
}
#[test]
fn clipboard_isolated_transport_fixture() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let binary = work().join(if cfg!(windows) {
        "clipboard.exe"
    } else {
        "clipboard"
    });
    passed(
        Command::new(clang())
            .args(["-std=c11", "-O1", "-Wno-unused-function"])
            .arg(repo.join("tools/stdlib/tests/clipboard_probe.c"))
            .arg("-o")
            .arg(&binary)
            .output()
            .expect("clang required"),
        "compile isolated clipboard fixture",
    );
    passed(
        Command::new(&binary).stdin(Stdio::null()).output().unwrap(),
        "isolated clipboard ownership fixture",
    );
}
#[test]
fn sengoo_fullscreen_mouse_api() {
    let directory = work();
    let source = directory.join("main.sg");
    fs::write(&source, r#"
import std::terminal;
def exercise() -> i64 {
    let terminal = terminal_open();
    if !terminal.is_ok || terminal.value.is_tty() { return 10; }
    if terminal.value.enter_fullscreen().is_ok { return 11; }
    if !terminal.value.leave_fullscreen().is_ok { return 12; }
    if terminal.value.set_mouse(true).is_ok { return 13; }
    let invalid = string_new(); invalid.push_char('a'); invalid.push_char('\0'); invalid.push_char('b');
    if invalid.len() != 3 { return 17; }
    if terminal_copy_text(&invalid).is_ok { return 16; }
    let mouse = terminal.value.mouse();
    if mouse.action != 0 || mouse.scroll_x != 0 || mouse.scroll_y != 0 { return 14; }
    return 0;
}
def main() -> i64 {
    let result = exercise();
    if result != 0 { return result; }
    if sengoo_terminal_live_handle_count() != 0 { return 15; }
    return 0;
}
"#).unwrap();
    let binary = directory.join(if cfg!(windows) { "main.exe" } else { "main" });
    let compiler = clang();
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
            .args(["--runtime-mode", "source-development", "build"])
            .arg(&source)
            .arg("--output")
            .arg(&binary)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .output()
            .unwrap(),
        "compile fullscreen/mouse SG API",
    );
    passed(
        Command::new(&binary).stdin(Stdio::null()).output().unwrap(),
        "fullscreen/mouse SG API",
    );
}
