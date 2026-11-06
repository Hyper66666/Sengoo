//! The opt-in large Base64 helper must not silently widen the legacy codec.
mod common;

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn public_bounded_base64_preserves_binary_and_legacy_boundaries() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = std::env::temp_dir().join(format!(
        "sengoo-encoding-bounded-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&directory).unwrap();
    let source = directory.join("main.sg");
    fs::copy(
        repository.join("tools/stdlib/tests/encoding_bounded.sg"),
        &source,
    )
    .unwrap();
    let binary = directory.join(if cfg!(windows) { "main.exe" } else { "main" });
    let mut compiler = common::source_sgc_command();
    compiler
        .args(["build", "--force-rebuild", "-O", "0"])
        .arg(&source)
        .arg("--output")
        .arg(&binary);
    if let Some(clang) = std::env::var_os("SENGOO_CLANG") {
        let clang = PathBuf::from(clang);
        let mut paths = clang
            .parent()
            .map(Path::to_path_buf)
            .into_iter()
            .collect::<Vec<_>>();
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        compiler.env("PATH", std::env::join_paths(paths).unwrap());
    }
    let built = compiler.output().expect("Sengoo compiler is required");
    assert!(
        built.status.success(),
        "compile bounded Base64: {}\n{}\n{}",
        built.status,
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr)
    );
    let result = Command::new(&binary).output().unwrap();
    assert!(
        result.status.success(),
        "bounded Base64: {}\n{}\n{}",
        result.status,
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
