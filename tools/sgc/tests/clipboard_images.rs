//! Clipboard fixtures never inspect or change the interactive clipboard.
mod common;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

fn clang() -> PathBuf {
    if let Some(path) = std::env::var_os("SENGOO_CLANG") {
        return path.into();
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
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
fn work() -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "sengoo-clipboard-images-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    directory
}
fn passed(output: Output, label: &str) {
    assert!(
        output.status.success(),
        "{label}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[cfg(windows)]
fn isolated_clipboard_with_real_image_conversion() {
    let directory = work();
    let binary = directory.join("clipboard-images.exe");
    passed(
        Command::new(clang())
            .args(["-std=c11", "-O1", "-Wno-unused-function"])
            .arg(repository().join("tools/stdlib/tests/clipboard_image_probe.c"))
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap(),
        "compile image fixture",
    );
    let output = directory.join("owned-output.png");
    passed(
        Command::new(&binary)
            .arg(&output)
            .stdin(Stdio::null())
            .output()
            .unwrap(),
        "isolated image/text/files fixture",
    );
    assert!(
        !output.exists(),
        "fixture must clean its exact generated output"
    );
}

#[test]
fn public_clipboard_types_and_rejected_path() {
    let directory = work();
    let binary = directory.join(if cfg!(windows) {
        "clipboard-api.exe"
    } else {
        "clipboard-api"
    });
    let compiler = clang();
    let mut paths = compiler
        .parent()
        .into_iter()
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    passed(
        common::source_sgc_command()
            .arg("build")
            .arg(repository().join("tools/stdlib/tests/clipboard_images.sg"))
            .arg("-o")
            .arg(&binary)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .output()
            .unwrap(),
        "compile public clipboard API",
    );
    passed(
        Command::new(&binary).stdin(Stdio::null()).output().unwrap(),
        "reject NUL before clipboard access",
    );
}
