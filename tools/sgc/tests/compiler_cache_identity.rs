//! Compiler replacement must invalidate every persisted reuse layer.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn run(binary: &Path, source: &Path) -> String {
    let output = Command::new(binary)
        .args([
            "--runtime-mode",
            "source-development",
            "build",
            "--emit-llvm",
        ])
        .arg(source)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn identities(directory: &Path) -> Vec<String> {
    [
        "build/main.build-cache.json",
        "build/workset/main.frontend-session-v4.json",
        "build/workset/main.generic-instance-cache.json",
    ]
    .iter()
    .map(|file| {
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join(file)).unwrap()).unwrap();
        value["compiler_identity"].as_str().unwrap().to_owned()
    })
    .collect()
}
#[test]
fn copied_compiler_reuses_but_replaced_same_version_compiler_rebuilds() {
    let directory = std::env::temp_dir().join(format!(
        "sengoo-compiler-identity-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&directory).unwrap();
    let source = directory.join("main.sg");
    fs::write(&source, "def main() -> i64 { 0 }\n").unwrap();
    let original = PathBuf::from(env!("CARGO_BIN_EXE_sgc"));
    let moved = directory.join(if cfg!(windows) {
        "sgc-copy.exe"
    } else {
        "sgc-copy"
    });
    fs::copy(&original, &moved).unwrap();
    run(&original, &source);
    let old = identities(&directory);
    assert!(old.iter().all(|identity| identity == &old[0]));
    let unchanged = run(&moved, &source);
    assert!(unchanged.contains("build cache hit"), "{unchanged}");
    assert_eq!(old, identities(&directory));
    // Executable overlays do not alter behavior but do alter the compiler's
    // content identity. Re-sign the modified Mach-O fixture on macOS.
    OpenOptions::new()
        .append(true)
        .open(&moved)
        .unwrap()
        .write_all(b"\nSengoo cache identity regression fixture\n")
        .unwrap();
    #[cfg(target_os = "macos")]
    assert!(Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(&moved)
        .status()
        .unwrap()
        .success());
    let changed = run(&moved, &source);
    assert!(!changed.contains("build cache hit"), "{changed}");
    assert!(
        changed.contains("frontend session: reused_modules=0 rebuilt_modules=1"),
        "{changed}"
    );
    let new = identities(&directory);
    assert_ne!(old[0], new[0]);
    assert!(new.iter().all(|identity| identity == &new[0]));
    assert!(run(&moved, &source).contains("build cache hit"));
}
