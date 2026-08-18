mod common;

use common::source_sgc_command;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
    let build = source_sgc_command()
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

    let run = source_sgc_command()
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
    let native = source_sgc_command()
        .args(["run", source.to_str().unwrap(), "--force-rebuild"])
        .output()
        .expect("run native");

    let bytecode = source_sgc_command()
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
    let build = source_sgc_command()
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
    let build = source_sgc_command()
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
    // miette may soft-wrap long diagnostics and inject box-drawing prefixes, so
    // match the stable code, target field, and path fragments independently.
    assert!(
        stderr.contains("unsupported-target-capability")
            && stderr.contains("target `bytecode`")
            && stderr.contains("docs/portable")
            && stderr.contains("targets.md"),
        "stderr:\n{stderr}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn wasm_run_rejects_hostile_type_count_without_panic() {
    let dir = temp_dir("wasm_hostile_count");
    let artifact = dir.join("hostile.wasm");
    // Minimal malformed module: type section claims an enormous vector count.
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    bytes.push(1); // type section
    bytes.push(5); // payload length
    bytes.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]); // huge ULEB count
    fs::write(&artifact, &bytes).unwrap();

    let run = source_sgc_command()
        .args(["run", artifact.to_str().unwrap(), "--target", "wasm"])
        .output()
        .expect("run hostile wasm");
    assert_ne!(
        run.status.code(),
        Some(101),
        "hostile count must not abort as capacity overflow panic\nstderr:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        !run.status.success(),
        "hostile module must fail closed with a diagnostic"
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("invalid WebAssembly")
            || stderr.contains("truncated")
            || stderr.contains("missing"),
        "stderr:\n{stderr}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn wasm_target_rejects_main_with_parameters() {
    let dir = temp_dir("wasm_main_params");
    let source = dir.join("main.sg");
    fs::write(
        &source,
        r#"
def main(value: i64) -> i64 {
    value
}
"#,
    )
    .unwrap();

    let build = source_sgc_command()
        .args(["build", source.to_str().unwrap(), "--target", "wasm"])
        .output()
        .expect("build wasm main-with-params");
    assert!(
        !build.status.success(),
        "parameterized main must fail closed for wasm ABI"
    );
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        stderr.contains("unsupported-target-capability")
            && stderr.contains("target `wasm`")
            && (stderr.contains("parameter") || stderr.contains("main() -> i64")),
        "stderr:\n{stderr}"
    );

    // External artifacts that export a non-() main must also fail validation
    // before the Node runner calls main().
    // type: (func (param i64) (result i64))
    let types = [0x01u8, 0x60, 0x01, 0x7e, 0x01, 0x7e];
    let mut exports = vec![0x01];
    exports.push(4); // name len
    exports.extend_from_slice(b"main");
    exports.push(0x00); // func
    exports.push(0x00); // index
                        // code: one function, 0 locals, i64.const 0, end
    let body = [0x00u8, 0x42, 0x00, 0x0b];
    let mut code = vec![0x01, body.len() as u8];
    code.extend_from_slice(&body);
    let mut custom = Vec::new();
    let name = b"sengoo.portable_runtime_abi";
    custom.push(name.len() as u8);
    custom.extend_from_slice(name);
    custom.push(1); // mir
    custom.push(1); // runtime
    let target_name = b"wasm32";
    custom.push(target_name.len() as u8);
    custom.extend_from_slice(target_name);

    let mut ordered = b"\0asm\x01\0\0\0".to_vec();
    write_raw_wasm_section(&mut ordered, 0, &custom);
    write_raw_wasm_section(&mut ordered, 1, &types);
    write_raw_wasm_section(&mut ordered, 3, &[0x01, 0x00]);
    write_raw_wasm_section(&mut ordered, 7, &exports);
    write_raw_wasm_section(&mut ordered, 10, &code);

    let artifact = dir.join("bad-main.wasm");
    fs::write(&artifact, &ordered).unwrap();
    let run = source_sgc_command()
        .args(["run", artifact.to_str().unwrap(), "--target", "wasm"])
        .output()
        .expect("run wasm bad-main artifact");
    assert!(
        !run.status.success(),
        "non-() main artifact must fail validation"
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("() -> i64") || stderr.contains("param"),
        "stderr:\n{stderr}"
    );
    let _ = fs::remove_dir_all(dir);
}

