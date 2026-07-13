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
