//! A self-hosted executable must be locatable after cwd/PATH changes.
use std::{fs, path::{Path, PathBuf}, process::Command, time::{SystemTime, UNIX_EPOCH}};

#[test]
fn running_image_preserves_unicode_and_ignores_cwd_and_path() {
    let scratch = std::env::temp_dir().join(format!("sengoo-image-目录-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(scratch.join("other-cwd")).unwrap();
    let source = scratch.join("main.sg");
    fs::write(&source, r#"
import std::system;
import std::io;
def main() -> i64 {
    let image = current_executable_string();
    if not image.is_ok() { return 1; };
    io_stdout_write(image.value.as_str()).unwrap_or(0);
    0
}
"#).unwrap();
    let binary = scratch.join(if cfg!(windows) { "自启 image.exe" } else { "自启 image" });
    let clang: PathBuf = std::env::var_os("SENGOO_CLANG").map(PathBuf::from).unwrap_or_else(|| {
        if cfg!(windows) {
            if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                let value = PathBuf::from(local).join("SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe");
                if value.is_file() { return value; }
            }
        }
        "clang".into()
    });
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let built = Command::new(env!("CARGO_BIN_EXE_sgc")).current_dir(repo)
        .args(["--runtime-mode", "source-development", "build"]).arg(&source)
        .arg("-o").arg(&binary).env("SENGOO_CLANG", clang).output().unwrap();
    assert!(built.status.success(), "{}\n{}", String::from_utf8_lossy(&built.stdout), String::from_utf8_lossy(&built.stderr));
    for _ in 0..3 {
        let ran = Command::new(&binary).current_dir(scratch.join("other-cwd")).env("PATH", "").output().unwrap();
        assert!(ran.status.success(), "{:?}: {}", ran.status, String::from_utf8_lossy(&ran.stderr));
        let reported = String::from_utf8(ran.stdout).unwrap();
        assert_eq!(fs::canonicalize(reported).unwrap(), fs::canonicalize(&binary).unwrap());
    }
}
