use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root_from_manifest_dir() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
}

pub(crate) fn find_runtime_c() -> Option<String> {
    if let Ok(path) = std::env::var("SENGOO_RUNTIME") {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }

    if let Some(workspace_root) = workspace_root_from_manifest_dir() {
        let candidate = workspace_root.join("tools").join("stdlib").join("runtime.c");
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = exe.parent().unwrap_or(Path::new("."));

        let candidate = exe_dir.join("runtime.c");
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }

        if let Some(parent) = exe_dir.parent() {
            if let Some(grandparent) = parent.parent() {
                let candidate = grandparent.join("tools").join("stdlib").join("runtime.c");
                if candidate.exists() {
                    return Some(candidate.to_string_lossy().to_string());
                }
            }
        }
    }

    let candidate = Path::new("tools/stdlib/runtime.c");
    if candidate.exists() {
        return Some(candidate.to_string_lossy().to_string());
    }

    None
}

fn find_tool(tool: &str, windows_candidates: &[&str], unix_candidates: &[&str]) -> Option<String> {
    if std::env::consts::OS == "windows" {
        for path in windows_candidates {
            if Path::new(path).exists() {
                return Some((*path).to_string());
            }
        }

        let exe_name = format!("{}.exe", tool);
        if let Ok(output) = Command::new("where").arg(&exe_name).output() {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    return path.lines().next().map(|s| s.trim().to_string());
                }
            }
        }
    } else {
        for path in unix_candidates {
            if Path::new(path).exists() {
                return Some((*path).to_string());
            }
        }

        if let Ok(output) = Command::new("which").arg(tool).output() {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    return path.lines().next().map(|s| s.trim().to_string());
                }
            }
        }
    }

    None
}

pub(crate) fn find_clang() -> Option<String> {
    find_tool(
        "clang",
        &[
            "C:\\Program Files\\LLVM\\bin\\clang.exe",
            "C:\\Program Files (x86)\\LLVM\\bin\\clang.exe",
            "clang.exe",
            "clang",
        ],
        &["clang", "/usr/bin/clang", "/usr/local/bin/clang"],
    )
}

pub(crate) fn find_lli() -> Option<String> {
    find_tool(
        "lli",
        &[
            "C:\\Program Files\\LLVM\\bin\\lli.exe",
            "C:\\Program Files (x86)\\LLVM\\bin\\lli.exe",
            "lli.exe",
            "lli",
        ],
        &["lli", "/usr/bin/lli", "/usr/local/bin/lli"],
    )
}
