//! Bounded-memory file SHA-256 for artifacts above the old 64-MiB buffer cap.
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
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
        "sengoo-filehash-{}-{}",
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
        "{label} ({}):\n{}\n{}",
        o.status,
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
}
#[test]
fn hash_file_native_failure_and_unicode_boundaries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = work();
    // Compile the actual shared transform, avoiding either a duplicate crypto
    // implementation in the probe or unrelated breadth bridge dependencies.
    let breadth = fs::read_to_string(root.join("tools/stdlib/runtime_breadth.c")).unwrap();
    let start = breadth
        .find("static const uint32_t sengoo_sha256_k[64]")
        .unwrap();
    let end = breadth.find("long long sengoo_hash_sha256(").unwrap();
    let core = directory.join("sha_core.c");
    fs::write(
        &core,
        format!("#include <stdint.h>\n{}", &breadth[start..end]),
    )
    .unwrap();
    let binary = directory.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    passed(
        Command::new(clang())
            .args([
                "-std=c11",
                "-O1",
                "-Wall",
                "-Wextra",
                "-Wno-unused-function",
            ])
            .arg(root.join("tools/stdlib/tests/hash_file_probe.c"))
            .arg(&core)
            .arg("-o")
            .arg(&binary)
            .output()
            .expect("clang required"),
        "compile file hash probe",
    );
    passed(
        Command::new(&binary).output().unwrap(),
        "native file hash error/Unicode probe",
    );
}
#[test]
fn public_file_hash_streams_large_artifact() {
    let directory = work();
    let payload = directory.join("哈希-😀.bin");
    let mut file = fs::File::create(&payload).unwrap();
    let mut expected = Sha256::new();
    let chunk = (0..65536)
        .map(|i| ((i * 17 + i / 13) & 255) as u8)
        .collect::<Vec<_>>();
    for _ in 0..2064 {
        file.write_all(&chunk).unwrap();
        expected.update(&chunk);
    }
    file.write_all(&chunk[..17]).unwrap();
    expected.update(&chunk[..17]);
    drop(file);
    let size = fs::metadata(&payload).unwrap().len();
    assert!(size > 128 * 1024 * 1024);
    let digest = format!("{:x}", expected.finalize());
    let source = directory.join("main.sg");
    fs::write(
        &source,
        r#"
import std::hash;
def main() -> i64 {
    let digest = hash_sha256_file("PAYLOAD", MAX_SIZE);
    if !digest.is_ok || digest.value.len() != 64 { return 1; }
    if digest.value.as_str() != "EXPECTED" { return 2; }
    let capped = hash_sha256_file("PAYLOAD", MAX_SIZE - 1);
    if capped.is_ok || capped.error != 13 { return 3; }
    let old = ffi_buffer_new(64);
    if !old.is_ok || !hash_sha256_str("abc", old.value).is_ok { return 4; }
    return 0;
}
"#
        .replace("PAYLOAD", &payload.to_string_lossy().replace('\\', "/"))
        .replace("MAX_SIZE", &size.to_string())
        .replace("EXPECTED", &digest),
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
        "compile public bounded file hash",
    );
    passed(
        Command::new(&binary).output().unwrap(),
        "large file hash and legacy API",
    );
    fs::remove_file(payload).unwrap();
}
