use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn distribution_release_publishes_one_sha_evidence_manifest() {
    let root = workspace_root();
    let workflow = fs::read_to_string(root.join(".github/workflows/toolchain-distribution.yml"))
        .expect("read toolchain distribution workflow");
    let generator = fs::read_to_string(root.join("scripts/generate-release-evidence.ps1"))
        .expect("read release evidence generator");

    assert!(
        workflow.contains("Generate one-SHA release evidence")
            && workflow.contains("scripts/generate-release-evidence.ps1")
            && workflow.contains("target/dist/release-evidence.json")
            && workflow.contains("-PreviousVersion $previousVersion")
            && workflow.contains("steps.archive-provenance.outputs['attestation-url']"),
        "tag publication should retain one-SHA archive, checksum, transition, workflow, and provenance evidence"
    );
    for required in [
        "x86_64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "source revision mismatch",
        "release eligible",
        "archive checksum mismatch",
        "complete_target_set",
        "published_upgrade",
        "compatibility_fixtures",
        "checksum_verified_rollback",
        "release-transition-linux-x86_64",
        "release-transition-windows-x86_64",
        "release-transition-macos-x86_64",
        "release-transition-macos-arm64",
    ] {
        assert!(
            generator.contains(required),
            "release evidence generator should enforce `{required}`"
        );
    }
}

