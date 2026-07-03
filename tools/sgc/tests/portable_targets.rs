use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn sgc() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sgc"))
}

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("sgc_portable_{name}_{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_scalar_program(root: &Path) -> PathBuf {
    let source = root.join("main.sg");
    fs::write(
        &source,
        r#"
def choose(value: i64) -> i64 {
    if value >= 40 {
        value + 2
    } else {
        0
    }
}

def main() -> i64 {
    choose(40)
}
"#,
    )
    .unwrap();
    source
}

fn write_recursive_program(root: &Path) -> PathBuf {
    let source = root.join("recursive.sg");
    fs::write(
        &source,
        r#"
def fib(n: i64) -> i64 {
    if n <= 1 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

def main() -> i64 {
    fib(7)
}
"#,
    )
    .unwrap();
    source
}

#[test]
fn bytecode_target_builds_and_runs_without_native_toolchain() {
    let dir = temp_dir("bytecode");
    let source = write_scalar_program(&dir);
    let artifact = dir.join("app.sgbc");
    let build = Command::new(sgc())
        .args([
            "build",
            source.to_str().unwrap(),
            "--target",
            "bytecode",
            "--output",
            artifact.to_str().unwrap(),
        ])
        .env_remove("PATH")
        .output()
        .expect("build bytecode");
    assert!(
        build.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
        fs::read(&artifact).unwrap().starts_with(b"SGB1"),
        "bytecode artifact should carry the stable magic"
    );

    let run = Command::new(sgc())
        .args(["run", source.to_str().unwrap(), "--target", "bytecode"])
        .env_remove("PATH")
        .output()
        .expect("run bytecode");
    assert_eq!(
        run.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn bytecode_target_matches_native_for_recursive_scalar_program() {
    let dir = temp_dir("bytecode_recursive");
    let source = write_recursive_program(&dir);
    let native = Command::new(sgc())
        .args(["run", source.to_str().unwrap(), "--force-rebuild"])
        .output()
        .expect("run native");

    let bytecode = Command::new(sgc())
        .args(["run", source.to_str().unwrap(), "--target", "bytecode"])
        .env_remove("PATH")
        .output()
        .expect("run bytecode");
    assert_eq!(
        bytecode.status.code(),
        native.status.code(),
        "native stdout:\n{}\nnative stderr:\n{}\nbytecode stdout:\n{}\nbytecode stderr:\n{}",
        String::from_utf8_lossy(&native.stdout),
        String::from_utf8_lossy(&native.stderr),
        String::from_utf8_lossy(&bytecode.stdout),
        String::from_utf8_lossy(&bytecode.stderr)
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn wasm_target_emits_a_valid_exported_main_module() {
    let dir = temp_dir("wasm");
    let source = write_scalar_program(&dir);
    let artifact = dir.join("app.wasm");
    let build = Command::new(sgc())
        .args([
            "build",
            source.to_str().unwrap(),
            "--target",
            "wasm",
            "--output",
            artifact.to_str().unwrap(),
        ])
        .output()
        .expect("build wasm");
    assert!(
        build.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let bytes = fs::read(&artifact).unwrap();
    assert!(bytes.starts_with(b"\0asm\x01\0\0\0"));

    if let Ok(node) = which::which("node") {
        let script = dir.join("run-wasm.js");
        fs::write(
            &script,
            "const fs=require('fs'); const b=fs.readFileSync(process.argv[2]); WebAssembly.instantiate(b).then(({instance})=>process.exit(instance.exports.main()===42n?0:1));\n",
        )
        .unwrap();
        let run = Command::new(node)
            .args([script.to_str().unwrap(), artifact.to_str().unwrap()])
            .output()
            .expect("run wasm with node");
        assert!(
            run.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        );
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn portable_targets_reject_unsupported_stdlib_with_documented_diagnostic() {
    let dir = temp_dir("unsupported_stdlib");
    let source = dir.join("time.sg");
    fs::write(
        &source,
        r#"
import std::time;

def main() -> i64 {
    time_unix_ms()
}
"#,
    )
    .unwrap();
    let build = Command::new(sgc())
        .args(["build", source.to_str().unwrap(), "--target", "bytecode"])
        .output()
        .expect("build unsupported bytecode");
    assert!(
        !build.status.success(),
        "unsupported stdlib should fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        stderr.contains("portable target does not support")
            && stderr.contains("docs/portable-targets.md"),
        "stderr:\n{stderr}"
    );
    let _ = fs::remove_dir_all(dir);
}
