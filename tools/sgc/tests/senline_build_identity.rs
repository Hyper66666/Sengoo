mod common;

use common::source_sgc_command;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const SOURCE_REVISION: &str = "1de09ccafa7e8f182af68e82352e2d4be39496b0";
const TOOLCHAIN_VERSION: &str = "0.1.0";
const APPLICATION_VERSION: &str = "0.1.0";
const BUILD_MANIFEST_ID: &str = "1111111111111111111111111111111111111111111111111111111111111111";

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sengoo-worker-identity-{tag}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create build identity test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgc crate should live under tools/sgc")
        .to_path_buf()
}

fn worker_root() -> PathBuf {
    workspace_root().join("examples/realworld/senline-domain-worker")
}

fn generator_path() -> PathBuf {
    worker_root().join("scripts/generate-build-identity.ps1")
}

fn powershell() -> PathBuf {
    which::which("pwsh")
        .or_else(|_| which::which("powershell"))
        .expect("build identity generation requires PowerShell")
}

fn generate_identity(
    output: &Path,
    handshake: &Path,
    source_revision: &str,
    toolchain_version: &str,
    application_version: &str,
    build_manifest_id: &str,
) -> Output {
    Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(generator_path())
        .arg("-SourceRevision")
        .arg(source_revision)
        .arg("-ToolchainVersion")
        .arg(toolchain_version)
        .arg("-ApplicationVersion")
        .arg(application_version)
        .arg("-BuildManifestId")
        .arg(build_manifest_id)
        .arg("-OutputPath")
        .arg(output)
        .arg("-HandshakeOutputPath")
        .arg(handshake)
        .output()
        .expect("run build identity generator")
}

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} failed with {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn object(value: &Value) -> &Map<String, Value> {
    value.as_object().expect("handshake must be a JSON object")
}

fn expected_handshake(
    source_revision: &str,
    toolchain_version: &str,
    application_version: &str,
    build_manifest_id: &str,
) -> Value {
    serde_json::json!({
        "kind": "handshake",
        "protocol_version": 1,
        "sengoo_source_revision": source_revision,
        "toolchain_version": toolchain_version,
        "application_version": application_version,
        "build_manifest_id": build_manifest_id,
    })
}

fn externally_matches(handshake: &Value, expected: &Value) -> bool {
    let exact_fields = [
        "kind",
        "protocol_version",
        "sengoo_source_revision",
        "toolchain_version",
        "application_version",
        "build_manifest_id",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    object(handshake)
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        == exact_fields
        && handshake == expected
}

fn module_map(worker: &Path, identity_source: &Path) -> std::ffi::OsString {
    std::env::join_paths([
        format!(
            "senline_domain_worker={}",
            worker.join("src/lib.sg").display()
        ),
        format!("senline_build_identity={}", identity_source.display()),
        format!(
            "senline_facts_to_plan={}",
            worker
                .join("packages/senline-facts-to-plan/src/lib.sg")
                .display()
        ),
        format!(
            "sgframing={}",
            worker.join("packages/sgframing/src/lib.sg").display()
        ),
        format!(
            "sgjson_contract={}",
            worker.join("packages/sgjson-contract/src/lib.sg").display()
        ),
    ])
    .expect("encode worker identity module map")
}

fn unframe_exact(bytes: &[u8]) -> &[u8] {
    assert!(bytes.len() >= 4, "worker handshake needs a frame prefix");
    let len = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), len + 4, "worker emitted a surplus frame");
    &bytes[4..]
}

