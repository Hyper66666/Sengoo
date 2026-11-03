//! Read-only environment diagnosis. Probes and their artifacts live in a fresh
//! temporary directory; dependency installation is always the user's choice.
use clap::ValueEnum;
use miette::{IntoDiagnostic, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::installed_runtime::{native_runtime_mode, NativeRuntimeMode};
use crate::native_toolchain::{
    ensure_supported_clang_toolchain, validate_runtime_abi_compatibility,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DoctorFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DoctorProfile {
    Native,
    Python,
}

#[derive(Debug, Serialize)]
struct Check {
    id: String,
    ok: bool,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fix: Option<String>,
}

impl Check {
    fn result(id: &str, result: std::result::Result<String, String>, fix: &str) -> Self {
        match result {
            Ok(detail) => Self {
                id: id.into(),
                ok: true,
                detail,
                fix: None,
            },
            Err(detail) => Self {
                id: id.into(),
                ok: false,
                detail,
                fix: Some(fix.into()),
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: u32,
    profile: DoctorProfile,
    target: String,
    toolchain: String,
    runtime_abi: u32,
    ok: bool,
    checks: Vec<Check>,
}

fn tool_path(name: &str, current: &Path) -> std::result::Result<PathBuf, String> {
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    let sibling = current.parent().unwrap_or(Path::new(".")).join(&file);
    if sibling.is_file() {
        return Ok(sibling);
    }
    which::which(name).map_err(|error| format!("{name} is missing: {error}"))
}

fn tool_check(name: &str, current: &Path) -> std::result::Result<String, String> {
    let path = tool_path(name, current)?;
    let output = Command::new(&path)
        .arg("--version")
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = format!("{name} {}", crate::cli::SGC_VERSION);
    if !output.status.success() || stdout.trim() != expected {
        return Err(format!(
            "{} reports {:?}; expected {expected:?}",
            path.display(),
            stdout.trim()
        ));
    }
    Ok(format!("{}: {}", path.display(), stdout.trim()))
}

struct ProbeDirectory(PathBuf);
impl ProbeDirectory {
    fn new() -> std::result::Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("sengoo-doctor-{}-{stamp}", std::process::id()));
        fs::create_dir(&path).map_err(|e| e.to_string())?;
        Ok(Self(path))
    }
}
impl Drop for ProbeDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn native_probe(current: &Path) -> std::result::Result<String, String> {
    let directory = ProbeDirectory::new()?;
    let source = directory.0.join("probe.sg");
    let executable = directory
        .0
        .join(format!("probe{}", std::env::consts::EXE_SUFFIX));
    fs::write(
        &source,
        "def main() -> i64 {\n    print(\"sengoo-doctor-ok\");\n    0;\n}\n",
    )
    .map_err(|e| e.to_string())?;
    let mode = match native_runtime_mode() {
        NativeRuntimeMode::Installed => "installed",
        NativeRuntimeMode::SourceDevelopment => "source-development",
    };
    let output = Command::new(current)
        .args(["--runtime-mode", mode, "build"])
        .arg(&source)
        .args(["-O", "0", "-o"])
        .arg(&executable)
        .current_dir(&directory.0)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "minimal Sengoo compile/link failed ({}):\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let output = Command::new(&executable)
        .current_dir(&directory.0)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success()
        || !String::from_utf8_lossy(&output.stdout).contains("sengoo-doctor-ok")
    {
        return Err(format!(
            "linked probe did not run successfully: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(format!(
        "compiled, linked and ran a temporary native program ({mode} runtime)"
    ))
}

fn python_check() -> std::result::Result<String, String> {
    let python = std::env::var_os("SENGOO_PYTHON")
        .map(PathBuf::from)
        .or_else(|| which::which("python3").ok())
        .or_else(|| which::which("python").ok())
        .ok_or_else(|| "CPython was not found; set SENGOO_PYTHON to its executable".to_string())?;
    let script = "import sys, platform; assert platform.python_implementation() == 'CPython', 'CPython required'; assert (3,11) <= sys.version_info[:2] <= (3,14), 'supported versions: 3.11-3.14'; import sengoo_python; print(sys.version.split()[0] + ' / sengoo-python backend available')";
    let output = Command::new(&python)
        .args(["-c", script])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "{}: {}",
            python.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(format!(
        "{}: {}",
        python.display(),
        String::from_utf8_lossy(&output.stdout).trim()
    ))
}

pub(crate) fn cmd_doctor(format: DoctorFormat, profile: DoctorProfile) -> Result<()> {
    let current = std::env::current_exe().into_diagnostic()?;
    let mut checks = vec![];
    for name in ["sgc", "sgpm", "sgfmt", "sglsp"] {
        checks.push(Check::result(
            name,
            tool_check(name, &current),
            "Install all four tools from the same Sengoo archive; put its bin directory on PATH.",
        ));
    }
    let stdlib = crate::find_stdlib_root()
        .ok_or_else(|| "standard library not found".to_string())
        .and_then(|root| {
            for file in [
                "string.sg",
                "collections.sg",
                "option.sg",
                "result.sg",
                "assert.sg",
            ] {
                if !root.join(file).is_file() {
                    return Err(format!("missing {}", root.join(file).display()));
                }
            }
            Ok(root.display().to_string())
        });
    checks.push(Check::result("stdlib", stdlib, "Reinstall the complete toolchain including share/sengoo; source developers may set SENGOO_STDLIB."));
    let runtime = (|| {
        crate::installed_runtime::validate_native_runtime_mode_environment()
            .map_err(|e| e.to_string())?;
        let runtime_c =
            crate::find_runtime_c().ok_or_else(|| "C runtime sources not found".to_string())?;
        validate_runtime_abi_compatibility(Path::new(&runtime_c)).map_err(|e| e.to_string())?;
        if native_runtime_mode() == NativeRuntimeMode::Installed {
            crate::installed_runtime::validate_installed_native_runtime_for_host()
                .map_err(|e| e.to_string())?;
        }
        Ok(format!(
            "ABI {}: {} ({:?})",
            crate::native_toolchain::SENGOO_RUNTIME_ABI_VERSION,
            runtime_c,
            native_runtime_mode()
        ))
    })();
    checks.push(Check::result("runtime", runtime, "Reinstall matching runtime payloads. In a source checkout, use --runtime-mode source-development."));
    let clang = crate::find_clang()
        .ok_or_else(|| "clang not found".to_string())
        .and_then(|path| {
            ensure_supported_clang_toolchain(&path).map_err(|e| e.to_string())?;
            Ok(path)
        });
    checks.push(Check::result(
        "clang",
        clang,
        "Install LLVM/clang 15 or newer and add its bin directory to PATH (CI uses LLVM 19).",
    ));
    #[cfg(windows)]
    {
        let linker = crate::native_toolchain::find_msvc_link_exe()
            .ok_or_else(|| "MSVC linker not found".to_string())
            .and_then(|path| {
                let libraries = crate::native_toolchain::windows_link_lib_paths();
                for name in ["kernel32.lib", "ucrt.lib", "vcruntime.lib"] {
                    if !libraries.iter().any(|dir| dir.join(name).is_file()) {
                        return Err(format!("Windows SDK/MSVC library {name} not found"));
                    }
                }
                Ok(path.display().to_string())
            });
        checks.push(Check::result("system-linker", linker, "Install Visual Studio Build Tools with Desktop development with C++ and a Windows SDK."));
    }
    let ready = checks
        .iter()
        .filter(|c| matches!(c.id.as_str(), "runtime" | "clang" | "system-linker"))
        .all(|c| c.ok);
    checks.push(Check::result("native-probe", if ready { native_probe(&current) } else { Err("probe could not run because required native environment checks failed".into()) },
        "Fix the reported compiler/runtime/linker errors; macOS needs Xcode Command Line Tools, Linux needs the C compiler and development libc, Windows needs MSVC and Windows SDK."));
    if profile == DoctorProfile::Python {
        checks.push(Check::result("python-backend", python_check(), "Use CPython 3.11-3.14 and install sengoo-python into that environment. Set SENGOO_PYTHON to select it."));
    }
    let report = Report {
        schema_version: 1,
        profile,
        target: crate::NativeBuildTarget::host().triple,
        toolchain: crate::cli::SGC_VERSION.into(),
        runtime_abi: crate::native_toolchain::SENGOO_RUNTIME_ABI_VERSION,
        ok: checks.iter().all(|check| check.ok),
        checks,
    };
    match format {
        DoctorFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&report).into_diagnostic()?
        ),
        DoctorFormat::Text => {
            println!(
                "Sengoo doctor schema v1 / {} / {:?}",
                report.target, profile
            );
            for check in &report.checks {
                println!(
                    "[{}] {}: {}",
                    if check.ok { "ok" } else { "FAIL" },
                    check.id,
                    check.detail
                );
                if let Some(fix) = &check.fix {
                    println!("  Fix: {fix}");
                }
            }
        }
    }
    if report.ok {
        Ok(())
    } else {
        Err(miette::miette!(
            "environment checks failed; see doctor report"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_reports_have_actionable_fixes_and_stable_schema() {
        let check = Check::result("clang", Err("not found".into()), "install LLVM");
        let report = Report {
            schema_version: 1,
            profile: DoctorProfile::Native,
            target: "test".into(),
            toolchain: "0.2.0".into(),
            runtime_abi: 1,
            ok: false,
            checks: vec![check],
        };
        let value = serde_json::to_value(report).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["checks"][0]["fix"], "install LLVM");
        assert_eq!(value["ok"], false);
    }
}
