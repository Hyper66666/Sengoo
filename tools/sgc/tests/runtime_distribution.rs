use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("sgc_installed_runtime_{name}_{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn host_target() -> &'static str {
    if cfg!(windows) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_os = "linux") {
        "x86_64-unknown-linux-gnu"
    } else {
        "x86_64-apple-darwin"
    }
}

fn runtime_library_name() -> &'static str {
    if cfg!(windows) {
        "sengoo_runtime.lib"
    } else {
        "libsengoo_runtime.a"
    }
}

fn write_manifest_with_runtime_abi(install_root: &Path, abi_version: u32) {
    let runtime_relative = format!(
        "share/sengoo/runtime/{}/{}",
        host_target(),
        runtime_library_name()
    );
    let manifest = json!({
        "schema_version": 2,
        "version": env!("CARGO_PKG_VERSION"),
        "target": host_target(),
        "build_hash": "installed-runtime-test",
        "build_manifest_id": "1111111111111111111111111111111111111111111111111111111111111111",
        "payloads": [],
        "native_runtime": {
            "abi_version": abi_version,
            "target": host_target(),
            "library": runtime_relative,
            "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "link_args": [],
            "dynamic_dependencies": []
        }
    });
    fs::write(
        install_root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

fn update_manifest(install_root: &Path, update: impl FnOnce(&mut serde_json::Value)) {
    let manifest_path = install_root.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    update(&mut manifest);
    fs::write(manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

fn expected_runtime_link_args() -> Vec<&'static str> {
    if cfg!(windows) {
        vec![
            "kernel32.lib",
            "ntdll.lib",
            "userenv.lib",
            "ws2_32.lib",
            "dbghelp.lib",
            "advapi32.lib",
            "bcrypt.lib",
            "crypt32.lib",
            "ncrypt.lib",
            "secur32.lib",
            "legacy_stdio_definitions.lib",
            "msvcrt.lib",
            "vcruntime.lib",
            "ucrt.lib",
        ]
    } else if cfg!(target_os = "macos") {
        vec!["-framework", "Security", "-framework", "CoreFoundation"]
    } else {
        vec!["-lm"]
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

fn copy_installed_stdlib(install_root: &Path) {
    let source = workspace_root().join("tools").join("stdlib");
    let destination = install_root.join("share").join("sengoo").join("stdlib");
    fs::create_dir_all(&destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = path.file_name().unwrap();
        let is_runtime_bridge = name.to_string_lossy().starts_with("runtime")
            && matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("c" | "h")
            );
        if path.extension().and_then(|value| value.to_str()) == Some("sg") || is_runtime_bridge {
            fs::copy(&path, destination.join(name)).unwrap();
        }
    }
}

fn payload_entry(install_root: &Path, payload: &Path) -> serde_json::Value {
    let relative = payload
        .strip_prefix(install_root)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let bytes = fs::read(payload).unwrap();
    json!({
        "path": relative,
        "sha256": format!("{:x}", Sha256::digest(&bytes)),
        "size": bytes.len()
    })
}

fn runtime_bridge_paths(install_root: &Path) -> Vec<PathBuf> {
    let stdlib = install_root.join("share").join("sengoo").join("stdlib");
    [
        "runtime.c",
        "runtime_breadth.c",
        "runtime_collections.c",
        "runtime_json.c",
        "runtime_process.c",
        "runtime_string.c",
        "runtime_shared.h",
    ]
    .into_iter()
    .map(|file| stdlib.join(file))
    .collect()
}

fn assert_no_forbidden_paths(label: &str, text: &str, forbidden_paths: &[PathBuf]) {
    let normalized_text = text.replace('\\', "/").to_ascii_lowercase();
    for path in forbidden_paths {
        let normalized_path = fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        assert!(
            !normalized_text.contains(&normalized_path),
            "{label} exposes forbidden path {}",
            path.display()
        );
    }
}

fn write_fake_cargo(bin_dir: &Path, clang: &Path) {
    let source = bin_dir.join("fake_cargo.c");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <stdlib.h>

int main(void) {
    const char* marker = getenv("SENGOO_FAKE_CARGO_MARKER");
    if (marker) {
        FILE* output = fopen(marker, "wb");
        if (output) {
            fputs("invoked", output);
            fclose(output);
        }
    }
    return 97;
}
"#,
    )
    .unwrap();
    let executable = bin_dir.join(if cfg!(windows) { "cargo.exe" } else { "cargo" });
    let status = Command::new(clang)
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .status()
        .expect("fake Cargo compiler should run");
    assert!(status.success(), "fake Cargo should compile");
}

#[test]
fn source_native_build_requires_explicit_source_runtime_mode_before_cargo() {
    let Ok(clang) = which::which("clang").or_else(|_| which::which("clang.exe")) else {
        eprintln!("skip: native clang toolchain unavailable");
        return;
    };

    let root = temp_dir("source_mode_required");
    let consumer = root.join("consumer");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&consumer).unwrap();
    fs::create_dir_all(&fake_bin).unwrap();
    fs::write(consumer.join("main.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let marker = root.join("cargo-invoked.txt");
    write_fake_cargo(&fake_bin, &clang);
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let joined_path = std::env::join_paths(
        std::iter::once(fake_bin.clone()).chain(std::env::split_paths(&original_path)),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .current_dir(&consumer)
        .args(["build", "main.sg", "--force-rebuild"])
        .env("PATH", joined_path)
        .env("SENGOO_FAKE_CARGO_MARKER", &marker)
        .output()
        .expect("source sgc should execute");

    assert!(!output.status.success(), "implicit source mode must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("source runtime development mode is not selected")
            && stderr.contains("source-development")
            && stderr.contains("non-release Cargo runtime construction"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
    assert!(
        !marker.exists(),
        "source-local layout alone must not authorize Cargo runtime construction"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn explicit_source_runtime_mode_marks_cargo_build_as_non_release() {
    let Ok(clang) = which::which("clang").or_else(|_| which::which("clang.exe")) else {
        eprintln!("skip: native clang toolchain unavailable");
        return;
    };

    let root = temp_dir("source_mode_non_release");
    let consumer = root.join("consumer");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&consumer).unwrap();
    fs::create_dir_all(&fake_bin).unwrap();
    fs::write(consumer.join("main.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let marker = root.join("cargo-invoked.txt");
    write_fake_cargo(&fake_bin, &clang);
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let joined_path = std::env::join_paths(
        std::iter::once(fake_bin.clone()).chain(std::env::split_paths(&original_path)),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .current_dir(&consumer)
        .args([
            "--runtime-mode",
            "source-development",
            "build",
            "main.sg",
            "--force-rebuild",
        ])
        .env("PATH", joined_path)
        .env("SENGOO_FAKE_CARGO_MARKER", &marker)
        .output()
        .expect("source sgc should execute");

    assert!(
        !output.status.success(),
        "the fake Cargo fixture should fail after source mode is authorized"
    );
    assert!(
        marker.exists(),
        "explicit source mode must reach fake Cargo"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[toolchain::source_runtime_development]")
            && stderr.contains("runtime_mode=source-development")
            && stderr.contains("artifact_provenance=source-cargo-development")
            && stderr.contains("release_eligible=false")
            && stderr.contains("senline_pin_evidence=false"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn explicit_source_runtime_build_records_non_release_provenance() {
    let root = temp_dir("source_mode_metadata");
    let consumer = root.join("consumer");
    fs::create_dir_all(&consumer).unwrap();
    fs::write(consumer.join("main.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .current_dir(&consumer)
        .args([
            "--runtime-mode",
            "source-development",
            "build",
            "main.sg",
            "--force-rebuild",
        ])
        .output()
        .expect("source sgc should execute");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata: serde_json::Value = serde_json::from_slice(
        &fs::read(consumer.join("build").join("main.build-cache.json")).unwrap(),
    )
    .unwrap();
    let provenance = &metadata["runtime_provenance"];
    assert_eq!(provenance["runtime_mode"], "source-development");
    assert_eq!(
        provenance["artifact_provenance"],
        "source-cargo-development"
    );
    assert_eq!(provenance["release_eligible"], false);
    assert_eq!(provenance["senline_pin_evidence"], false);
    assert!(provenance["build_manifest_id"].is_null());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn explicit_source_runtime_mode_propagates_to_test_children() {
    let root = temp_dir("source_mode_test_child");
    let package = root.join("package");
    let tests = package.join("tests");
    fs::create_dir_all(&tests).unwrap();
    fs::write(tests.join("pass.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .current_dir(&package)
        .args(["--runtime-mode", "source-development", "test", "."])
        .output()
        .expect("source sgc test should execute");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("test result: 1 passed"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(tests.join("build").join("pass.run-cache.json")).unwrap())
            .unwrap();
    let provenance = &metadata["runtime_provenance"];
    assert_eq!(provenance["runtime_mode"], "source-development");
    assert_eq!(
        provenance["artifact_provenance"],
        "source-cargo-development"
    );
    assert_eq!(provenance["release_eligible"], false);
    assert_eq!(provenance["senline_pin_evidence"], false);
    assert!(provenance["build_manifest_id"].is_null());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn fresh_installed_build_and_run_avoid_cargo_and_absolute_runtime_identity() {
    let clang = which::which("clang")
        .or_else(|_| which::which("clang.exe"))
        .expect("distribution smoke requires the native clang toolchain");
    let runtime_source = workspace_root()
        .join("target")
        .join("staticlib")
        .join(runtime_library_name());
    assert!(
        runtime_source.is_file(),
        "distribution runtime fixture is missing: {}; build the staticlib profile first",
        runtime_source.display()
    );

    let root = temp_dir("fresh_installed_smoke");
    let install_root = root.join("install");
    let install_bin = install_root.join("bin");
    let runtime_dir = install_root
        .join("share")
        .join("sengoo")
        .join("runtime")
        .join(host_target());
    let consumer = root.join("consumer");
    let fake_bin = root.join("fake-bin");
    let fake_home = root.join("forbidden-user-home");
    let fake_cargo_home = root.join("forbidden-cargo-home");
    fs::create_dir_all(&install_bin).unwrap();
    fs::create_dir_all(&runtime_dir).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    fs::create_dir_all(&fake_bin).unwrap();
    fs::create_dir_all(&fake_home).unwrap();
    fs::create_dir_all(&fake_cargo_home).unwrap();
    copy_installed_stdlib(&install_root);

    let installed_sgc = install_bin.join(if cfg!(windows) { "sgc.exe" } else { "sgc" });
    fs::copy(env!("CARGO_BIN_EXE_sgc"), &installed_sgc).unwrap();
    let runtime_library = runtime_dir.join(runtime_library_name());
    fs::copy(&runtime_source, &runtime_library).unwrap();
    let runtime_hash = format!("{:x}", Sha256::digest(fs::read(&runtime_library).unwrap()));
    write_manifest_with_runtime_abi(&install_root, 1);
    update_manifest(&install_root, |manifest| {
        manifest["artifact_provenance"] = json!("prebuilt-unverified");
        manifest["release_eligible"] = json!(false);
        manifest["native_runtime"]["sha256"] = json!(runtime_hash);
        manifest["native_runtime"]["link_args"] = json!(expected_runtime_link_args());
        let mut payloads = runtime_bridge_paths(&install_root)
            .iter()
            .map(|path| payload_entry(&install_root, path))
            .collect::<Vec<_>>();
        payloads.push(payload_entry(&install_root, &runtime_library));
        manifest["payloads"] = json!(payloads);
    });
    fs::write(
        consumer.join("main.sg"),
        "import std::status;\n\ndef main() -> i64 { STATUS_OK() }\n",
    )
    .unwrap();
    fs::create_dir_all(consumer.join("tests")).unwrap();
    fs::write(
        consumer.join("tests").join("pass.sg"),
        "def main() -> i64 { 0 }\n",
    )
    .unwrap();

    let marker = root.join("cargo-invoked.txt");
    write_fake_cargo(&fake_bin, &clang);
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let joined_path = std::env::join_paths(
        std::iter::once(fake_bin.clone()).chain(std::env::split_paths(&original_path)),
    )
    .unwrap();
    let mut command_transcript = String::new();
    for arguments in [
        vec!["check", "main.sg"],
        vec!["build", "main.sg", "--force-rebuild"],
        vec!["run", "main.sg", "--force-rebuild"],
        vec!["test", "."],
    ] {
        let output = Command::new(&installed_sgc)
            .current_dir(&consumer)
            .args(&arguments)
            .env("PATH", &joined_path)
            .env("SENGOO_FAKE_CARGO_MARKER", &marker)
            .env("HOME", &fake_home)
            .env("USERPROFILE", &fake_home)
            .env("CARGO_HOME", &fake_cargo_home)
            .env_remove("SENGOO_ROOT")
            .env_remove("SENGOO_STDLIB")
            .env_remove("SENGOO_RUNTIME")
            .output()
            .expect("installed sgc command should execute");
        assert!(
            output.status.success(),
            "command {:?}\nstdout:\n{}\nstderr:\n{}",
            arguments,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        command_transcript.push_str(&String::from_utf8_lossy(&output.stdout));
        command_transcript.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    assert!(!marker.exists(), "installed commands must not invoke Cargo");

    let forbidden_paths = vec![
        workspace_root(),
        workspace_root().join("target"),
        fake_home.clone(),
        fake_cargo_home.clone(),
    ];
    assert_no_forbidden_paths(
        "installed command output",
        &command_transcript,
        &forbidden_paths,
    );
    for metadata_path in [
        consumer.join("build").join("main.build-cache.json"),
        consumer.join("build").join("main.run-cache.json"),
        consumer
            .join("tests")
            .join("build")
            .join("pass.run-cache.json"),
    ] {
        let metadata_text = fs::read_to_string(&metadata_path).unwrap();
        assert_no_forbidden_paths(
            &format!("installed cache metadata {}", metadata_path.display()),
            &metadata_text,
            &forbidden_paths,
        );
        let metadata: serde_json::Value = serde_json::from_str(&metadata_text).unwrap();
        assert_eq!(metadata["runtime_provenance"]["runtime_mode"], "installed");
        assert_eq!(
            metadata["runtime_provenance"]["artifact_provenance"],
            "prebuilt-unverified"
        );
        assert_eq!(metadata["runtime_provenance"]["release_eligible"], false);
        assert_eq!(
            metadata["runtime_provenance"]["senline_pin_evidence"],
            false
        );
        assert_eq!(
            metadata["runtime_provenance"]["build_manifest_id"],
            "1111111111111111111111111111111111111111111111111111111111111111"
        );
        let runtime_identity = metadata["runtime_c"].as_str().unwrap_or_default();
        assert_eq!(
            runtime_identity, "installed:share/sengoo/stdlib/runtime.c",
            "runtime identity must not expose an install, checkout, Cargo, or user-profile path"
        );
    }

    let manifest_text = fs::read_to_string(install_root.join("manifest.json")).unwrap();
    assert_no_forbidden_paths("installed manifest", &manifest_text, &forbidden_paths);
    assert!(!manifest_text.contains(&workspace_root().to_string_lossy().to_string()));
    assert!(!manifest_text.to_ascii_lowercase().contains("cargo"));

    update_manifest(&install_root, |manifest| {
        manifest["artifact_provenance"] = json!("forged-local-claim");
        manifest["release_eligible"] = json!(true);
    });
    let output = Command::new(&installed_sgc)
        .current_dir(&consumer)
        .args(["build", "main.sg"])
        .env("PATH", &joined_path)
        .env("SENGOO_FAKE_CARGO_MARKER", &marker)
        .env("HOME", &fake_home)
        .env("USERPROFILE", &fake_home)
        .env("CARGO_HOME", &fake_cargo_home)
        .env_remove("SENGOO_ROOT")
        .env_remove("SENGOO_STDLIB")
        .env_remove("SENGOO_RUNTIME")
        .output()
        .expect("installed sgc build should execute after provenance changes");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("build cache miss: metadata changed"),
        "provenance changes must invalidate installed build cache identity\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let metadata: serde_json::Value = serde_json::from_slice(
        &fs::read(consumer.join("build").join("main.build-cache.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        metadata["runtime_provenance"]["artifact_provenance"],
        "forged-local-claim"
    );
    assert_eq!(metadata["runtime_provenance"]["release_eligible"], true);
    assert_eq!(
        metadata["runtime_provenance"]["senline_pin_evidence"], false,
        "an installed manifest cannot self-authenticate as Senline pin evidence"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn installed_check_rejects_wrong_runtime_abi() {
    let root = temp_dir("check_wrong_abi");
    let install_root = root.join("install");
    let install_bin = install_root.join("bin");
    let consumer = root.join("consumer");
    fs::create_dir_all(&install_bin).unwrap();
    fs::create_dir_all(&consumer).unwrap();

    let installed_sgc = install_bin.join(if cfg!(windows) { "sgc.exe" } else { "sgc" });
    fs::copy(env!("CARGO_BIN_EXE_sgc"), &installed_sgc).unwrap();
    write_manifest_with_runtime_abi(&install_root, 2);
    fs::write(consumer.join("main.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let output = Command::new(&installed_sgc)
        .current_dir(&consumer)
        .args(["check", "main.sg"])
        .env_remove("SENGOO_ROOT")
        .env_remove("SENGOO_STDLIB")
        .env_remove("SENGOO_RUNTIME")
        .output()
        .expect("installed sgc check should execute");

    assert!(
        !output.status.success(),
        "wrong runtime ABI must fail check"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("installed native runtime ABI mismatch")
            && stderr.contains("manifest=2")
            && stderr.contains("supported=1"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn installed_check_rejects_wrong_toolchain_target() {
    let root = temp_dir("check_wrong_target");
    let install_root = root.join("install");
    let install_bin = install_root.join("bin");
    let consumer = root.join("consumer");
    fs::create_dir_all(&install_bin).unwrap();
    fs::create_dir_all(&consumer).unwrap();

    let installed_sgc = install_bin.join(if cfg!(windows) { "sgc.exe" } else { "sgc" });
    fs::copy(env!("CARGO_BIN_EXE_sgc"), &installed_sgc).unwrap();
    write_manifest_with_runtime_abi(&install_root, 1);
    update_manifest(&install_root, |manifest| {
        manifest["target"] = json!("aarch64-unknown-invalid");
    });
    fs::write(consumer.join("main.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let output = Command::new(&installed_sgc)
        .current_dir(&consumer)
        .args(["check", "main.sg"])
        .output()
        .expect("installed sgc check should execute");

    assert!(!output.status.success(), "wrong target must fail check");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("installed toolchain target mismatch")
            && stderr.contains("manifest=aarch64-unknown-invalid")
            && stderr.contains(host_target()),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn installed_check_rejects_incomplete_runtime_metadata() {
    let root = temp_dir("check_incomplete_metadata");
    let install_root = root.join("install");
    let install_bin = install_root.join("bin");
    let consumer = root.join("consumer");
    fs::create_dir_all(&install_bin).unwrap();
    fs::create_dir_all(&consumer).unwrap();

    let installed_sgc = install_bin.join(if cfg!(windows) { "sgc.exe" } else { "sgc" });
    fs::copy(env!("CARGO_BIN_EXE_sgc"), &installed_sgc).unwrap();
    write_manifest_with_runtime_abi(&install_root, 1);
    update_manifest(&install_root, |manifest| {
        manifest["native_runtime"]
            .as_object_mut()
            .unwrap()
            .remove("link_args");
    });
    fs::write(consumer.join("main.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let output = Command::new(&installed_sgc)
        .current_dir(&consumer)
        .args(["check", "main.sg"])
        .output()
        .expect("installed sgc check should execute");

    assert!(
        !output.status.success(),
        "missing runtime metadata must fail check"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid installed toolchain manifest")
            && stderr.contains("missing field `link_args`"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn installed_check_rejects_incomplete_runtime_bridge() {
    let root = temp_dir("check_incomplete_bridge");
    let install_root = root.join("install");
    let install_bin = install_root.join("bin");
    let runtime_dir = install_root
        .join("share")
        .join("sengoo")
        .join("runtime")
        .join(host_target());
    let stdlib_dir = install_root.join("share").join("sengoo").join("stdlib");
    let consumer = root.join("consumer");
    fs::create_dir_all(&install_bin).unwrap();
    fs::create_dir_all(&runtime_dir).unwrap();
    fs::create_dir_all(&stdlib_dir).unwrap();
    fs::create_dir_all(&consumer).unwrap();

    let installed_sgc = install_bin.join(if cfg!(windows) { "sgc.exe" } else { "sgc" });
    fs::copy(env!("CARGO_BIN_EXE_sgc"), &installed_sgc).unwrap();
    let runtime_library = runtime_dir.join(runtime_library_name());
    fs::write(&runtime_library, b"runtime fixture").unwrap();
    let runtime_hash = format!("{:x}", Sha256::digest(b"runtime fixture"));
    write_manifest_with_runtime_abi(&install_root, 1);
    update_manifest(&install_root, |manifest| {
        manifest["native_runtime"]["sha256"] = json!(runtime_hash);
        manifest["native_runtime"]["link_args"] = json!(expected_runtime_link_args());
    });
    for file in [
        "runtime.c",
        "runtime_breadth.c",
        "runtime_collections.c",
        "runtime_process.c",
        "runtime_string.c",
        "runtime_shared.h",
    ] {
        fs::write(stdlib_dir.join(file), b"bridge fixture").unwrap();
    }
    fs::write(consumer.join("main.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let output = Command::new(&installed_sgc)
        .current_dir(&consumer)
        .args(["check", "main.sg"])
        .output()
        .expect("installed sgc check should execute");

    assert!(
        !output.status.success(),
        "missing runtime bridge file must fail check"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("installed native runtime bridge file is missing")
            && stderr.contains("runtime_json.c"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn installed_check_rejects_tampered_runtime_bridge_payload() {
    let root = temp_dir("check_tampered_bridge");
    let install_root = root.join("install");
    let install_bin = install_root.join("bin");
    let runtime_dir = install_root
        .join("share")
        .join("sengoo")
        .join("runtime")
        .join(host_target());
    let stdlib_dir = install_root.join("share").join("sengoo").join("stdlib");
    let consumer = root.join("consumer");
    fs::create_dir_all(&install_bin).unwrap();
    fs::create_dir_all(&runtime_dir).unwrap();
    fs::create_dir_all(&stdlib_dir).unwrap();
    fs::create_dir_all(&consumer).unwrap();

    let installed_sgc = install_bin.join(if cfg!(windows) { "sgc.exe" } else { "sgc" });
    fs::copy(env!("CARGO_BIN_EXE_sgc"), &installed_sgc).unwrap();
    let runtime_library = runtime_dir.join(runtime_library_name());
    fs::write(&runtime_library, b"runtime fixture").unwrap();
    let runtime_hash = format!("{:x}", Sha256::digest(b"runtime fixture"));
    for path in runtime_bridge_paths(&install_root) {
        fs::write(path, b"bridge fixture").unwrap();
    }
    write_manifest_with_runtime_abi(&install_root, 1);
    update_manifest(&install_root, |manifest| {
        manifest["native_runtime"]["sha256"] = json!(runtime_hash);
        manifest["native_runtime"]["link_args"] = json!(expected_runtime_link_args());
        let mut payloads = runtime_bridge_paths(&install_root)
            .iter()
            .map(|path| payload_entry(&install_root, path))
            .collect::<Vec<_>>();
        payloads.push(payload_entry(&install_root, &runtime_library));
        manifest["payloads"] = json!(payloads);
    });
    fs::write(
        stdlib_dir.join("runtime_json.c"),
        b"tampered bridge fixture",
    )
    .unwrap();
    fs::write(consumer.join("main.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let output = Command::new(&installed_sgc)
        .current_dir(&consumer)
        .args(["check", "main.sg"])
        .output()
        .expect("installed sgc check should execute");

    assert!(
        !output.status.success(),
        "tampered runtime bridge must fail check"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("installed runtime payload SHA-256 mismatch")
            && stderr.contains("runtime_json.c"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn installed_commands_reject_external_runtime_overrides() {
    let root = temp_dir("check_external_runtime_override");
    let install_root = root.join("install");
    let install_bin = install_root.join("bin");
    let runtime_dir = install_root
        .join("share")
        .join("sengoo")
        .join("runtime")
        .join(host_target());
    let consumer = root.join("consumer");
    fs::create_dir_all(&install_bin).unwrap();
    fs::create_dir_all(&runtime_dir).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    copy_installed_stdlib(&install_root);

    let installed_sgc = install_bin.join(if cfg!(windows) { "sgc.exe" } else { "sgc" });
    fs::copy(env!("CARGO_BIN_EXE_sgc"), &installed_sgc).unwrap();
    let runtime_library = runtime_dir.join(runtime_library_name());
    fs::write(&runtime_library, b"runtime fixture").unwrap();
    let runtime_hash = format!("{:x}", Sha256::digest(b"runtime fixture"));
    write_manifest_with_runtime_abi(&install_root, 1);
    update_manifest(&install_root, |manifest| {
        manifest["native_runtime"]["sha256"] = json!(runtime_hash);
        manifest["native_runtime"]["link_args"] = json!(expected_runtime_link_args());
        let mut payloads = runtime_bridge_paths(&install_root)
            .iter()
            .map(|path| payload_entry(&install_root, path))
            .collect::<Vec<_>>();
        payloads.push(payload_entry(&install_root, &runtime_library));
        manifest["payloads"] = json!(payloads);
    });
    fs::write(consumer.join("main.sg"), "def main() -> i64 { 0 }\n").unwrap();
    let external_runtime = root.join("external-runtime.c");
    fs::write(&external_runtime, "int sengoo_external_runtime = 1;\n").unwrap();
    let external_stdlib = root.join("external-stdlib");
    let external_root = root.join("external-root");
    fs::create_dir_all(&external_stdlib).unwrap();
    fs::create_dir_all(&external_root).unwrap();

    for (variable, value) in [
        ("SENGOO_RUNTIME", external_runtime.as_path()),
        ("SENGOO_STDLIB", external_stdlib.as_path()),
        ("SENGOO_ROOT", external_root.as_path()),
    ] {
        for arguments in [
            vec!["check", "main.sg"],
            vec!["build", "main.sg", "--emit-llvm"],
            vec!["run", "--cranelift-fast-jit", "main.sg"],
            vec!["test", "."],
        ] {
            let output = Command::new(&installed_sgc)
                .current_dir(&consumer)
                .args(&arguments)
                .env_remove("SENGOO_ROOT")
                .env_remove("SENGOO_STDLIB")
                .env_remove("SENGOO_RUNTIME")
                .env(variable, value)
                .output()
                .expect("installed sgc command should execute");

            assert!(
                !output.status.success(),
                "installed {:?} must reject {variable} overrides",
                arguments
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains(&format!("installed runtime mode rejects {variable}")),
                "command {:?}\nstdout:\n{}\nstderr:\n{}",
                arguments,
                String::from_utf8_lossy(&output.stdout),
                stderr
            );
        }
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn installed_test_rejects_wrong_runtime_abi_before_empty_suite_success() {
    let root = temp_dir("test_wrong_abi");
    let install_root = root.join("install");
    let install_bin = install_root.join("bin");
    let package = root.join("package");
    fs::create_dir_all(&install_bin).unwrap();
    fs::create_dir_all(&package).unwrap();

    let installed_sgc = install_bin.join(if cfg!(windows) { "sgc.exe" } else { "sgc" });
    fs::copy(env!("CARGO_BIN_EXE_sgc"), &installed_sgc).unwrap();
    write_manifest_with_runtime_abi(&install_root, 2);

    let output = Command::new(&installed_sgc)
        .current_dir(&package)
        .args(["test", "."])
        .env_remove("SENGOO_ROOT")
        .env_remove("SENGOO_STDLIB")
        .env_remove("SENGOO_RUNTIME")
        .output()
        .expect("installed sgc test should execute");

    assert!(
        !output.status.success(),
        "wrong runtime ABI must fail before an empty suite reports success"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("installed native runtime ABI mismatch")
            && stderr.contains("manifest=2")
            && stderr.contains("supported=1"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn installed_native_build_rejects_missing_manifest_runtime_without_cargo_or_checkout_fallback() {
    let Ok(clang) = which::which("clang").or_else(|_| which::which("clang.exe")) else {
        eprintln!("skip: native clang toolchain unavailable");
        return;
    };

    let root = temp_dir("missing_runtime");
    let install_root = root.join("install");
    let install_bin = install_root.join("bin");
    let runtime_dir = install_root
        .join("share")
        .join("sengoo")
        .join("runtime")
        .join(host_target());
    let consumer = root.join("consumer");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&install_bin).unwrap();
    fs::create_dir_all(&runtime_dir).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    fs::create_dir_all(&fake_bin).unwrap();

    let installed_sgc = install_bin.join(if cfg!(windows) { "sgc.exe" } else { "sgc" });
    fs::copy(env!("CARGO_BIN_EXE_sgc"), &installed_sgc).unwrap();

    let runtime_relative = format!(
        "share/sengoo/runtime/{}/{}",
        host_target(),
        runtime_library_name()
    );
    let manifest = json!({
        "schema_version": 2,
        "version": env!("CARGO_PKG_VERSION"),
        "target": host_target(),
        "build_hash": "installed-runtime-test",
        "build_manifest_id": "1111111111111111111111111111111111111111111111111111111111111111",
        "payloads": [],
        "native_runtime": {
            "abi_version": 1,
            "target": host_target(),
            "library": runtime_relative,
            "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "link_args": [],
            "dynamic_dependencies": []
        }
    });
    fs::write(
        install_root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let source = consumer.join("main.sg");
    fs::write(&source, "def main() -> i64 { 0 }\n").unwrap();

    let marker = root.join("cargo-invoked.txt");
    write_fake_cargo(&fake_bin, &clang);
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let joined_path = std::env::join_paths(
        std::iter::once(fake_bin.clone()).chain(std::env::split_paths(&original_path)),
    )
    .unwrap();
    let output = Command::new(&installed_sgc)
        .current_dir(&consumer)
        .args(["build", "main.sg", "--force-rebuild"])
        .env("PATH", joined_path)
        .env("SENGOO_FAKE_CARGO_MARKER", &marker)
        .env_remove("SENGOO_ROOT")
        .env_remove("SENGOO_STDLIB")
        .env_remove("SENGOO_RUNTIME")
        .output()
        .expect("installed sgc should execute");

    assert!(
        !output.status.success(),
        "missing runtime must fail the build"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("installed native runtime library is missing")
            && stderr.contains(runtime_library_name()),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
    assert!(
        !marker.exists(),
        "installed builds must not invoke Cargo or fall back to the source checkout"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn relocated_sgc_without_manifest_rejects_implicit_source_checkout_fallback() {
    let Ok(clang) = which::which("clang").or_else(|_| which::which("clang.exe")) else {
        eprintln!("skip: native clang toolchain unavailable");
        return;
    };

    let root = temp_dir("missing_manifest");
    let install_bin = root.join("relocated").join("bin");
    let consumer = root.join("consumer");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&install_bin).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    fs::create_dir_all(&fake_bin).unwrap();
    let relocated_sgc = install_bin.join(if cfg!(windows) { "sgc.exe" } else { "sgc" });
    fs::copy(env!("CARGO_BIN_EXE_sgc"), &relocated_sgc).unwrap();
    fs::write(consumer.join("main.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let priming = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .current_dir(&consumer)
        .args([
            "--runtime-mode",
            "source-development",
            "build",
            "main.sg",
            "--force-rebuild",
        ])
        .output()
        .expect("source compiler should prime the native build cache");
    assert!(
        priming.status.success(),
        "cache priming failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&priming.stdout),
        String::from_utf8_lossy(&priming.stderr)
    );

    let marker = root.join("cargo-invoked.txt");
    write_fake_cargo(&fake_bin, &clang);
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let joined_path = std::env::join_paths(
        std::iter::once(fake_bin.clone()).chain(std::env::split_paths(&original_path)),
    )
    .unwrap();
    let output = Command::new(&relocated_sgc)
        .current_dir(&consumer)
        .args(["build", "main.sg"])
        .env("PATH", joined_path)
        .env("SENGOO_FAKE_CARGO_MARKER", &marker)
        .env_remove("SENGOO_ROOT")
        .env_remove("SENGOO_STDLIB")
        .env_remove("SENGOO_RUNTIME")
        .output()
        .expect("relocated sgc should execute");

    assert!(
        !output.status.success(),
        "missing manifest must fail the build"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("installed toolchain manifest is missing")
            && stderr.contains("Cargo fallback is disabled"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
    assert!(
        !marker.exists(),
        "relocated compilers must not fall back to Cargo or a compiled-in checkout"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn installed_native_build_rejects_runtime_hash_mismatch_before_link_or_cargo() {
    let Ok(clang) = which::which("clang").or_else(|_| which::which("clang.exe")) else {
        eprintln!("skip: native clang toolchain unavailable");
        return;
    };

    let root = temp_dir("hash_mismatch");
    let install_root = root.join("install");
    let install_bin = install_root.join("bin");
    let runtime_dir = install_root
        .join("share")
        .join("sengoo")
        .join("runtime")
        .join(host_target());
    let consumer = root.join("consumer");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&install_bin).unwrap();
    fs::create_dir_all(&runtime_dir).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    fs::create_dir_all(&fake_bin).unwrap();

    let installed_sgc = install_bin.join(if cfg!(windows) { "sgc.exe" } else { "sgc" });
    fs::copy(env!("CARGO_BIN_EXE_sgc"), &installed_sgc).unwrap();
    let runtime_library = runtime_dir.join(runtime_library_name());
    fs::write(&runtime_library, b"not a native runtime library").unwrap();

    let runtime_relative = format!(
        "share/sengoo/runtime/{}/{}",
        host_target(),
        runtime_library_name()
    );
    let manifest = json!({
        "schema_version": 2,
        "version": env!("CARGO_PKG_VERSION"),
        "target": host_target(),
        "build_hash": "installed-runtime-test",
        "build_manifest_id": "1111111111111111111111111111111111111111111111111111111111111111",
        "payloads": [],
        "native_runtime": {
            "abi_version": 1,
            "target": host_target(),
            "library": runtime_relative,
            "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "link_args": [],
            "dynamic_dependencies": []
        }
    });
    fs::write(
        install_root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    fs::write(consumer.join("main.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let priming = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .current_dir(&consumer)
        .args([
            "--runtime-mode",
            "source-development",
            "build",
            "main.sg",
            "--force-rebuild",
        ])
        .output()
        .expect("source compiler should prime the native build cache");
    assert!(
        priming.status.success(),
        "cache priming failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&priming.stdout),
        String::from_utf8_lossy(&priming.stderr)
    );

    let marker = root.join("cargo-invoked.txt");
    write_fake_cargo(&fake_bin, &clang);
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let joined_path = std::env::join_paths(
        std::iter::once(fake_bin.clone()).chain(std::env::split_paths(&original_path)),
    )
    .unwrap();
    let output = Command::new(&installed_sgc)
        .current_dir(&consumer)
        .args(["build", "main.sg"])
        .env("PATH", joined_path)
        .env("SENGOO_FAKE_CARGO_MARKER", &marker)
        .env_remove("SENGOO_ROOT")
        .env_remove("SENGOO_STDLIB")
        .env_remove("SENGOO_RUNTIME")
        .output()
        .expect("installed sgc should execute");

    assert!(
        !output.status.success(),
        "tampered runtime must fail the build"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("installed native runtime SHA-256 mismatch")
            && stderr.contains(runtime_library_name()),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
    assert!(
        !marker.exists(),
        "runtime verification must not invoke Cargo or the source checkout"
    );

    let _ = fs::remove_dir_all(root);
}
