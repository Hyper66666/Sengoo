use miette::Result;
use std::path::Path;

pub const CROSS_COMPILATION_DOC: &str = "docs/cross-compilation.md";

pub const REFERENCE_TARGET_WINDOWS_MSVC: &str = "x86_64-pc-windows-msvc";
pub const REFERENCE_TARGET_LINUX_GNU: &str = "x86_64-unknown-linux-gnu";

pub const REFERENCE_TARGETS: &[&str] = &[REFERENCE_TARGET_WINDOWS_MSVC, REFERENCE_TARGET_LINUX_GNU];

fn host_triple_for(target_os: &str, target_arch: &str) -> &'static str {
    if target_os == "windows" {
        REFERENCE_TARGET_WINDOWS_MSVC
    } else if target_os == "macos" {
        // Keep the host triple aligned with packaged/installed distribution
        // targets (`*-apple-darwin`) so installed sgc resolves the native
        // runtime without alias mismatches against `*-apple-macosx`.
        if target_arch == "aarch64" {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        }
    } else {
        REFERENCE_TARGET_LINUX_GNU
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeBuildTarget {
    pub(crate) triple: String,
}

impl NativeBuildTarget {
    pub(crate) fn host() -> Self {
        Self {
            triple: host_triple().to_string(),
        }
    }

    pub(crate) fn resolve(cli_target: Option<&str>) -> Result<Self> {
        match cli_target {
            None => Ok(Self::host()),
            Some(triple) => {
                if !REFERENCE_TARGETS.contains(&triple) {
                    return Err(miette::miette!(
                        "unsupported target triple '{triple}'; supported reference triples are {} and {}; see {CROSS_COMPILATION_DOC}",
                        REFERENCE_TARGET_WINDOWS_MSVC,
                        REFERENCE_TARGET_LINUX_GNU
                    ));
                }
                Ok(Self {
                    triple: triple.to_string(),
                })
            }
        }
    }

    pub(crate) fn is_cross(&self) -> bool {
        self.triple != host_triple()
    }

    pub(crate) fn is_windows_msvc(&self) -> bool {
        self.triple == REFERENCE_TARGET_WINDOWS_MSVC
    }

    pub(crate) fn is_linux_gnu(&self) -> bool {
        self.triple == REFERENCE_TARGET_LINUX_GNU
    }

    pub(crate) fn object_extension(&self) -> &'static str {
        if self.is_windows_msvc() {
            "obj"
        } else {
            "o"
        }
    }

    pub(crate) fn executable_suffix(&self) -> &'static str {
        if self.is_windows_msvc() {
            ".exe"
        } else {
            ""
        }
    }

    pub(crate) fn default_output_basename(&self, stem: &str) -> String {
        format!("{stem}{}", self.executable_suffix())
    }
}

pub(crate) fn host_triple() -> &'static str {
    host_triple_for(std::env::consts::OS, std::env::consts::ARCH)
}

pub(crate) fn linux_sysroot_from_env() -> Result<String> {
    std::env::var("SENGOO_LINUX_SYSROOT").map_err(|_| {
        miette::miette!(
            "cross-compiling to {REFERENCE_TARGET_LINUX_GNU} requires SENGOO_LINUX_SYSROOT; see {CROSS_COMPILATION_DOC}"
        )
    })
}

pub(crate) fn windows_cross_sdk_include_paths() -> Result<Vec<std::path::PathBuf>> {
    let sdk_root = std::env::var("SENGOO_WINDOWS_SDK_ROOT").map_err(|_| {
        miette::miette!(
            "cross-compiling to {REFERENCE_TARGET_WINDOWS_MSVC} requires SENGOO_WINDOWS_SDK_ROOT; see {CROSS_COMPILATION_DOC}"
        )
    })?;
    let sdk_root = Path::new(&sdk_root);
    let mut paths = Vec::new();
    for leaf in ["ucrt", "um", "shared"] {
        let include_dir = sdk_root.join(leaf);
        if include_dir.exists() {
            paths.push(include_dir);
        }
    }
    if paths.is_empty() {
        return Err(miette::miette!(
            "SENGOO_WINDOWS_SDK_ROOT '{}' does not contain ucrt/um/shared include directories; see {CROSS_COMPILATION_DOC}",
            sdk_root.display()
        ));
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_compile_host_triple_uses_macos_x86_64_when_requested() {
        assert_eq!(host_triple_for("macos", "x86_64"), "x86_64-apple-darwin");
    }

    #[test]
    fn cross_compile_host_triple_uses_macos_aarch64_when_requested() {
        assert_eq!(host_triple_for("macos", "aarch64"), "aarch64-apple-darwin");
    }

    #[test]
    fn cross_compile_rejects_unsupported_target_triple() {
        let err = NativeBuildTarget::resolve(Some("aarch64-unknown-linux-gnu"))
            .expect_err("unsupported triple should fail");
        let message = err.to_string();
        assert!(message.contains("unsupported target triple"));
        assert!(message.contains(CROSS_COMPILATION_DOC));
    }

    #[test]
    fn cross_compile_accepts_reference_targets() {
        for triple in REFERENCE_TARGETS {
            NativeBuildTarget::resolve(Some(triple)).expect("reference triple should be accepted");
        }
    }

    #[test]
    fn cross_compile_defaults_to_host_triple() {
        let target = NativeBuildTarget::resolve(None).expect("host triple should resolve");
        assert_eq!(target.triple, host_triple());
        assert!(!target.is_cross());
    }
}
