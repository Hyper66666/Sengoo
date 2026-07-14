use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgpm manifest should live under tools/sgpm")
        .to_path_buf()
}

fn toml_value(path: &Path) -> toml::Value {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    raw.parse::<toml::Value>()
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()))
}

fn workspace_version(root: &Path) -> String {
    toml_value(&root.join("Cargo.toml"))["workspace"]["package"]["version"]
        .as_str()
        .expect("workspace package version should be a string")
        .to_string()
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

fn assert_tool_manifest_uses_workspace_version(root: &Path, tool: &str) {
    let manifest = toml_value(&root.join("tools").join(tool).join("Cargo.toml"));
    let package = manifest["package"]
        .as_table()
        .unwrap_or_else(|| panic!("{tool} manifest should have [package]"));
    let version = package
        .get("version")
        .unwrap_or_else(|| panic!("{tool} manifest should declare package.version"));
    let workspace = version
        .get("workspace")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    assert!(
        workspace,
        "{tool} must inherit package.version from workspace.package.version"
    );
}

fn parse_tool_version(tool: &str, stdout: &[u8]) -> (String, String) {
    let line = String::from_utf8_lossy(stdout).trim().to_string();
    let prefix = format!("{tool} ");
    assert!(
        line.starts_with(&prefix),
        "{tool} --version should start with `{prefix}`, got `{line}`"
    );
    let rest = &line[prefix.len()..];
    let open = rest
        .rfind(" (")
        .unwrap_or_else(|| panic!("{tool} --version should include a hash in parentheses: {line}"));
    assert!(
        rest.ends_with(')'),
        "{tool} --version should end with `)`, got `{line}`"
    );
    let version = rest[..open].to_string();
    let hash = rest[open + 2..rest.len() - 1].to_string();
    assert!(
        !version.trim().is_empty(),
        "{tool} version must not be empty"
    );
    assert!(!hash.trim().is_empty(), "{tool} hash must not be empty");
    (version, hash)
}

#[test]
fn tool_versions_share_workspace_version_and_hash() {
    let root = workspace_root();
    let expected_version = workspace_version(&root);
    let tools = ["sgc", "sgpm", "sgfmt", "sglsp"];
    let mut signatures = BTreeMap::new();

    for tool in tools {
        assert_tool_manifest_uses_workspace_version(&root, tool);

        let output = if tool == "sgpm" {
            Command::new(env!("CARGO_BIN_EXE_sgpm"))
                .arg("--version")
                .output()
                .unwrap_or_else(|err| panic!("failed to run built sgpm --version: {err}"))
        } else {
            Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
                .args([
                    "run",
                    "--quiet",
                    "-p",
                    tool,
                    "--bin",
                    tool,
                    "--",
                    "--version",
                ])
                .current_dir(&root)
                .output()
                .unwrap_or_else(|err| panic!("failed to run {tool} --version through cargo: {err}"))
        };
        assert!(
            output.status.success(),
            "{tool} --version failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let (version, hash) = parse_tool_version(tool, &output.stdout);
        assert_eq!(
            version, expected_version,
            "{tool} should report workspace.package.version"
        );
        signatures.insert(tool, format!("{version}|{hash}"));
    }

    let unique = signatures.values().collect::<BTreeSet<_>>();
    assert_eq!(
        unique.len(),
        1,
        "tool version/hash mismatch: {signatures:?}"
    );
}

#[test]
fn distribution_workflow_covers_native_macos_architectures() {
    let root = workspace_root();
    let workflow = fs::read_to_string(root.join(".github/workflows/toolchain-distribution.yml"))
        .expect("read toolchain distribution workflow");
    assert!(
        workflow.contains("macos-15") && workflow.contains("aarch64-apple-darwin"),
        "workflow should package the native Apple Silicon channel"
    );
    assert!(
        workflow.contains("macos-15-intel") && workflow.contains("x86_64-apple-darwin"),
        "workflow should package the native Intel macOS channel"
    );
    assert!(
        workflow.contains("SENGOO_DIST_TARGET"),
        "packaging must receive an explicit matrix target"
    );
    assert!(
        workflow.contains("needs: package-smoke") && workflow.contains("attest-build-provenance"),
        "release publication should wait for every platform and emit GitHub provenance attestations"
    );
    assert!(
        workflow.contains("prerelease: ${{ contains(github.ref_name, '-') }}"),
        "semver prerelease tags should publish as GitHub prereleases"
    );
}

#[test]
fn installers_detect_darwin_architecture_instead_of_assuming_x86_64() {
    let root = workspace_root();
    let shell = fs::read_to_string(root.join("scripts/install.sh")).expect("read install.sh");
    assert!(
        shell.contains("uname -m"),
        "install.sh should inspect host arch"
    );
    assert!(
        shell.contains("aarch64-apple-darwin"),
        "install.sh should select the Apple Silicon archive"
    );
    let powershell =
        fs::read_to_string(root.join("scripts/install.ps1")).expect("read install.ps1");
    assert!(
        powershell.contains("ProcessArchitecture"),
        "install.ps1 should inspect host architecture"
    );
    assert!(
        powershell.contains("aarch64-apple-darwin"),
        "install.ps1 should select the Apple Silicon archive"
    );
}

#[test]
fn distribution_workflow_smokes_explicit_upgrade_outside_checkout_without_a_real_tag() {
    let root = workspace_root();
    let workflow = fs::read_to_string(root.join(".github/workflows/toolchain-distribution.yml"))
        .expect("read toolchain distribution workflow");
    let prepare_release_feed = workflow_step_block(&workflow, "Prepare local release feed");
    let upgrade_posix = workflow_step_block(&workflow, "Upgrade package by version (POSIX)");
    let upgrade_windows = workflow_step_block(&workflow, "Upgrade package by version (Windows)");
    assert!(
        workflow.contains("Prepare local release feed"),
        "workflow should stage a local versioned release feed for deterministic dry-run upgrade smoke"
    );
    assert!(
        workflow.contains("Install package by version")
            && workflow.contains("Upgrade package by version"),
        "workflow should install and then upgrade through the documented pinned-version path"
    );
    assert!(
        workflow.contains("--base-url") && workflow.contains("-BaseUrl"),
        "workflow should exercise install-script version mode against a local release feed"
    );
    assert!(
        workflow.contains("SemanticVersion") && workflow.contains("$upgradePatch-upgrade-smoke"),
        "workflow should stage a semantically newer patch prerelease for the upgrade smoke"
    );
    assert!(
        prepare_release_feed.contains("$primaryBuildHash = $env:SENGOO_BUILD_HASH")
            && prepare_release_feed.contains(
                "$primaryBuildHash -notmatch '^[0-9a-fA-F]+$'"
            )
            && prepare_release_feed.contains("$firstNibble = [Convert]::ToInt32($primaryBuildHash.Substring(0, 1), 16)")
            && prepare_release_feed.contains(
                "$secondaryBuildHash = \"{0:x}{1}\" -f (($firstNibble + 1) % 16), $primaryBuildHash.Substring(1)"
            )
            && prepare_release_feed.contains("$secondaryBuildHash -eq $primaryBuildHash")
            && prepare_release_feed.contains("$env:SENGOO_BUILD_HASH = $secondaryBuildHash")
            && prepare_release_feed.contains("cargo build -p sgc -p sgpm -p sgfmt -p sglsp --release")
            && prepare_release_feed.contains(
                "./scripts/package-toolchain.ps1 -Version $upgradeVersion -OutputDir $upgradeOutputDir -NoBuild"
            ),
        "workflow should rebuild release tools with a deterministic secondary hex hash before packaging the synthetic upgrade archive with -NoBuild"
    );
    assert!(
        workflow.contains("outside-checkout"),
        "workflow should run the upgrade smoke from a temp workspace outside the repository checkout"
    );
    assert!(
        workflow.contains("primaryManifest.version")
            && workflow.contains("upgradedManifest.version"),
        "workflow should prove that installation content moves from the primary to secondary package manifest version"
    );
    assert!(
        upgrade_posix.contains("function Assert-InstalledToolVersions($Manifest, $BinDir)")
            && upgrade_posix.contains("tool_versions.PSObject.Properties[$tool].Value")
            && upgrade_posix.contains("payload does not match manifest")
            && upgrade_posix.contains("return $expectedSignature")
            && upgrade_posix.contains("$primarySignature = Assert-InstalledToolVersions $primaryManifest $bin")
            && upgrade_posix.contains("$upgradedSignature = Assert-InstalledToolVersions $upgradedManifest $bin")
            && upgrade_posix.contains("if ($upgradedSignature -eq $primarySignature)"),
        "POSIX upgrade smoke should compare installed tool payloads against manifest.tool_versions and require the upgraded signature to change"
    );
    assert!(
        upgrade_windows.contains("function Assert-InstalledToolVersions($Manifest, $BinDir)")
            && upgrade_windows.contains("tool_versions.PSObject.Properties[$tool].Value")
            && upgrade_windows.contains("payload does not match manifest")
            && upgrade_windows.contains("return $expectedSignature")
            && upgrade_windows.contains(
                "$primarySignature = Assert-InstalledToolVersions $primaryManifest (Join-Path $installRoot \"bin\")"
            )
            && upgrade_windows.contains("$upgradedSignature = Assert-InstalledToolVersions $upgradedManifest $bin")
            && upgrade_windows.contains("if ($upgradedSignature -eq $primarySignature)"),
        "Windows upgrade smoke should compare installed tool payloads against manifest.tool_versions and require the upgraded signature to change"
    );
}

#[test]
fn installers_support_local_release_feeds_for_deterministic_upgrade_smoke() {
    let root = workspace_root();
    let shell = fs::read_to_string(root.join("scripts/install.sh")).expect("read install.sh");
    assert!(
        shell.contains("[ -d \"$base_url\" ]"),
        "install.sh should treat a local release-feed directory as a valid version source"
    );
    assert!(
        shell.contains("cp \"$source\" \"$destination\""),
        "install.sh should copy versioned archives from a local release feed during dry-run smoke"
    );
    let powershell =
        fs::read_to_string(root.join("scripts/install.ps1")).expect("read install.ps1");
    assert!(
        powershell.contains("Test-Path -LiteralPath $BaseUrl"),
        "install.ps1 should treat a local release-feed directory as a valid version source"
    );
    assert!(
        powershell.contains("Copy-Item -LiteralPath $Source -Destination $Destination"),
        "install.ps1 should copy versioned archives from a local release feed during dry-run smoke"
    );
}

#[test]
fn compatibility_policy_freezes_edition_deprecation_and_supported_hosts() {
    let root = workspace_root();
    let policy = fs::read_to_string(root.join("docs/compatibility-policy.md"))
        .expect("read compatibility policy");

    for heading in [
        "## Source and edition policy",
        "## Deprecation window",
        "## Runtime and data schemas",
        "## Supported release hosts",
        "## Release support window",
    ] {
        assert!(
            policy.contains(heading),
            "compatibility policy should contain `{heading}`"
        );
    }
    assert!(
        policy.contains("edition = \"2026\"")
            && policy.contains("unsupported Sengoo edition")
            && policy.contains("at least one minor release"),
        "policy should freeze the 2026 edition rejection and deprecation window"
    );
    for target in [
        "x86_64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
    ] {
        assert!(
            policy.contains(target),
            "compatibility policy should list supported target `{target}`"
        );
    }
    assert!(
        policy.contains("latest prerelease line") && policy.contains("security or soundness"),
        "pre-1.0 support and emergency compatibility exceptions must be explicit"
    );
}

#[test]
fn compatibility_workflow_runs_retained_project_with_previous_and_current_toolchains() {
    let root = workspace_root();
    let workflow = fs::read_to_string(root.join(".github/workflows/compatibility.yml"))
        .expect("read compatibility workflow");
    let fixture = root.join("examples/compat/v0.1.0-rc.1");

    for relative in ["Sengoo.toml", "Sengoo.lock", "src/lib.sg", "tests/smoke.sg"] {
        assert!(
            fixture.join(relative).is_file(),
            "retained compatibility fixture should contain {relative}"
        );
    }
    for needle in [
        "v0.1.0-rc.1",
        "scripts/install.sh",
        "outside-checkout",
        "SGPM_SGC",
        "check --locked",
        "test --locked",
        "fmt --check --locked",
        "doc --locked",
        "build --locked",
        "compatibility-transcript",
    ] {
        assert!(
            workflow.contains(needle),
            "compatibility workflow should contain `{needle}`"
        );
    }
}

#[test]
fn native_safety_workflow_is_fail_closed_and_preserves_longevity_evidence() {
    let root = workspace_root();
    let workflow = fs::read_to_string(root.join(".github/workflows/native-safety.yml"))
        .expect("read native safety workflow");
    let script = fs::read_to_string(root.join("scripts/runtime-sanitizer-gate.sh"))
        .expect("read runtime sanitizer gate");
    let probe = root.join("tools/stdlib/tests/runtime_sanitizer_probe.c");

    assert!(probe.is_file(), "native sanitizer probe should be retained");
    for needle in [
        "bash scripts/runtime-sanitizer-gate.sh",
        "nightly-2026-07-01",
        "-Zsanitizer=address",
        "detect_leaks=1:halt_on_error=1",
        "--features native-bridge",
        "timeout 1800",
        "seq 1 10",
        "native-longevity-transcript",
        "if-no-files-found: error",
    ] {
        assert!(
            workflow.contains(needle),
            "native safety workflow should contain `{needle}`"
        );
    }
    for needle in [
        "set -euo pipefail",
        "-fsanitize=address,undefined",
        "detect_leaks=1:halt_on_error=1",
        "runtime_sanitizer_probe.c",
    ] {
        assert!(
            script.contains(needle),
            "runtime sanitizer gate should contain `{needle}`"
        );
    }
    assert!(
        !workflow.contains("continue-on-error"),
        "native safety jobs must fail closed"
    );
}

#[test]
fn performance_workflow_blocks_project_budgets_and_preserves_raw_evidence() {
    let root = workspace_root();
    let workflow = fs::read_to_string(root.join(".github/workflows/perf-smoke.yml"))
        .expect("read performance workflow")
        .replace("\r\n", "\n");
    let blocking_compile = "- name: Enforce compile wall-time, RSS, and regression budgets\n        shell: pwsh\n        run:";
    let blocking_resources =
        "- name: Enforce artifact, startup, CLI, and runtime budgets\n        shell: pwsh\n        run:";

    assert!(
        workflow.contains(blocking_compile),
        "compile and RSS budget step must not use continue-on-error"
    );
    assert!(
        workflow.contains(blocking_resources),
        "release resource budget step must not use continue-on-error"
    );
    for needle in [
        "-Mode hard -SkipAbsoluteTargets",
        "release_resource_gate.py",
        "runtime_loop.sg",
        "production-performance-evidence",
        "bench/results/*.json",
        "if-no-files-found: error",
        "Report cross-language absolute target status (informational)",
    ] {
        assert!(
            workflow.contains(needle),
            "performance workflow should contain `{needle}`"
        );
    }
    let smoke_release_gate = workflow_step_block(
        &workflow,
        "Smoke release_resource_gate against the real sgc",
    );
    for needle in [
        "--iterations 3",
        "runtime_loop.sg",
        "--max-full-build-ms 0.01",
        "if ($LASTEXITCODE -eq 0)",
        "expected a budget violation",
        "$global:LASTEXITCODE = 0",
    ] {
        assert!(
            smoke_release_gate.contains(needle),
            "release resource smoke should contain `{needle}`"
        );
    }
}

#[test]
fn frontend_baseline_points_at_an_exact_retained_ci_report() {
    let root = workspace_root();
    let baseline = fs::read_to_string(root.join("bench/frontend-memory-baseline.json"))
        .expect("read frontend baseline profile");
    let baseline: Value = serde_json::from_str(&baseline).expect("parse frontend baseline profile");
    let docs = fs::read_to_string(root.join("bench/FRONTEND_BASELINE.md"))
        .expect("read frontend baseline documentation");
    let retained_report_path = baseline["baseline_report_path"]
        .as_str()
        .expect("baseline should declare baseline_report_path");
    let retained_report = root.join(retained_report_path);
    let retained_report_bytes = fs::read(&retained_report)
        .unwrap_or_else(|err| panic!("read retained report {}: {err}", retained_report.display()));
    let retained_report_json: Value =
        serde_json::from_slice(&retained_report_bytes).expect("parse retained raw report");
    let expected_report_id = format!(
        "{}-advanced-pipeline",
        retained_report_json["generated_at_unix_ms"]
            .as_i64()
            .expect("retained report should expose generated_at_unix_ms")
    );

    assert!(
        retained_report.is_file(),
        "frontend baseline should retain the pinned advanced benchmark report in-repo"
    );
    assert!(
        retained_report_path.starts_with("bench/results/")
            && retained_report_path.ends_with("-advanced-pipeline.json"),
        "baseline profile should reference a retained local advanced benchmark report"
    );
    assert!(
        docs.contains(retained_report_path),
        "frontend baseline documentation should point at the retained local benchmark report"
    );
    assert!(
        baseline.get("bootstrap_pending_raw_ci_report").is_none()
            && baseline.get("provenance_status").is_none(),
        "final baseline should not retain bootstrap exceptions"
    );
    assert_eq!(
        baseline["baseline_report_id"].as_str(),
        Some(expected_report_id.as_str()),
        "baseline report id should match the raw producer report id"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(&retained_report_bytes)),
        baseline["baseline_report_sha256"]
            .as_str()
            .expect("baseline should pin retained raw report SHA-256"),
        "retained report bytes should match the pinned artifact hash"
    );
    let actions_run = baseline["baseline_actions_run"]
        .as_i64()
        .expect("baseline should record the Actions run");
    let artifact_id = baseline["baseline_artifact_id"]
        .as_i64()
        .expect("baseline should record the Actions artifact id");
    let artifact_digest = baseline["baseline_artifact_digest"]
        .as_str()
        .expect("baseline should record the Actions artifact digest");
    assert!(actions_run > 0 && artifact_id > 0);
    assert!(artifact_digest.starts_with("sha256:"));
    assert!(docs.contains(&actions_run.to_string()));
    assert!(docs.contains(&artifact_id.to_string()));
    assert!(docs.contains(artifact_digest));
    assert!(!docs.contains("pending the next perf-smoke artifact upload"));
    let notes = retained_report_json["notes"]
        .as_array()
        .expect("retained raw report should preserve notes");
    assert!(
        notes.iter().all(|note| {
            note.as_str()
                .map(|note| !note.contains("reconstructed"))
                .unwrap_or(true)
        }),
        "retained CI report should not contain bootstrap reconstruction notes"
    );
}
