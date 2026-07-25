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

fn realworld_fixture_names() -> Vec<String> {
    let mut fixtures = fs::read_dir(workspace_root().join("examples/realworld"))
        .expect("read examples/realworld")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if !path.is_dir() || !path.join("Sengoo.toml").is_file() {
                return None;
            }
            Some(entry.file_name().to_string_lossy().into_owned())
        })
        .collect::<Vec<_>>();
    fixtures.sort();
    fixtures
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

fn workflow_step_block<'a>(workflow: &'a str, step_name: &str) -> &'a str {
    let marker = format!("- name: {step_name}");
    let start = workflow
        .find(&marker)
        .unwrap_or_else(|| panic!("workflow should contain step `{step_name}`"));
    let rest = &workflow[start..];
    let next = rest
        .match_indices("\n      - name: ")
        .next()
        .map(|(index, _)| index)
        .unwrap_or(rest.len());
    &rest[..next]
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

    for fixture in realworld_fixture_names() {
        let package = dir.join(&fixture);
        copy_dir_filtered(&realworld_fixture(&fixture), &package);

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
            vec!["--runtime-mode", "source-development", "check", "--locked"],
            vec!["--runtime-mode", "source-development", "test", "--locked"],
            vec!["fmt", "--check", "--locked"],
            vec!["--runtime-mode", "source-development", "doc", "--locked"],
            vec!["--runtime-mode", "source-development", "build", "--locked"],
            vec!["--runtime-mode", "source-development", "run", "--locked"],
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

#[test]
fn realworld_workflow_packages_and_installs_release_toolchain_before_running_every_fixture() {
    let workflow = fs::read_to_string(workspace_root().join(".github/workflows/realworld-e2e.yml"))
        .expect("read realworld-e2e workflow");
    let build_step = workflow_step_block(&workflow, "Build toolchain binaries");
    let package_step = workflow_step_block(&workflow, "Package toolchain");
    let realworld_step = workflow_step_block(&workflow, "Run installed realworld fixture loop");
    let official_step = workflow_step_block(&workflow, "Verify reviewed official package set");

    for target in [
        "ubuntu-latest",
        "windows-latest",
        "macos-15",
        "macos-15-intel",
    ] {
        assert!(
            workflow.contains(target),
            "workflow should cover supported host `{target}`"
        );
    }
    assert!(
        build_step.contains("cargo build --release -p sgc -p sgpm -p sgfmt -p sglsp"),
        "workflow should build release toolchain binaries before packaging"
    );
    assert!(
        package_step.contains("./scripts/package-toolchain.ps1 -Version 0.1.0-ci -NoBuild"),
        "workflow should package the prebuilt release binaries with -NoBuild"
    );
    assert!(
        package_step.contains(
            "$env:SENGOO_BUILD_HASH = \"${{ github.sha }}\".Substring(0, 12)"
        ),
        "workflow should pass package-toolchain the 12-character build identity embedded in the binaries"
    );
    assert!(
        workflow.contains("validate package-registry-distribution --strict")
            && !workflow.contains("validate package-release-defaults --strict"),
        "workflow should validate canonical package truth after package-release-defaults is archived"
    );
    assert!(
        workflow.find("cargo build --release -p sgc -p sgpm -p sgfmt -p sglsp")
            < workflow.find("./scripts/package-toolchain.ps1 -Version 0.1.0-ci -NoBuild"),
        "release build step should appear before package-toolchain -NoBuild"
    );
    for needle in [
        "actions/setup-python@v5",
        "Install package (POSIX)",
        "Install package (Windows)",
        "target/install-smoke",
        "Get-ChildItem examples/realworld -Directory",
        "Sengoo.toml",
        "sgpm update",
        "sgpm check --locked",
        "sgpm test --locked",
        "sgpm fmt --check --locked",
        "sgpm doc --locked",
        "sgpm build --locked",
        "sgpm run --locked",
    ] {
        assert!(
            realworld_step.contains(needle) || workflow.contains(needle),
            "workflow should contain `{needle}`"
        );
    }
    for needle in [
        "cli-json-audit",
        "workspace-audit",
        "http-client-status",
        "http-echo-service",
        "package-release-loop",
        "python-hot-path",
        "metadata --format json --locked",
        "publish --dry-run --locked --format json",
        "publish --registry local --locked --format json",
        "python smoke",
        "ctypes",
        ".sgreflect.json",
    ] {
        assert!(
            official_step.contains(needle),
            "official package review step should contain `{needle}`"
        );
    }
    assert!(
        !official_step.contains("documented host-only gap"),
        "official package review step should stop claiming Python interop as a documented gap"
    );
}