fn write_raw_wasm_section(module: &mut Vec<u8>, id: u8, payload: &[u8]) {
    module.push(id);
    let mut len = payload.len();
    loop {
        let mut byte = (len & 0x7f) as u8;
        len >>= 7;
        if len != 0 {
            byte |= 0x80;
        }
        module.push(byte);
        if len == 0 {
            break;
        }
    }
    module.extend_from_slice(payload);
}

#[test]
fn wasm_target_runs_scalar_main_with_host_runtime() {
    let dir = temp_dir("wasm_run");
    let source = write_scalar_program(&dir);
    let run = source_sgc_command()
        .args(["run", source.to_str().unwrap(), "--target", "wasm"])
        .output()
        .expect("run wasm");
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
fn wasm_target_rejects_aggregate_and_host_stdlib_with_stable_capability_diagnostic() {
    let dir = temp_dir("wasm_aggregate");
    let source = dir.join("main.sg");
    // Struct aggregates typecheck natively but are outside the scalar WASM MIR
    // subset; rejection must use the stable capability diagnostic code.
    fs::write(
        &source,
        r#"
struct Point {
    x: i64,
    y: i64,
}

def main() -> i64 {
    let p = Point { x: 1, y: 2 };
    p.x + p.y
}
"#,
    )
    .unwrap();
    let build = source_sgc_command()
        .args(["build", source.to_str().unwrap(), "--target", "wasm"])
        .output()
        .expect("build aggregate wasm");
    assert!(
        !build.status.success(),
        "aggregates must fail closed on wasm v1"
    );
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        stderr.contains("unsupported-target-capability") && stderr.contains("target `wasm`"),
        "stderr:\n{stderr}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn wasm_target_unsigned_compare_matches_native() {
    let dir = temp_dir("wasm_unsigned_cmp");
    let source = dir.join("main.sg");
    fs::write(
        &source,
        r#"
def main() -> i64 {
    if 18446744073709551615u64 > 0u64 {
        42
    } else {
        1
    }
}
"#,
    )
    .unwrap();

    let native = source_sgc_command()
        .args(["run", source.to_str().unwrap(), "--force-rebuild"])
        .output()
        .expect("run native unsigned compare");
    let wasm = source_sgc_command()
        .args(["run", source.to_str().unwrap(), "--target", "wasm"])
        .output()
        .expect("run wasm unsigned compare");
    assert_eq!(
        wasm.status.code(),
        native.status.code(),
        "native stdout:\n{}\nnative stderr:\n{}\nwasm stdout:\n{}\nwasm stderr:\n{}",
        String::from_utf8_lossy(&native.stdout),
        String::from_utf8_lossy(&native.stderr),
        String::from_utf8_lossy(&wasm.stdout),
        String::from_utf8_lossy(&wasm.stderr)
    );
    assert_eq!(wasm.status.code(), Some(42));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn wasm_target_narrowing_cast_matches_native() {
    let dir = temp_dir("wasm_narrowing_cast");
    let source = dir.join("main.sg");
    fs::write(
        &source,
        r#"
def main() -> i64 {
    if ((4294967296u64 as u32) as u64) == 0u64 {
        42
    } else {
        1
    }
}
"#,
    )
    .unwrap();

    let native = source_sgc_command()
        .args(["run", source.to_str().unwrap(), "--force-rebuild"])
        .output()
        .expect("run native narrowing cast");
    let wasm = source_sgc_command()
        .args(["run", source.to_str().unwrap(), "--target", "wasm"])
        .output()
        .expect("run wasm narrowing cast");
    assert_eq!(
        wasm.status.code(),
        native.status.code(),
        "native stdout:\n{}\nnative stderr:\n{}\nwasm stdout:\n{}\nwasm stderr:\n{}",
        String::from_utf8_lossy(&native.stdout),
        String::from_utf8_lossy(&native.stderr),
        String::from_utf8_lossy(&wasm.stdout),
        String::from_utf8_lossy(&wasm.stderr)
    );
    assert_eq!(wasm.status.code(), Some(42));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn wasm_target_signed_and_unsigned_extensions_match_native() {
    let dir = temp_dir("wasm_cast_extensions");
    let source = dir.join("main.sg");
    fs::write(
        &source,
        r#"
def main() -> i64 {
    if ((4294967295u32 as i32) as i64) == -1i64 {
        if ((-1i32 as u32) as u64) == 4294967295u64 {
            42
        } else {
            2
        }
    } else {
        1
    }
}
"#,
    )
    .unwrap();

    let native = source_sgc_command()
        .args(["run", source.to_str().unwrap(), "--force-rebuild"])
        .output()
        .expect("run native cast extensions");
    let wasm = source_sgc_command()
        .args(["run", source.to_str().unwrap(), "--target", "wasm"])
        .output()
        .expect("run wasm cast extensions");
    assert_eq!(
        wasm.status.code(),
        native.status.code(),
        "native stdout:\n{}\nnative stderr:\n{}\nwasm stdout:\n{}\nwasm stderr:\n{}",
        String::from_utf8_lossy(&native.stdout),
        String::from_utf8_lossy(&native.stderr),
        String::from_utf8_lossy(&wasm.stdout),
        String::from_utf8_lossy(&wasm.stderr)
    );
    assert_eq!(wasm.status.code(), Some(42));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn wasm_artifact_rejects_tampered_abi_version_before_run() {
    let dir = temp_dir("wasm_abi_tamper");
    let source = write_scalar_program(&dir);
    let artifact = dir.join("app.wasm");
    let build = source_sgc_command()
        .args([
            "build",
            source.to_str().unwrap(),
            "--target",
            "wasm",
            "--output",
            artifact.to_str().unwrap(),
        ])
        .output()
        .expect("build wasm for ABI tamper");
    assert!(
        build.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let mut bytes = fs::read(&artifact).expect("read wasm artifact");
    // Custom ABI section payload is near the start; flip MIR semantic ABI LEB
    // from 1 -> 2 while keeping the module otherwise well-formed.
    let name = b"sengoo.portable_runtime_abi";
    let name_pos = bytes
        .windows(name.len())
        .position(|w| w == name)
        .expect("failed to locate ABI custom section name");
    let version_pos = name_pos + name.len();
    assert_eq!(
        bytes.get(version_pos).copied(),
        Some(1),
        "expected MIR ABI version byte 1 after custom section name"
    );
    bytes[version_pos] = 2;
    fs::write(&artifact, &bytes).unwrap();

    let run = source_sgc_command()
        .args(["run", artifact.to_str().unwrap(), "--target", "wasm"])
        .output()
        .expect("run tampered wasm");
    assert!(
        !run.status.success(),
        "tampered ABI must fail closed before execution"
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("unsupported-mir-semantic-abi")
            || stderr.contains("unsupported-portable-runtime-abi"),
        "stderr:\n{stderr}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn wasm_target_uses_wasm32_pointer_sized_literal_bounds() {
    let dir = temp_dir("wasm32_usize_bounds");
    let source = dir.join("main.sg");
    fs::write(&source, "def main() -> usize { 4294967296usize }\n").unwrap();

    let build = source_sgc_command()
        .args(["build", source.to_str().unwrap(), "--target", "wasm"])
        .output()
        .expect("build wasm32 usize boundary");

    assert!(
        !build.status.success(),
        "2^32 must not type-check as wasm32 usize"
    );
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        stderr.contains("exceeds range of `usize`"),
        "stderr:\n{stderr}"
    );
    let _ = fs::remove_dir_all(dir);
}
