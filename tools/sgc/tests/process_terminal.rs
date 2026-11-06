//! Real native process/terminal boundary checks; no optional silent skips.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn passed(output: Output, what: &str) {
    assert!(
        output.status.success(),
        "{what}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn clang() -> PathBuf {
    if let Some(value) = std::env::var_os("SENGOO_CLANG") {
        return value.into();
    }
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let value = PathBuf::from(local)
                .join("SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe");
            if value.is_file() {
                return value;
            }
        }
    }
    "clang".into()
}
fn work(label: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "sengoo-process-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn real_pipes_tree_timeout_and_terminal_parser() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = work("native-中文");
    let binary = directory.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    passed(
        Command::new(clang())
            .args(["-std=c11", "-O1", "-Wno-unused-function"])
            .arg(repo.join("tools/stdlib/tests/process_terminal_probe.c"))
            .arg("-o")
            .arg(&binary)
            .output()
            .expect("clang required"),
        "compile native probe",
    );
    passed(
        Command::new(&binary).stdin(Stdio::null()).output().unwrap(),
        "process/terminal native probe",
    );
}

#[test]
fn sengoo_owning_process_and_terminal_apis() {
    let directory = work("sg");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let child_binary = directory.join(if cfg!(windows) { "child.exe" } else { "child" });
    passed(
        Command::new(clang())
            .args(["-std=c11", "-O1", "-Wno-unused-function"])
            .arg(repo.join("tools/stdlib/tests/process_terminal_probe.c"))
            .arg("-o")
            .arg(&child_binary)
            .output()
            .unwrap(),
        "compile SG child fixture",
    );
    let source = directory.join("main.sg");
    fs::write(
        &source,
        r#"
import std::process;
import std::terminal;
import std::ffi;
import std::time;
def exercise() -> i64 {
    let terminal = terminal_open();
    if !terminal.is_ok { return 10; }
    if terminal.value.is_tty() { return 11; }
    let raw = terminal.value.enter_raw();
    if raw.is_ok { return 12; }
    let event = terminal.value.read_event(10);
    if !event.is_ok { return 13; }
    if event.value.kind != 5 { return 14; }
    let command = process_command("missing-sengoo-process-probe-executable");
    if !command.is_ok { return 15; }
    let child = command.value.spawn_piped();
    if child.is_ok { return 16; }
    let flood = process_command("CHILD_PATH");
    if !flood.is_ok { return 17; }
    flood.value.arg("--flood");
    let process = flood.value.spawn_piped();
    if !process.is_ok { return 18; }
    let ignored = ffi_buffer_from_bytes("ignored");
    if !ignored.is_ok || process.value.try_write_stdin(&ignored.value, 7).value != 7 { return 27; }
    process.value.close_stdin();
    let mut out_buffer = ffi_buffer_new(4096);
    let mut err_buffer = ffi_buffer_new(4096);
    if !out_buffer.is_ok || !err_buffer.is_ok { return 19; }
    let mut out_count = 0;
    let mut err_count = 0;
    let mut attempts = 0;
    while attempts < 20000 {
        let out = process.value.read_stdout(&mut out_buffer.value);
        let err = process.value.read_stderr(&mut err_buffer.value);
        if !out.is_ok || !err.is_ok { return 20; }
        out_count = out_count + out.value;
        err_count = err_count + err.value;
        let done = process.value.try_wait();
        let out_eof = process.value.stdout_eof();
        let err_eof = process.value.stderr_eof();
        if done.value && out_eof.value && err_eof.value {
            if out_count != 524288 || err_count != 524288 { return 21; }
            if process.value.reap(0).value != 0 { return 22; }
            return 0;
        }
        time_sleep_ms(1);
        attempts = attempts + 1;
    }
    return 23;
}
def main() -> i64 {
    let outcome = exercise();
    if outcome != 0 { return outcome; }
    if sengoo_terminal_live_handle_count() != 0 { return 24; }
    if sengoo_process_live_handle_count() != 0 { return 25; }
    if sengoo_process_live_resource_count() != 0 { return 26; }
    return 0;
}
"#
        .replace(
            "CHILD_PATH",
            &child_binary.to_string_lossy().replace('\\', "/"),
        ),
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
        "compile SG process/terminal API",
    );
    passed(
        Command::new(&binary).stdin(Stdio::null()).output().unwrap(),
        "SG owning process/terminal API",
    );
}
