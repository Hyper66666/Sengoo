//! Local native byte transport and owning Sengoo wrappers.
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
        "sengoo-ipc-{}-{}",
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
        "{label}:\n{}\n{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
}
#[test]
fn real_same_logon_ipc_pressure_security_and_cleanup() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let binary = work().join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let mut command = Command::new(clang());
    command
        .args(["-std=c11", "-O1", "-Wall", "-Wextra", "-Werror"])
        .arg(repo.join("tools/stdlib/tests/ipc_probe.c"));
    if cfg!(windows) {
        command.arg("-ladvapi32");
    }
    passed(
        command
            .arg("-o")
            .arg(&binary)
            .output()
            .expect("clang required"),
        "compile local IPC probe",
    );
    passed(
        Command::new(&binary).output().unwrap(),
        "real local IPC probe",
    );
}
#[test]
fn sengoo_owning_local_ipc() {
    let directory = work();
    let source = directory.join("main.sg");
    let endpoint = format!(
        "sg-api-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    fs::write(
        &source,
        r#"
import std::ipc;
import std::ffi;
def exercise() -> i64 {
    let listener = ipc_listen("ENDPOINT");
    if !listener.is_ok { return 10; }
    let empty = listener.value.try_accept();
    if !empty.is_ok || empty.value.is_open() { return 11; }
    let mut client = ipc_connect("ENDPOINT");
    if !client.is_ok || !client.value.is_open() { return 12; }
    let mut server = listener.value.try_accept();
    if !server.is_ok || !server.value.is_open() { return 13; }
    let mut data = ffi_buffer_new(8);
    if !data.is_ok { return 14; }
    data.value.append_str("hi");
    let written = client.value.write(&data.value, 2);
    if !written.is_ok || written.value != 2 { return 15; }
    let writable = client.value.poll(2, 1000);
    if !writable.is_ok || writable.value != 2 { return 16; }
    let readable = server.value.poll(1, 1000);
    if !readable.is_ok || readable.value != 1 { return 17; }
    let count = server.value.read(&mut data.value);
    if !count.is_ok || count.value != 2 || data.value.used_len() != 2 { return 18; }
    if data.value.get_u8(0).value != 104 || data.value.get_u8(1).value != 105 { return 19; }
    if !client.value.close().is_ok { return 20; }
    if server.value.poll(1, 1000).value != 4 || !server.value.eof().value { return 21; }
    if !server.value.close().is_ok { return 22; }
    return 0;
}
def main() -> i64 {
    let outcome = exercise();
    if outcome != 0 { return outcome; }
    if sengoo_ipc_live_handle_count() != 0 { return 23; }
    return 0;
}
"#
        .replace("ENDPOINT", &endpoint),
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
        "compile SG owning IPC",
    );
    if cfg!(windows) {
        passed(Command::new(&binary).output().unwrap(), "SG owning IPC");
    }
}
