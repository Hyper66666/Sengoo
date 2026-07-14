use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn generation_handle_encoding_stays_positive_and_signals_exhaustion() {
    let Some(clang) = which::which("clang").ok() else {
        eprintln!("skipping generation-handle probe: clang not found");
        return;
    };
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "sengoo-generation-handle-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create generation-handle probe directory");
    let source = root.join("probe.c");
    let executable = root.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    fs::write(
        &source,
        r#"
#include "runtime_shared.h"
#include <stdint.h>

int main(void) {
    if (SENGOO_RUNTIME_HANDLE_GENERATION_MAX != UINT32_C(0x7fffffff)) return 1;
    if (sengoo_runtime_next_handle_generation(0) != 1) return 2;
    if (sengoo_runtime_next_handle_generation(1) != 2) return 3;
    if (sengoo_runtime_next_handle_generation(UINT32_C(0x7ffffffe)) != UINT32_C(0x7fffffff)) return 4;
    if (sengoo_runtime_next_handle_generation(UINT32_C(0x7fffffff)) != 0) return 5;
    if (sengoo_runtime_encode_handle(1, 0) != INT64_C(0x0000000100000001)) return 6;
    if (sengoo_runtime_encode_handle(UINT32_C(0x7fffffff), (size_t)UINT32_MAX - 1) != INT64_MAX) return 7;
    if (sengoo_runtime_encode_handle(0, 0) != 0) return 8;
    if (sengoo_runtime_encode_handle(UINT32_C(0x80000000), 0) != 0) return 9;
    if (sengoo_runtime_encode_handle(1, (size_t)UINT32_MAX) != 0) return 10;
    return 0;
}
"#,
    )
    .expect("write generation-handle probe");

    let stdlib = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("stdlib");
    let compile = Command::new(clang)
        .arg("-std=c11")
        .arg("-I")
        .arg(&stdlib)
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("clang should compile generation-handle probe");
    assert!(
        compile.status.success(),
        "generation-handle probe failed to compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );

    let output = Command::new(&executable)
        .output()
        .expect("generation-handle probe should run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "generation-handle probe exited with {:?}",
        output.status.code()
    );
    assert!(root.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(root);
}
