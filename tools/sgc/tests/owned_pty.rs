//! Owning Windows ConPTY API: isolated output, bounded I/O and tree cleanup.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn clang() -> PathBuf {
    if let Some(value) = std::env::var_os("SENGOO_CLANG") {
        return value.into();
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
        "sengoo-pty-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
fn passed(output: Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} ({}):\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn watched(mut command: Command, label: &str) -> Output {
    // File redirection is intentional: the PTY child must receive its own
    // console handles, and must not leak output into the caller's files.
    let directory = work();
    let stdout = directory.join("stdout.log");
    let stderr = directory.join("stderr.log");
    let mut child = command
        .stdin(Stdio::null())
        .stdout(fs::File::create(&stdout).unwrap())
        .stderr(fs::File::create(&stderr).unwrap())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(45);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("{label} exceeded 45s watchdog; closing the owner job on process exit terminates its children\n{}\n{}", fs::read_to_string(stdout).unwrap(), fs::read_to_string(stderr).unwrap());
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Output {
        status,
        stdout: fs::read(stdout).unwrap(),
        stderr: fs::read(stderr).unwrap(),
    }
}
#[test]
fn native_pseudoconsole_interaction_pressure_and_lifetime() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = work();
    let binary = directory.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let mut command = Command::new(clang());
    command
        .args([
            "-std=c11",
            "-O1",
            "-Wall",
            "-Wextra",
            "-Wno-unused-function",
            "-Wno-cast-function-type-mismatch",
        ])
        .arg(root.join("tools/stdlib/tests/pty_probe.c"));
    if cfg!(windows) {
        command.arg("-ladvapi32");
    }
    passed(
        command
            .arg("-o")
            .arg(&binary)
            .output()
            .expect("clang required"),
        "compile native ConPTY probe",
    );
    passed(
        watched(Command::new(binary), "native ConPTY probe"),
        "native ConPTY probe",
    );
}
#[test]
fn sengoo_pseudoconsole_powershell_and_scope_drop() {
    let directory = work();
    let source = directory.join("main.sg");
    fs::write(&source, r#"
import std::pty;
import std::time;

def exercise() -> i64 {
    if !pty_available() {
        let executable = string_from_str("unsupported-host").value;
        if pty_command(&executable).is_ok { return 1; }
        return 0;
    }
    let executable = string_from_str("powershell.exe").value;
    let command = pty_command(&executable);
    if !command.is_ok { return 2; }
    let no_logo = string_from_str("-NoLogo").value;
    let no_profile = string_from_str("-NoProfile").value;
    let command_flag = string_from_str("-Command").value;
    let script = string_from_str("Write-Host 'READY-中文😀'; $line=Read-Host; Write-Host ('ECHO-'+$line); exit 9").value;
    if !command.value.arg(&no_logo).is_ok || !command.value.arg(&no_profile).is_ok || !command.value.arg(&command_flag).is_ok || !command.value.arg(&script).is_ok { return 3; }
    let process = command.value.spawn(96, 28);
    if !process.is_ok { return 4; }
    let mut buffer = ffi_buffer_new(4096).value;
    let output = string_new();
    let collected = ffi_buffer_new(65536).value;
    let mut rounds = 0;
    while rounds < 600 {
        let count = process.value.read(&mut buffer);
        if !count.is_ok { return 5; }
        if count.value > 0 {
            let copied = collected.append_copy(&buffer, 0, count.value);
            if !copied.is_ok { return 60 + copied.error; }
            if collected.is_utf8() {
                let raw = sengoo_string_from_buffer(collected.raw_handle(), collected.used_len());
                if raw < 0 { return 100 - raw; }
                let piece = String { handle: raw };
                let cleared = output.clear();
                let appended = output.push_str(piece.as_str());
            }
        }
        if str_contains(output.as_str(), "READY-中文😀") { break; }
        if !process.value.poll(1, 20).is_ok { return 7; }
        rounds = rounds + 1;
    }
    if rounds == 600 { return 8; }
    if !process.value.resize(80, 24).is_ok { return 9; }
    if process.value.try_wait().value || process.value.reap(0).is_ok { return 10; }
    buffer.clear(); buffer.append_str("你好😀\r");
    let written = process.value.write(&buffer, buffer.used_len());
    if !written.is_ok || written.value != buffer.used_len() { return 11; }
    buffer.clear();
    rounds = 0;
    while rounds < 600 {
        let count = process.value.read(&mut buffer);
        if !count.is_ok { return 12; }
        if count.value > 0 {
            let copied = collected.append_copy(&buffer, 0, count.value);
            if !copied.is_ok { return 13; }
            if collected.is_utf8() {
                let raw = sengoo_string_from_buffer(collected.raw_handle(), collected.used_len());
                if raw < 0 { return 13; }
                let piece = String { handle: raw };
                let cleared = output.clear();
                let appended = output.push_str(piece.as_str());
            }
        }
        if str_contains(output.as_str(), "ECHO-你好😀") && process.value.try_wait().value { break; }
        if !process.value.poll(1, 20).is_ok { return 14; }
        rounds = rounds + 1;
    }
    if rounds == 600 || process.value.exit_code().value != 9 { return 15; }
    let slow_script = string_from_str("Start-Sleep -Seconds 60").value;
    let slow = pty_command(&executable);
    slow.value.arg(&no_logo); slow.value.arg(&no_profile); slow.value.arg(&command_flag); slow.value.arg(&slow_script);
    let running = slow.value.spawn(80, 24);
    if !running.is_ok || running.value.try_wait().value || running.value.reap(1).is_ok { return 16; }
    // Scope exit must stop the still-live process without writing its terminal
    // bytes into this test's redirected stdout/stderr.
    return 0;
}

def main() -> i64 {
    let result = exercise();
    if result != 0 { return result; }
    if sengoo_pty_live_handle_count() != 0 { return 17; }
    return 0;
}
"#).unwrap();
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
            .arg(source)
            .arg("--output")
            .arg(&binary)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .output()
            .unwrap(),
        "compile Sengoo ConPTY API",
    );
    let output = watched(Command::new(binary), "Sengoo ConPTY API");
    assert!(
        output.stdout.is_empty() && output.stderr.is_empty(),
        "PTY output escaped into caller streams"
    );
    passed(output, "Sengoo PowerShell ConPTY and Drop");
}

#[test]
fn pseudoconsole_owners_reject_cross_thread_marker_bounds() {
    let directory = work();
    for owner in ["PtyCommand", "PtyProcess"] {
        for marker in ["Send", "Sync"] {
            let source = directory.join(format!("{owner}_{marker}.sg"));
            fs::write(&source, format!("import std::pty;\ntrait {marker} {{}}\ndef require_marker<T: {marker}>(value: T) -> i64 {{ 0 }}\ndef main() -> i64 {{ require_marker({owner} {{ handle: 0 }}) }}\n")).unwrap();
            let output = Command::new(env!("CARGO_BIN_EXE_sgc"))
                .args(["--runtime-mode", "source-development", "check"])
                .arg(&source)
                .output()
                .unwrap();
            let diagnostic = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                !output.status.success()
                    && diagnostic.contains(&format!("does not implement `{marker}`")),
                "{owner} must reject {marker}: {diagnostic}"
            );
        }
    }
}
