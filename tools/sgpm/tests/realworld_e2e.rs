use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn sgpm() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sgpm"))
}

fn toolchain_binary(name: &str) -> PathBuf {
    let env_key = format!("CARGO_BIN_EXE_{name}");
    if let Ok(path) = std::env::var(&env_key) {
        return PathBuf::from(path);
    }
    let sgpm_bin = PathBuf::from(env!("CARGO_BIN_EXE_sgpm"));
    let dir = sgpm_bin
        .parent()
        .expect("sgpm binary should have a parent directory");
    if cfg!(windows) {
        dir.join(format!("{name}.exe"))
    } else {
        dir.join(name)
    }
}

fn sgc() -> PathBuf {
    toolchain_binary("sgc")
}

fn sgfmt() -> PathBuf {
    toolchain_binary("sgfmt")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgpm crate should live under tools/sgpm")
        .to_path_buf()
}

fn realworld_fixture(name: &str) -> PathBuf {
    workspace_root().join("examples/realworld").join(name)
}

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("sgpm_realworld_e2e_{name}_{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn copy_dir_filtered(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let file_name = entry.file_name();
        let file_name_text = file_name.to_string_lossy();
        if file_name_text == "target" || file_name_text == "build" {
            continue;
        }
        let destination_path = destination.join(file_name);
        if source_path.is_dir() {
            copy_dir_filtered(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path).unwrap();
        }
    }
}

fn native_toolchain_available() -> bool {
    which::which("clang").is_ok() || which::which("clang.exe").is_ok()
}

#[test]
fn realworld_locked_loop_uses_real_toolchain_binaries() {
    if !native_toolchain_available() {
        eprintln!(
            "skip realworld-e2e: native clang toolchain unavailable on this host (evidence: `clang` not on PATH)"
        );
        return;
    }

    let dir = temp_dir("realworld_locked_loop");
    let sgc_bin = sgc();
    let sgfmt_bin = sgfmt();
    if !sgc_bin.is_file() {
        eprintln!(
            "skip realworld-e2e: sgc binary unavailable at {}",
            sgc_bin.display()
        );
        return;
    }
    if !sgfmt_bin.is_file() {
        eprintln!(
            "skip realworld-e2e: sgfmt binary unavailable at {}",
            sgfmt_bin.display()
        );
        return;
    }

    for fixture in [
        "async-channel-smoke",
        "cli-json-audit",
        "compressed-json-artifact",
        "http-client-status",
        "http-echo-service",
        "package-release-loop",
        "workspace-doc-loop",
    ] {
        let package = dir.join(fixture);
        copy_dir_filtered(&realworld_fixture(fixture), &package);

        let update = Command::new(sgpm())
            .args(["update"])
            .current_dir(&package)
            .env("SGPM_SGC", &sgc_bin)
            .env("SGPM_SGFMT", &sgfmt_bin)
            .output()
            .expect("run sgpm update");
        assert!(
            update.status.success(),
            "{} update stdout:\n{}\nstderr:\n{}",
            fixture,
            String::from_utf8_lossy(&update.stdout),
            String::from_utf8_lossy(&update.stderr)
        );

        let lock_path = package.join("Sengoo.lock");
        let before = fs::read_to_string(&lock_path).expect("lockfile should exist after update");

        for args in [
            vec!["check", "--locked"],
            vec!["test", "--locked"],
            vec!["fmt", "--check", "--locked"],
            vec!["doc", "--locked"],
            vec!["build", "--locked"],
        ] {
            let output = Command::new(sgpm())
                .args(&args)
                .current_dir(&package)
                .env("SGPM_SGC", &sgc_bin)
                .env("SGPM_SGFMT", &sgfmt_bin)
                .output()
                .expect("run sgpm locked command");
            assert!(
                output.status.success(),
                "{} sgpm {} stdout:\n{}\nstderr:\n{}",
                fixture,
                args.join(" "),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );

            let after = fs::read_to_string(&lock_path).expect("lockfile should remain readable");
            assert_eq!(
                after,
                before,
                "{} sgpm {} should not rewrite Sengoo.lock",
                fixture,
                args.join(" ")
            );
        }
    }

    let _ = fs::remove_dir_all(dir);
}
