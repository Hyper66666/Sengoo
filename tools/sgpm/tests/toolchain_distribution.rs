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

        let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
            .args(["run", "--quiet", "-p", tool, "--", "--version"])
            .current_dir(&root)
            .output()
            .unwrap_or_else(|err| panic!("failed to run {tool} --version through cargo: {err}"));
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
