//! Exercise the synchronous file operations used by the native CLI.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn unicode_files_read_replace_copy_walk_and_remove() {
    let directory = std::env::temp_dir().join(format!(
        "sengoo-fs-项目😀-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&directory).unwrap();
    let source = directory.join("main.sg");
    fs::write(&source, r#"
import std::file;
import std::dir;
import std::string;

def read_text(path: &str) -> Result<String, i64> {
    let size = file_size(path)?;
    let buffer = ffi_buffer_new(size + 1)?;
    let count = file_read_into(path, buffer)?;
    if count != size or buffer.used_len() != size { return string_result_err(101); }
    string_from_buffer(buffer, count)
}
def exercise() -> Result<i64, i64> {
    let root = "项目😀/子目录";
    let original = "项目😀/子目录/数据🌏.txt";
    let temporary = "项目😀/子目录/临时😀.tmp";
    let copied = "项目😀/子目录/副本🌏.txt";
    let before = "第一行 世界😀\nneedle before\n";
    let after = "第一行 世界😀\nneedle after\n";
    dir_create_all(root)?;
    if !dir_exists(root) { return Result { is_ok: false, value: 0, error: 102 }; }
    if file_write_str(original, before)? != before.len() { return Result { is_ok: false, value: 0, error: 103 }; }
    if !file_exists(original) or file_kind(original)? != PATH_KIND_FILE() { return Result { is_ok: false, value: 0, error: 104 }; }
    if file_size(original)? != before.len() or file_len(original)? != before.len() { return Result { is_ok: false, value: 0, error: 105 }; }
    if file_modified_unix_ms(original)? < 1 { return Result { is_ok: false, value: 0, error: 106 }; }
    let first = read_text(original)?;
    if first.as_str() != before { return Result { is_ok: false, value: 0, error: 107 }; }
    // The CLI patches into an adjacent temporary file and atomically replaces.
    file_write_str(temporary, after)?;
    file_move(temporary, original, true)?;
    if file_exists(temporary) { return Result { is_ok: false, value: 0, error: 108 }; }
    let replaced = read_text(original)?;
    if replaced.as_str() != after { return Result { is_ok: false, value: 0, error: 109 }; }
    if file_copy(original, copied, false)? != after.len() { return Result { is_ok: false, value: 0, error: 110 }; }
    if file_copy(original, copied, false).error != 6 { return Result { is_ok: false, value: 0, error: 111 }; }
    if file_copy(original, original, true).error != 2 { return Result { is_ok: false, value: 0, error: 112 }; }
    if file_move(copied, original, false).error != 6 { return Result { is_ok: false, value: 0, error: 113 }; }
    file_append_str(copied, "追加😀")?;
    let appended = read_text(copied)?;
    if str_index_of(appended.as_str(), "追加😀") < 0 { return Result { is_ok: false, value: 0, error: 114 }; }
    if dir_entry_count(root)? != 2 { return Result { is_ok: false, value: 0, error: 115 }; }
    let name = dir_entry_name_string(root, 0)?;
    if str_index_of(name.as_str(), "🌏") < 0 { return Result { is_ok: false, value: 0, error: 116 }; }
    let walk = dir_walk("项目😀", 4)?;
    let mut found = false;
    let mut count = 0;
    loop {
        let item = walk.next_string();
        if !item.is_ok() { break; }
        if str_index_of(item.value.as_str(), "数据🌏.txt") >= 0 { found = true; }
        count = count + 1;
    }
    if !found or count != 3 { return Result { is_ok: false, value: 0, error: 117 }; }
    if !file_remove(copied) or !file_remove(original) { return Result { is_ok: false, value: 0, error: 118 }; }
    dir_remove(root)?;
    dir_remove("项目😀")?;
    if dir_exists("项目😀") { return Result { is_ok: false, value: 0, error: 119 }; }
    return Result { is_ok: true, value: 0, error: 0 };
}
def main() -> i64 {
    INVALID_UTF8_CHECKS
    let result = exercise();
    if !result.is_ok() { print(result.error); return 1; }
    print("Unicode filesystem passed");
    return 0;
}
"#.replace("INVALID_UTF8_CHECKS", if cfg!(windows) { r#"
    let invalid = ffi_buffer_new(2).unwrap_or(Buffer { handle: 0 });
    invalid.push_u8(255);
    invalid.push_u8(0);
    if file_len_raw(invalid.ptr()).error != 20 { return 40; }
    if file_kind_raw(invalid.ptr()).error != 20 { return 41; }
    if file_modified_unix_ms_raw(invalid.ptr()).error != 20 { return 42; }
    if sengoo_file_write_str(invalid.ptr(), sengoo_stdlib_str_ptr("x"), 1) != -20 { return 43; }
    if sengoo_file_read_into(invalid.ptr(), invalid.ptr(), 2) != -20 { return 44; }
    if sengoo_file_copy(invalid.ptr(), sengoo_stdlib_str_ptr("uncreated.txt"), 0) != -20 { return 45; }
    if sengoo_file_move(invalid.ptr(), sengoo_stdlib_str_ptr("uncreated.txt"), 0) != -20 { return 46; }
    if sengoo_dir_create_all(invalid.ptr()) != -20 { return 47; }
    if sengoo_dir_entry_count(invalid.ptr()) != -20 { return 48; }
    if sengoo_dir_walk_new(invalid.ptr(), 4) != -20 { return 49; }
    if sengoo_dir_remove(invalid.ptr()) != -20 { return 50; }
    if sengoo_file_exists(invalid.ptr()) != 0 or sengoo_dir_exists(invalid.ptr()) != 0 { return 51; }
    if sengoo_file_remove(invalid.ptr()) == 0 { return 52; }
    if sengoo_dir_create(invalid.ptr()) != -20 { return 53; }
"# } else { "" })).unwrap();
    let binary = directory.join(if cfg!(windows) {
        "filesystem.exe"
    } else {
        "filesystem"
    });
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
    let mut paths: Vec<PathBuf> = clang
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .into_iter()
        .map(Path::to_path_buf)
        .collect();
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .args(["--runtime-mode", "source-development", "build"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .current_dir(&repo)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "compile:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for _ in 0..2 {
        let result = Command::new(&binary)
            .current_dir(&directory)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "filesystem {:?}:\n{}\n{}",
            result.status.code(),
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&result.stdout).trim(),
            "Unicode filesystem passed"
        );
    }
}
