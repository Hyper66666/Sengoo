//! Native OS policy plus public owned snapshots and pure SG media headers.
mod common;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

fn scratch(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "sengoo-file-media-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn compiler() -> PathBuf {
    std::env::var_os("SENGOO_CLANG")
        .map(PathBuf::from)
        .unwrap_or_else(|| "clang".into())
}
fn passed(output: Output, label: &str, root: &Path) {
    fs::write(root.join(format!("{label}-stdout.txt")), &output.stdout).unwrap();
    fs::write(root.join(format!("{label}-stderr.txt")), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "{label}; retained {}\n{}\n{}",
        root.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn root() -> PathBuf {
    // Avoid canonicalize's Windows device prefix: clang's quoted relative
    // includes do not consistently resolve `..` from a \\?\ source spelling.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn native_file_snapshot_guard_bounds_and_cleanup() {
    let repository = root();
    let work = scratch("native");
    let breadth = fs::read_to_string(repository.join("tools/stdlib/runtime_breadth.c")).unwrap();
    let start = breadth
        .find("static const uint32_t sengoo_sha256_k[64]")
        .unwrap();
    let end = breadth.find("long long sengoo_hash_sha256(").unwrap();
    let core = work.join("sha_core.c");
    fs::write(
        &core,
        format!("#include <stdint.h>\n{}", &breadth[start..end]),
    )
    .unwrap();
    let binary = work.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    passed(
        Command::new(compiler())
            .args([
                "-std=c11",
                "-O1",
                "-Wall",
                "-Wextra",
                "-Wno-unused-function",
            ])
            .arg(repository.join("tools/stdlib/tests/file_snapshot_probe.c"))
            .arg(&core)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap(),
        "build",
        &work,
    );
    passed(Command::new(&binary).output().unwrap(), "native", &work);
}

#[test]
fn public_file_snapshot_move_drop_and_guarded_paths() {
    if !cfg!(windows) {
        return;
    } // The API reports unsupported; do not qualify POSIX guards.
    let repository = root();
    let work = scratch("owned");
    let source = work.join("main.sg");
    fs::copy(
        repository.join("tools/stdlib/tests/file_snapshot.sg"),
        &source,
    )
    .unwrap();
    let binary = work.join("snapshot.exe");
    passed(
        common::source_sgc_command()
            .arg("build")
            .arg(&source)
            .args(["--force-rebuild", "-O", "1", "-o"])
            .arg(&binary)
            .output()
            .unwrap(),
        "build",
        &work,
    );
    let file = work.join("binary 用户😀.bin");
    fs::write(&file, b"a\0b").unwrap();
    passed(
        Command::new(&binary).arg(&file).output().unwrap(),
        "public",
        &work,
    );
    assert_eq!(fs::read(&file).unwrap(), b"after-close");
    fs::remove_file(file).unwrap();
}

#[test]
fn pure_sengoo_media_header_classification_and_explicit_limits() {
    let repository = root();
    let work = scratch("headers");
    let source = work.join("main.sg");
    fs::copy(
        repository.join("tools/stdlib/tests/media_inspection.sg"),
        &source,
    )
    .unwrap();
    let binary = work.join(if cfg!(windows) { "media.exe" } else { "media" });
    passed(
        common::source_sgc_command()
            .arg("build")
            .arg(&source)
            .args(["--force-rebuild", "-O", "1", "-o"])
            .arg(&binary)
            .output()
            .unwrap(),
        "build",
        &work,
    );
    passed(Command::new(&binary).output().unwrap(), "headers", &work);
}

#[test]
fn installed_file_media_consumer_uses_copied_payload_without_cargo() {
    if !cfg!(windows) {
        return;
    } // Only this host has a qualified guard implementation.
    let repository = root();
    let work = scratch("installed");
    let install = work.join("install");
    let bin = install.join("bin");
    let stdlib = install.join("share/sengoo/stdlib");
    let runtime = install.join("share/sengoo/runtime/x86_64-pc-windows-msvc/sengoo_runtime.lib");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&stdlib).unwrap();
    fs::create_dir_all(runtime.parent().unwrap()).unwrap();
    let installed_sgc = bin.join("sgc.exe");
    fs::copy(env!("CARGO_BIN_EXE_sgc"), &installed_sgc).unwrap();
    fs::copy(
        repository.join("target/staticlib/sengoo_runtime.lib"),
        &runtime,
    )
    .expect("build the native staticlib fixture first");
    fn copy_payload(
        source: &Path,
        destination: &Path,
        install: &Path,
        payloads: &mut Vec<serde_json::Value>,
    ) {
        fs::copy(source, destination).unwrap();
        payloads.push(json!({"path":destination.strip_prefix(install).unwrap().to_string_lossy().replace('\\',"/"),"sha256":format!("{:x}",Sha256::digest(fs::read(destination).unwrap()))}));
    }
    fn copy_data(
        source: &Path,
        destination: &Path,
        install: &Path,
        payloads: &mut Vec<serde_json::Value>,
    ) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                copy_data(
                    &entry.path(),
                    &destination.join(entry.file_name()),
                    install,
                    payloads,
                );
            } else if entry.file_type().unwrap().is_file() {
                copy_payload(
                    &entry.path(),
                    &destination.join(entry.file_name()),
                    install,
                    payloads,
                );
            } else {
                panic!("fixture refuses nonregular package data");
            }
        }
    }
    let mut payloads = Vec::new();
    for entry in fs::read_dir(repository.join("tools/stdlib")).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = entry.file_name();
        let extension = path.extension().and_then(|v| v.to_str());
        if entry.file_type().unwrap().is_file()
            && (extension == Some("sg")
                || (name.to_string_lossy().starts_with("runtime")
                    && matches!(extension, Some("c" | "h"))))
        {
            copy_payload(&path, &stdlib.join(name), &install, &mut payloads);
        }
    }
    copy_data(
        &repository.join("tools/stdlib/unicode-data"),
        &stdlib.join("unicode-data"),
        &install,
        &mut payloads,
    );
    let runtime_hash = format!("{:x}", Sha256::digest(fs::read(&runtime).unwrap()));
    payloads.push(json!({"path":"share/sengoo/runtime/x86_64-pc-windows-msvc/sengoo_runtime.lib","sha256":runtime_hash}));
    let manifest = json!({"schema_version":2,"target":"x86_64-pc-windows-msvc","build_manifest_id":"1111111111111111111111111111111111111111111111111111111111111111","artifact_provenance":"prebuilt-unverified","release_eligible":false,"payloads":payloads,"native_runtime":{"abi_version":1,"target":"x86_64-pc-windows-msvc","library":"share/sengoo/runtime/x86_64-pc-windows-msvc/sengoo_runtime.lib","sha256":runtime_hash,"link_args":["kernel32.lib","ntdll.lib","userenv.lib","ws2_32.lib","dbghelp.lib","advapi32.lib","bcrypt.lib","crypt32.lib","ncrypt.lib","secur32.lib","legacy_stdio_definitions.lib","msvcrt.lib","vcruntime.lib","ucrt.lib"],"dynamic_dependencies":[]}});
    fs::write(
        install.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let restricted_path = std::env::join_paths([
        compiler().parent().unwrap().to_path_buf(),
        PathBuf::from("C:/Windows/System32"),
        PathBuf::from("C:/Windows"),
    ])
    .unwrap();
    let consumer = work.join("consumer");
    fs::create_dir(&consumer).unwrap();
    for (label, fixture) in [
        ("snapshot", "file_snapshot.sg"),
        ("media", "media_inspection.sg"),
    ] {
        let source = consumer.join(format!("{label}.sg"));
        fs::copy(repository.join("tools/stdlib/tests").join(fixture), &source).unwrap();
        let binary = consumer.join(format!("{label}.exe"));
        let mut command = Command::new(&installed_sgc);
        command
            .current_dir(&consumer)
            .arg("build")
            .arg(&source)
            .args(["--force-rebuild", "-O", "1", "-o"])
            .arg(&binary)
            .env("PATH", &restricted_path)
            .env("CARGO_HOME", work.join("unused-cargo-home"))
            .env_remove("SENGOO_ROOT")
            .env_remove("SENGOO_STDLIB")
            .env_remove("SENGOO_RUNTIME");
        passed(
            command.output().unwrap(),
            &format!("installed-{label}-build"),
            &work,
        );
        let mut run = Command::new(&binary);
        run.env("PATH", "C:/Windows/System32;C:/Windows");
        let data = consumer.join("copied binary 用户😀.bin");
        if label == "snapshot" {
            fs::write(&data, b"a\0b").unwrap();
            run.arg(&data);
        }
        passed(
            run.output().unwrap(),
            &format!("installed-{label}-run"),
            &work,
        );
        if label == "snapshot" {
            assert_eq!(fs::read(&data).unwrap(), b"after-close");
            fs::remove_file(data).unwrap();
        }
    }
    // Retain an explicit file inventory; no recursive cleanup and no source-tree rename.
    fs::write(work.join("evidence.txt"),format!("copied install: {}\nconsumer: {}\nrestricted PATH: {}\nno recursive cleanup; source checkout was not physically hidden\n",install.display(),consumer.display(),restricted_path.to_string_lossy())).unwrap();
    println!("retained installed evidence: {}", work.display());
}
