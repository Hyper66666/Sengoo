//! The OS argument boundary must preserve UTF-8 independently of Windows ACP.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn arguments_preserve_unicode_quotes_empty_and_trailing_slashes() {
    let directory = std::env::temp_dir().join(format!(
        "sengoo-args-目录-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&directory).unwrap();
    let source = directory.join("main.sg");
    fs::write(
        &source,
        r#"
import std::args;
import std::process;
import std::dir;
import std::env;
def verify(index: i64, expected: &str) -> i64 {
    let size = arg_len(index).unwrap_or(-1);
    if size != expected.len() { return 1; }
    let buffer = ffi_buffer_new(size + 1);
    if !buffer.is_ok { return 2; }
    let copied = arg_copy(index, buffer.value).unwrap_or(-1);
    if copied != size { return 3; }
    if buffer.value.used_len() != copied { return 6; }
    let text = string_from_buffer(buffer.value, copied);
    if !text.is_ok { return 4; }
    if text.value.as_str() != expected { return 5; }
    return 0;
}
def main() -> i64 {
    if args_len() != 6 { return 10; }
    if verify(0, "世界 with spaces") != 0 { return 11; }
    if verify(1, "literal\"quote") != 0 { return 12; }
    if verify(2, "tail\\") != 0 { return 13; }
    if verify(3, "") != 0 { return 14; }
    if verify(4, "--looks-like-an-option") != 0 { return 15; }
    if verify(5, "emoji😀") != 0 { return 16; }
    if arg_exists(-1) or arg_exists(6) { return 17; }
    if arg_len(-1).is_ok or arg_len(9223372036854775807).is_ok { return 18; }
    let directory_length = process_current_dir_len().unwrap_or(-1);
    if directory_length < 1 { return 19; }
    let directory_buffer = ffi_buffer_new(directory_length + 1);
    if !directory_buffer.is_ok { return 20; }
    let copied_directory = process_current_dir_copy(directory_buffer.value).unwrap_or(-1);
    if copied_directory != directory_length { return 21; }
    let directory_text = string_from_bytes_raw(directory_buffer.value.ptr(), copied_directory);
    if !directory_text.is_ok { return 22; }
    if str_index_of(directory_text.value.as_str(), "sengoo-args-目录-") < 0 { return 23; }
    if !dir_exists(directory_text.value.as_str()) { return 24; }
    if !dir_create_all("嵌套/目录").is_ok() { return 25; }
    if !dir_exists("嵌套/目录") { return 26; }
    let environment_buffer = ffi_buffer_new(256);
    if !environment_buffer.is_ok { return 27; }
    let environment_length = env_var_copy("SENGOO_ARGS_UNICODE_ENV", environment_buffer.value).unwrap_or(-1);
    if environment_length != 18 { return 28; }
    if environment_buffer.value.used_len() != environment_length { return 29; }
    let environment_value = string_from_buffer(environment_buffer.value, environment_length);
    if !environment_value.is_ok { return 30; }
    if environment_value.value.as_str() != "managed 世界😀" { return 31; }
    let empty_buffer = ffi_buffer_new(4);
    if !empty_buffer.is_ok { return 32; }
    let empty_length = env_var_copy("SENGOO_ARGS_EMPTY_ENV", empty_buffer.value).unwrap_or(-1);
    if empty_length != 0 or empty_buffer.value.used_len() != 0 { return 33; }
    let empty_value = string_from_buffer(empty_buffer.value, 0);
    if !empty_value.is_ok or empty_value.value.len() != 0 { return 34; }
    print("UTF-8 arguments passed");
    return 0;
}
"#,
    )
    .unwrap();
    let binary = directory.join(if cfg!(windows) { "args.exe" } else { "args" });
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let clang: PathBuf = std::env::var_os("SENGOO_CLANG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                    let path = PathBuf::from(local).join(
                        "SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe",
                    );
                    if path.is_file() {
                        return path;
                    }
                }
            }
            "clang".into()
        });
    let mut tool_paths: Vec<PathBuf> = clang
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .into_iter()
        .map(Path::to_path_buf)
        .collect();
    tool_paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let built = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .current_dir(&repo)
        .args(["--runtime-mode", "source-development", "build"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .env("SENGOO_CLANG", &clang)
        .env("PATH", std::env::join_paths(tool_paths).unwrap())
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "compile:\n{}\n{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr)
    );
    for _ in 0..3 {
        let output = Command::new(&binary)
            .current_dir(&directory)
            .env("SENGOO_ARGS_UNICODE_ENV", "managed 世界😀")
            .env("SENGOO_ARGS_EMPTY_ENV", "")
            .args([
                "世界 with spaces",
                "literal\"quote",
                "tail\\",
                "",
                "--looks-like-an-option",
                "emoji😀",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "argument program: {:?}\n{}\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "UTF-8 arguments passed"
        );
    }
}