#[test]
fn generator_is_reproducible_and_rejects_invalid_identity_inputs() {
    let root = TempDir::new("generator");
    let first_source = root.path().join("first.sg");
    let first_handshake = root.path().join("first.json");
    let second_source = root.path().join("second.sg");
    let second_handshake = root.path().join("second.json");
    assert_success(
        "first build identity generation",
        &generate_identity(
            &first_source,
            &first_handshake,
            SOURCE_REVISION,
            TOOLCHAIN_VERSION,
            APPLICATION_VERSION,
            BUILD_MANIFEST_ID,
        ),
    );
    assert_success(
        "second build identity generation",
        &generate_identity(
            &second_source,
            &second_handshake,
            SOURCE_REVISION,
            TOOLCHAIN_VERSION,
            APPLICATION_VERSION,
            BUILD_MANIFEST_ID,
        ),
    );
    assert_eq!(
        fs::read(&first_source).unwrap(),
        fs::read(&second_source).unwrap()
    );
    assert_eq!(
        fs::read(&first_handshake).unwrap(),
        fs::read(&second_handshake).unwrap()
    );
    let generated: Value = serde_json::from_slice(&fs::read(&first_handshake).unwrap()).unwrap();
    assert!(externally_matches(
        &generated,
        &expected_handshake(
            SOURCE_REVISION,
            TOOLCHAIN_VERSION,
            APPLICATION_VERSION,
            BUILD_MANIFEST_ID,
        )
    ));

    let changed_id = "2".repeat(64);
    let changed_source = root.path().join("changed.sg");
    let changed_handshake = root.path().join("changed.json");
    assert_success(
        "changed build identity generation",
        &generate_identity(
            &changed_source,
            &changed_handshake,
            SOURCE_REVISION,
            TOOLCHAIN_VERSION,
            APPLICATION_VERSION,
            &changed_id,
        ),
    );
    assert_ne!(
        fs::read(&first_source).unwrap(),
        fs::read(&changed_source).unwrap()
    );
    assert!(externally_matches(
        &serde_json::from_slice(&fs::read(&changed_handshake).unwrap()).unwrap(),
        &expected_handshake(
            SOURCE_REVISION,
            TOOLCHAIN_VERSION,
            APPLICATION_VERSION,
            &changed_id,
        )
    ));

    let invalid_revision = generate_identity(
        &root.path().join("invalid-revision.sg"),
        &root.path().join("invalid-revision.json"),
        "not-a-revision",
        TOOLCHAIN_VERSION,
        APPLICATION_VERSION,
        BUILD_MANIFEST_ID,
    );
    assert!(!invalid_revision.status.success());
    let invalid_manifest = generate_identity(
        &root.path().join("invalid-manifest.sg"),
        &root.path().join("invalid-manifest.json"),
        SOURCE_REVISION,
        TOOLCHAIN_VERSION,
        APPLICATION_VERSION,
        "abc",
    );
    assert!(!invalid_manifest.status.success());
    let invalid_version = generate_identity(
        &root.path().join("invalid-version.sg"),
        &root.path().join("invalid-version.json"),
        SOURCE_REVISION,
        "0.1.0\"\nforged",
        APPLICATION_VERSION,
        BUILD_MANIFEST_ID,
    );
    assert!(!invalid_version.status.success());
}

#[test]
fn real_worker_embeds_generated_identity_but_external_manifest_remains_authoritative() {
    let root = TempDir::new("worker");
    let identity_source = root.path().join("senline-build-identity.sg");
    let handshake_path = root.path().join("handshake.json");
    let embedded_source_revision = "a".repeat(40);
    let embedded_manifest_id = "2".repeat(64);
    assert_success(
        "worker build identity generation",
        &generate_identity(
            &identity_source,
            &handshake_path,
            &embedded_source_revision,
            TOOLCHAIN_VERSION,
            APPLICATION_VERSION,
            &embedded_manifest_id,
        ),
    );

    let worker = worker_root();
    let executable = root.path().join(if cfg!(windows) {
        "identity-worker.exe"
    } else {
        "identity-worker"
    });
    let build = source_sgc_command()
        .arg("build")
        .arg(worker.join("src/main.sg"))
        .arg("--output")
        .arg(&executable)
        .arg("--force-rebuild")
        .current_dir(&worker)
        .env("SENGOO_MODULE_MAP", module_map(&worker, &identity_source))
        .output()
        .expect("build worker with generated identity");
    assert_success("worker with generated identity build", &build);

    let output = Command::new(&executable)
        .stdin(Stdio::null())
        .output()
        .expect("run worker identity handshake");
    assert_success("worker identity handshake", &output);
    assert!(output.stderr.is_empty());
    let reported_bytes = unframe_exact(&output.stdout);
    assert_eq!(
        reported_bytes,
        fs::read(&handshake_path).unwrap(),
        "worker handshake must be byte-identical to the external generated record"
    );
    let reported: Value = serde_json::from_slice(reported_bytes).unwrap();
    let external = expected_handshake(
        &embedded_source_revision,
        TOOLCHAIN_VERSION,
        APPLICATION_VERSION,
        &embedded_manifest_id,
    );
    assert!(externally_matches(&reported, &external));

    let mut mismatched_external = external;
    mismatched_external["build_manifest_id"] = Value::String("f".repeat(64));
    assert!(
        !externally_matches(&reported, &mismatched_external),
        "a worker self-report must not override the externally verified manifest"
    );
}