#[test]
fn release_host_matrix_proves_http_tls_with_source_and_installed_sgc() {
    let root = workspace_root();
    let distribution =
        fs::read_to_string(root.join(".github/workflows/toolchain-distribution.yml"))
            .expect("read distribution workflow");
    let realworld = fs::read_to_string(root.join(".github/workflows/realworld-e2e.yml"))
        .expect("read realworld workflow");
    let common = fs::read_to_string(root.join("tools/sgc/tests/common/mod.rs"))
        .expect("read sgc integration helper");
    let test_name = "real_sgc_tls_router_keep_alive_streaming_composes_with_verified_ca";

    assert!(
        distribution.contains("Verify production HTTP TLS composition")
            && distribution.contains("http_server_tls_router_keep_alive_and_streaming_compose")
            && distribution.contains(test_name),
        "every package host should run runtime and real-sgc verified TLS composition"
    );
    assert!(
        realworld.contains("Verify installed sgc HTTP TLS composition")
            && realworld.contains("SENGOO_TEST_INSTALLED_SGC")
            && realworld.contains(test_name),
        "the installed realworld matrix should compile the TLS fixture with packaged sgc"
    );
    assert!(
        common.contains("var_os(\"SENGOO_TEST_INSTALLED_SGC\")"),
        "the integration harness should honor the explicit installed-sgc path"
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
        prepare_release_feed.contains(
            "$primaryBuildHash = \"${{ github.sha }}\".Substring(0, 12)"
        )
            && prepare_release_feed.contains(
                "$primaryBuildHash -notmatch '^[0-9a-fA-F]{12}$'"
            )
            && prepare_release_feed.contains("$firstNibble = [Convert]::ToInt32($primaryBuildHash.Substring(0, 1), 16)")
            && prepare_release_feed.contains(
                "$secondaryBuildHash = \"{0:x}{1}\" -f (($firstNibble + 1) % 16), $primaryBuildHash.Substring(1)"
            )
            && prepare_release_feed.contains("$secondaryBuildHash -eq $primaryBuildHash")
            && prepare_release_feed.contains("$env:SENGOO_BUILD_HASH = $secondaryBuildHash")
            && prepare_release_feed.contains("cargo build -p sgc -p sgpm -p sgfmt -p sglsp --release")
            && prepare_release_feed.contains(
                "cargo build -p sengoo-runtime --lib --features native-bridge --profile staticlib"
            )
            && prepare_release_feed.contains(
                "./scripts/package-toolchain.ps1 -Version $upgradeVersion -OutputDir $upgradeOutputDir -NoBuild"
            ),
        "workflow should rebuild release tools and the native runtime with a deterministic secondary hex hash before packaging the synthetic upgrade archive with -NoBuild"
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
fn tagged_distribution_requires_published_upgrade_and_rollback_on_every_host() {
    let root = workspace_root();
    let workflow = fs::read_to_string(root.join(".github/workflows/toolchain-distribution.yml"))
        .expect("read toolchain distribution workflow");
    let transition = workflow_step_block(&workflow, "Verify published upgrade and rollback");

    for needle in [
        "release-transition:",
        "needs: package-smoke",
        "sengoo-toolchain-${{ matrix.artifact }}",
        "0.2.0-rc.1",
        "0.2.0-rc.2",
        "0.2.0",
        "0.1.0-rc.1",
        "releases/download/v$previousVersion",
        "examples/compat/v0.1.0-rc.1",
        "examples/compat/v0.2.0-rc.1",
        "release-transition-${{ matrix.artifact }}",
        "needs: [package-smoke, release-transition]",
    ] {
        assert!(
            workflow.contains(needle),
            "tagged distribution workflow should contain `{needle}`"
        );
    }
    for needle in [
        "Invoke-ToolchainInstall $previousVersion $installRoot",
        "Invoke-ToolchainInstall $currentVersion $installRoot",
        "$rollbackSignature -ne $previousSignature",
        "Invoke-CompatibilityFixture \"previous\"",
        "Invoke-CompatibilityFixture \"upgraded\"",
        "Invoke-CompatibilityFixture \"rolled-back\"",
        "Sengoo.toml",
        "Sengoo.lock",
        "Get-FileHash",
        "SGPM_SGC",
        "@(\"check\", \"--locked\")",
        "@(\"test\", \"--locked\")",
        "@(\"fmt\", \"--check\", \"--locked\")",
        "@(\"doc\", \"--locked\")",
        "@(\"build\", \"--locked\")",
    ] {
        assert!(
            transition.contains(needle),
            "published transition step should contain `{needle}`"
        );
    }
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
        "## Surface stability classes",
        "## Source and edition policy",
        "## Deprecation window",
        "## Runtime and data schemas",
        "## Supported release hosts",
        "## Release support window",
        "## Public-input panic policy",
    ] {
        assert!(
            policy.contains(heading),
            "compatibility policy should contain `{heading}`"
        );
    }
    assert!(
        policy.contains("edition = \"2026\"")
            && policy.contains("unsupported Sengoo edition")
            && policy.contains("at least one minor release")
            && policy.contains("v0.2.x")
            && root.join("docs/migration-v0-1-to-v0-2.md").is_file()
            && root
                .join("examples/compat/v0.2.0-rc.1/Sengoo.toml")
                .is_file(),
        "policy should freeze the 2026 edition rejection, deprecation window, and v0.2 fixtures"
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
    let fixture_v02 = root.join("examples/compat/v0.2.0-rc.1");

    for relative in ["Sengoo.toml", "Sengoo.lock", "src/lib.sg", "tests/smoke.sg"] {
        assert!(
            fixture.join(relative).is_file(),
            "retained compatibility fixture should contain {relative}"
        );
        assert!(
            fixture_v02.join(relative).is_file(),
            "v0.2 compatibility fixture should contain {relative}"
        );
    }
    let smoke_v02 =
        fs::read_to_string(fixture_v02.join("tests/smoke.sg")).expect("read v0.2 smoke fixture");
    assert!(
        smoke_v02.contains("import compat_v0_2_0_rc_1"),
        "v0.2 smoke must import compat_v0_2_0_rc_1, got:\n{smoke_v02}"
    );
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
        "runtime_args=(--runtime-mode \"$runtime_mode\")",
        "run_loop \"current\" \"$GITHUB_WORKSPACE/target/release\" \"source-development\"",
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

fn distribution_temp_dir(tag: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sengoo-distribution-{tag}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create distribution test directory");
    path
}

fn distribution_manifest() -> Value {
    json!({
        "schema_version": 2,
        "version": "0.1.0-repro-test",
        "target": "x86_64-pc-windows-msvc",
        "build_hash": "111111111111",
        "source_revision": "1111111111111111111111111111111111111111",
        "source_dirty": false,
        "artifact_provenance": "built-by-package-toolchain",
        "release_eligible": true,
        "build_manifest_id": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        "tools": ["sgc", "sgpm", "sgfmt", "sglsp"],
        "tool_versions": {
            "sgc": "sgc 0.1.0 (111111111111)",
            "sgpm": "sgpm 0.1.0 (111111111111)",
            "sgfmt": "sgfmt 0.1.0 (111111111111)",
            "sglsp": "sglsp 0.1.0 (111111111111)"
        },
        "stdlib_modules": ["io.sg", "json.sg"],
        "runtime_sources": ["runtime.c", "runtime_json.c"],
        "native_runtime": {
            "abi_version": 1,
            "target": "x86_64-pc-windows-msvc",
            "library": "share/sengoo/runtime/x86_64-pc-windows-msvc/sengoo_runtime.lib",
            "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "link_args": ["kernel32.lib", "bcrypt.lib"],
            "dynamic_dependencies": ["vcruntime140.dll", "ucrtbase.dll"]
        },
        "payload_checksum_file": "payloads.sha256",
        "payloads": [
            {
                "path": "bin/sgc.exe",
                "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "size": 100
            },
            {
                "path": "share/sengoo/runtime/x86_64-pc-windows-msvc/sengoo_runtime.lib",
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "size": 200
            }
        ],
        "archive_file": "sengoo-0.1.0-repro-test-x86_64-pc-windows-msvc.zip",
        "checksum_file": "sengoo-0.1.0-repro-test-x86_64-pc-windows-msvc.zip.sha256",
        "runner_os": "Windows",
        "runner_image": "windows-2025",
        "smoke_evidence": "build A",
        "license_included": true,
        "generated_at_utc": "2026-07-15T00:00:00Z"
    })
}

fn powershell() -> &'static str {
    if cfg!(windows) {
        "powershell.exe"
    } else {
        "pwsh"
    }
}

fn run_manifest_comparator(left: &Path, right: &Path, output: &Path) -> std::process::Output {
    Command::new(powershell())
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-File"])
        .arg(
            workspace_root()
                .join("scripts")
                .join("compare-distribution-manifests.ps1"),
        )
        .arg("-LeftManifest")
        .arg(left)
        .arg("-RightManifest")
        .arg(right)
        .arg("-OutputDir")
        .arg(output)
        .output()
        .expect("run distribution manifest comparator")
}

fn write_json(path: &Path, value: &Value) {
    fs::write(
        path,
        serde_json::to_vec_pretty(value).expect("serialize JSON"),
    )
    .expect("write JSON fixture");
}

fn run_package_toolchain(
    root: &Path,
    output_dir: &Path,
    cargo_target_dir: &Path,
    environment: &[(&str, &str)],
) -> std::process::Output {
    let mut command = Command::new(powershell());
    command
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-File"])
        .arg(
            workspace_root()
                .join("scripts")
                .join("package-toolchain.ps1"),
        )
        .arg("-NoBuild")
        .arg("-RepoRoot")
        .arg(root)
        .arg("-OutputDir")
        .arg(output_dir)
        .arg("-CargoTargetDir")
        .arg(cargo_target_dir);
    for (name, value) in environment {
        command.env(name, value);
    }
    command.output().expect("run package-toolchain.ps1")
}

#[test]
fn distribution_packages_and_verifies_the_target_native_runtime_payload() {
    let root = workspace_root();
    let package = fs::read_to_string(root.join("scripts/package-toolchain.ps1"))
        .expect("read package-toolchain.ps1");
    assert!(
        package.contains("sengoo_runtime.lib") && package.contains("libsengoo_runtime.a"),
        "packaging must select the target-native runtime static library"
    );
    assert!(
        package.contains("schema_version = 2")
            && package.contains("native_runtime")
            && package.contains("abi_version = 1")
            && package.contains("build_manifest_id"),
        "manifest v2 must bind runtime ABI, target, payload, and build identity"
    );
    assert!(
        package.contains("payloads.sha256") && package.contains("Get-FileHash"),
        "packaging must emit per-file SHA-256 evidence"
    );
    assert!(
        package.contains("source_revision = $sourceRevision")
            && package.contains("source_dirty")
            && package.contains("artifact_provenance")
            && package.contains("release_eligible"),
        "packaging must distinguish source identity from unverified prebuilt artifacts"
    );

    let powershell =
        fs::read_to_string(root.join("scripts/install.ps1")).expect("read install.ps1");
    assert!(
        powershell.contains("payloads.sha256")
            && powershell.contains("Get-FileHash")
            && powershell.contains("payload checksum mismatch"),
        "PowerShell installation must verify every packaged payload before copying"
    );

    let shell = fs::read_to_string(root.join("scripts/install.sh")).expect("read install.sh");
    assert!(
        shell.contains("payloads.sha256")
            && shell.contains("sha256sum -c")
            && shell.contains("shasum -a 256 -c"),
        "POSIX installation must verify every packaged payload before copying"
    );

    let workflow = fs::read_to_string(root.join(".github/workflows/toolchain-distribution.yml"))
        .expect("read toolchain-distribution.yml");
    assert!(
        !workflow.contains("package-toolchain.ps1 -Version $version -NoBuild")
            && workflow.contains("release_eligible"),
        "release packaging must build its own artifacts and reject non-release provenance"
    );
}

#[test]
fn installers_allow_only_the_checksummed_v010_rc1_legacy_archive() {
    let root = workspace_root();
    let powershell =
        fs::read_to_string(root.join("scripts/install.ps1")).expect("read install.ps1");
    let shell = fs::read_to_string(root.join("scripts/install.sh")).expect("read install.sh");

    for installer in [&powershell, &shell] {
        assert!(
            installer.contains("0.1.0-rc.1")
                && installer.contains("predates payloads.sha256")
                && installer.contains("verified release archive SHA-256")
                && installer.contains("archive does not contain payloads.sha256"),
            "installer should allow only the named legacy release while keeping new archives fail-closed"
        );
    }
}

#[test]
fn distribution_workflow_compares_independent_windows_and_linux_builds() {
    let root = workspace_root();
    let workflow = fs::read_to_string(root.join(".github/workflows/toolchain-distribution.yml"))
        .expect("read toolchain-distribution.yml");
    let package_script = fs::read_to_string(root.join("scripts/package-toolchain.ps1"))
        .expect("read package-toolchain.ps1");

    assert!(
        workflow.contains("reproducible: true")
            && workflow.contains("sengoo-cargo-package-a-")
            && workflow.contains("sengoo-cargo-package-b-"),
        "Windows and Linux packaging must use independent Cargo target directories"
    );
    assert!(
        package_script.contains("--remap-path-prefix=")
            && package_script.contains("CARGO_ENCODED_RUSTFLAGS"),
        "package-toolchain must remap source and target paths for reproducible artifacts"
    );
    assert!(
        workflow.contains("compare-distribution-manifests.ps1")
            && workflow.contains("target/repro-evidence/")
            && workflow.contains("normalized-a.json")
            && workflow.contains("comparison.json"),
        "independent manifests must be normalized and compared with retained evidence"
    );
    assert!(
        workflow.contains("Install reproducibility build B (POSIX)")
            && workflow.contains("Install reproducibility build B (Windows)")
            && workflow.contains("target/install-smoke-repro-b"),
        "both independently built archives must pass checksum-verifying installation"
    );
    assert!(
        workflow.contains("Upload reproducibility evidence")
            && workflow.contains("sengoo-reproducibility-${{ matrix.artifact }}"),
        "build B and the normalized comparison must be retained outside release publication inputs"
    );
}

#[test]
fn distribution_manifest_comparator_allows_only_documented_provenance_differences() {
    let root = distribution_temp_dir("comparator-allowed");
    let left_path = root.join("left.json");
    let right_path = root.join("right.json");
    let evidence_dir = root.join("evidence");
    let left = distribution_manifest();
    let mut right = left.clone();
    right["generated_at_utc"] = json!("2026-07-15T00:01:00Z");
    right["runner_os"] = json!("Windows-retry");
    right["runner_image"] = json!("windows-2025.1");
    right["smoke_evidence"] = json!("build B");
    right["tools"].as_array_mut().unwrap().reverse();
    right["stdlib_modules"].as_array_mut().unwrap().reverse();
    right["runtime_sources"].as_array_mut().unwrap().reverse();
    right["native_runtime"]["dynamic_dependencies"]
        .as_array_mut()
        .unwrap()
        .reverse();
    right["payloads"].as_array_mut().unwrap().reverse();
    write_json(&left_path, &left);
    write_json(&right_path, &right);

    let output = run_manifest_comparator(&left_path, &right_path, &evidence_dir);
    assert!(
        output.status.success(),
        "comparator rejected documented differences\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let comparison: Value = serde_json::from_slice(
        &fs::read(evidence_dir.join("comparison.json")).expect("comparison evidence"),
    )
    .expect("parse comparison evidence");
    assert_eq!(comparison["status"], "reproducible");
    assert_eq!(
        comparison["left"]["normalized_sha256"],
        comparison["right"]["normalized_sha256"]
    );
    assert_eq!(
        comparison["excluded_fields"],
        json!([
            "generated_at_utc",
            "runner_os",
            "runner_image",
            "smoke_evidence"
        ])
    );
    assert_eq!(
        comparison["excluded_differences"]
            .as_array()
            .expect("excluded differences")
            .len(),
        4
    );
    assert!(evidence_dir.join("normalized-a.json").is_file());
    assert!(evidence_dir.join("normalized-b.json").is_file());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn distribution_manifest_comparator_rejects_identity_schema_and_path_drift() {
    type Mutation = (&'static str, &'static str, Box<dyn Fn(&mut Value)>);

    let mutations: Vec<Mutation> = vec![
        (
            "payload hash drift",
            "payloads",
            Box::new(|value| value["payloads"][0]["sha256"] = json!("c".repeat(64))),
        ),
        (
            "runtime ABI drift",
            "native_runtime.abi_version",
            Box::new(|value| value["native_runtime"]["abi_version"] = json!(2)),
        ),
        (
            "ordered link argument drift",
            "native_runtime.link_args",
            Box::new(|value| {
                value["native_runtime"]["link_args"]
                    .as_array_mut()
                    .unwrap()
                    .reverse()
            }),
        ),
        (
            "dynamic dependency drift",
            "native_runtime.dynamic_dependencies",
            Box::new(|value| {
                value["native_runtime"]["dynamic_dependencies"][0] = json!("other.dll")
            }),
        ),
        (
            "source revision drift",
            "source_revision",
            Box::new(|value| value["source_revision"] = json!("2".repeat(40))),
        ),
        (
            "tool version drift",
            "tool_versions",
            Box::new(|value| value["tool_versions"]["sgc"] = json!("sgc 0.1.1 (111111111111)")),
        ),
        (
            "unknown top-level field",
            "unknown manifest field",
            Box::new(|value| value["unexpected"] = json!(true)),
        ),
        (
            "missing required field",
            "missing manifest field",
            Box::new(|value| {
                value.as_object_mut().unwrap().remove("license_included");
            }),
        ),
        (
            "absolute payload path",
            "normalized relative path",
            Box::new(|value| value["payloads"][0]["path"] = json!("C:/checkout/sgc.exe")),
        ),
        (
            "duplicate payload path",
            "duplicate payload path",
            Box::new(|value| {
                value["payloads"][1]["path"] = value["payloads"][0]["path"].clone();
            }),
        ),
    ];

    for (label, expected_error, mutate) in mutations {
        let root = distribution_temp_dir(&label.replace(' ', "-"));
        let left_path = root.join("left.json");
        let right_path = root.join("right.json");
        let evidence_dir = root.join("evidence");
        let left = distribution_manifest();
        let mut right = left.clone();
        mutate(&mut right);
        write_json(&left_path, &left);
        write_json(&right_path, &right);

        let output = run_manifest_comparator(&left_path, &right_path, &evidence_dir);
        assert!(
            !output.status.success(),
            "{label} unexpectedly compared as reproducible"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected_error),
            "{label} did not report `{expected_error}`\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            stderr
        );

        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn package_toolchain_rejects_source_revision_override_that_is_not_head() {
    let temp = distribution_temp_dir("package-source-override");
    let output = run_package_toolchain(
        &workspace_root(),
        &temp.join("dist"),
        &temp.join("cargo-target"),
        &[(
            "SENGOO_SOURCE_REVISION",
            "ffffffffffffffffffffffffffffffffffffffff",
        )],
    );
    assert!(
        !output.status.success(),
        "mismatched source revision must fail"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("must equal repository HEAD"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!temp.join("dist").exists());
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn package_toolchain_rejects_a_target_that_does_not_match_the_host() {
    let temp = distribution_temp_dir("package-host-target");
    let wrong_target = if cfg!(windows) {
        "x86_64-unknown-linux-gnu"
    } else {
        "x86_64-pc-windows-msvc"
    };
    let output = run_package_toolchain(
        &workspace_root(),
        &temp.join("dist"),
        &temp.join("cargo-target"),
        &[("SENGOO_DIST_TARGET", wrong_target)],
    );
    assert!(
        !output.status.success(),
        "cross-host target spoofing must fail"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("does not match host target"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!temp.join("dist").exists());
    let _ = fs::remove_dir_all(temp);
}
