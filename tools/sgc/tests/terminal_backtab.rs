//! Standard byte BackTab and native Windows Shift+Tab share one public key.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn clang() -> PathBuf {
    if let Some(value) = std::env::var_os("SENGOO_CLANG") { return value.into(); }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let candidate = PathBuf::from(local).join("SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe");
        if candidate.is_file() { return candidate; }
    } "clang".into()
}
fn passed(output: Output, what: &str) {
    assert!(output.status.success(), "{what}:\n{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}
#[test]
fn standard_backtab_split_input_and_native_console_parity() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = std::env::temp_dir().join(format!("sengoo-terminal-backtab-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&directory).unwrap();
    let compiler = clang();
    let native = directory.join(if cfg!(windows) { "backtab.exe" } else { "backtab" });
    passed(Command::new(&compiler).args(["-std=c11", "-O1", "-Wno-unused-function"])
        .arg(repo.join("tools/stdlib/tests/terminal_backtab_probe.c")).arg("-o").arg(&native).output().expect("clang required"), "compile native BackTab probe");
    passed(Command::new(&native).stdin(Stdio::null()).output().unwrap(), "native complete/split/paste parser and owned console parity");
    let public = directory.join(if cfg!(windows) { "public.exe" } else { "public" });
    let mut paths = compiler.parent().into_iter().map(Path::to_path_buf).collect::<Vec<_>>();
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
    passed(Command::new(env!("CARGO_BIN_EXE_sgc")).args(["--runtime-mode", "source-development", "build", "--force-rebuild"])
        .arg(repo.join("tools/stdlib/tests/terminal_backtab.sg")).arg("--output").arg(&public)
        .env("PATH", std::env::join_paths(paths).unwrap()).output().unwrap(), "compile public Sengoo BackTab consumer");
    let mut child = Command::new(&public).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"\x1b[Z\tx").unwrap();
    passed(child.wait_with_output().unwrap(), "public Sengoo BackTab/Tab/next-byte/EOF");
    fs::remove_file(native).unwrap(); fs::remove_file(public).unwrap();
    // Some native toolchains emit an adjacent debug symbol file.
    for entry in fs::read_dir(&directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() { fs::remove_file(path).unwrap(); }
    }
    fs::remove_dir(directory).unwrap();
}
