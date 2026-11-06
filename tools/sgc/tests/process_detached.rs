//! Explicit detached Windows launch returns identity evidence, no kill owner.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn root() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../..") }
fn clang() -> PathBuf {
    if let Some(value) = std::env::var_os("SENGOO_CLANG") { return value.into(); }
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let path = PathBuf::from(local).join("SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe");
            if path.is_file() { return path; }
        }
    }
    "clang".into()
}
fn workspace() -> PathBuf {
    let path = std::env::temp_dir().join(format!("sengoo-detached-用户 空格-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&path).unwrap(); path
}
fn passed(output: Output, label: &str) {
    assert!(output.status.success(),"{label}: {}\n{}\n{}",output.status,String::from_utf8_lossy(&output.stdout),String::from_utf8_lossy(&output.stderr));
}
fn watched(mut command: Command, label: &str) -> Output {
    let directory=workspace(); let out=directory.join("stdout"); let err=directory.join("stderr");
    let mut child=command.stdin(Stdio::null()).stdout(fs::File::create(&out).unwrap()).stderr(fs::File::create(&err).unwrap()).spawn().unwrap();
    let deadline=Instant::now()+Duration::from_secs(30);
    let status=loop {
        if let Some(status)=child.try_wait().unwrap(){break status;}
        if Instant::now()>=deadline{child.kill().unwrap();child.wait().unwrap();panic!("{label} exceeded 30s watchdog");}
        thread::sleep(Duration::from_millis(10));
    };
    Output{status,stdout:fs::read(out).unwrap(),stderr:fs::read(err).unwrap()}
}

#[test]
fn detached_native_identity_environment_and_parent_exit() {
    let binary=workspace().join(if cfg!(windows){"native.exe"}else{"native"});
    passed(Command::new(clang()).args(["-std=c11","-O1","-Wno-unused-function"]).arg(root().join("tools/stdlib/tests/process_detached_probe.c")).arg("-o").arg(&binary).output().unwrap(),"compile detached probe");
    let output=watched(Command::new(binary),"native detached probe");
    if cfg!(windows) { assert!(String::from_utf8_lossy(&output.stdout).contains("parent exit, forbidden breakaway")); }
    passed(output,"native detached probe");
}

#[test]
fn detached_sengoo_receipt_does_not_own_child_lifetime() {
    let directory=workspace(); let source=directory.join("main.sg"); let binary=directory.join(if cfg!(windows){"main.exe"}else{"main"});
    fs::write(&source,r#"
import std::process;
import std::time;
import std::env;

def run_test() -> Result<i64,i64> {
    if not env_is_windows() {
        let unavailable=process_command("unused")?;
        if unavailable.spawn_detached().is_ok() { return process_i64_result(-99); };
        return process_i64_result(0);
    };
    let command=process_command("C:\\Windows\\System32\\cmd.exe")?;
    command.arg("/d")?; command.arg("/c")?;
    command.arg("ping -n 2 127.0.0.1 >NUL")?;
    let receipt=command.spawn_detached()?;
    if receipt.pid <= 0 or receipt.creation_identity <= 0 or not receipt.is_running()? { return process_i64_result(-91); };
    if process_identity_matches(receipt.pid,receipt.creation_identity+1)? { return process_i64_result(-92); };
    let started=time_unix_ms();
    while receipt.is_running()? { if time_since_ms(started)>10000 { return process_i64_result(-93); }; time_sleep_ms(5); };
    if sengoo_process_live_handle_count() != 0 { return process_i64_result(-94); };
    process_i64_result(0)
}
def main() -> i64 { let result=run_test(); if not result.is_ok() { println(result.error); return 1; }; 0 }
"#).unwrap();
    let compiler=std::env::var_os("SENGOO_TEST_SGC").map(PathBuf::from).unwrap_or_else(||root().join("target/debug/sgc.exe"));
    let mut build=Command::new(compiler);
    build.current_dir(root()).args(["--runtime-mode","source-development","build"]).arg(&source).args(["-O","0","-o"]).arg(&binary);
    if let Some(parent)=clang().parent() {
        let mut paths=vec![parent.to_path_buf()];
        if let Some(path)=std::env::var_os("PATH"){paths.extend(std::env::split_paths(&path));}
        build.env("PATH",std::env::join_paths(paths).unwrap());
    }
    passed(build.output().unwrap(),"compile detached Sengoo wrapper");
    passed(watched(Command::new(binary),"Sengoo detached wrapper"),"Sengoo detached wrapper");
}
