//! Task 5.4: plan/request binding rejection surface for the framed worker.

mod common;

use common::source_sgc_command;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "senline-plan-binding-{tag}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp");
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
        .join("..")
        .join("..")
}

fn worker_root() -> PathBuf {
    workspace_root().join("examples/realworld/senline-domain-worker")
}

fn module_map(worker: &Path) -> std::ffi::OsString {
    // Match the realworld harness: PATH-style module=path entries.
    std::env::join_paths([
        format!(
            "senline_domain_worker={}",
            worker.join("src/lib.sg").display()
        ),
        format!(
            "senline_build_identity={}",
            worker
                .join("packages/senline-build-identity/src/lib.sg")
                .display()
        ),
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
    .expect("encode module map")
}

fn build_worker(temp: &TempDir) -> PathBuf {
    let worker = worker_root();
    let exe = temp.path().join(if cfg!(windows) {
        "senline_domain_worker.exe"
    } else {
        "senline_domain_worker"
    });
    let output = source_sgc_command()
        .arg("build")
        .arg(worker.join("src/main.sg"))
        .arg("-o")
        .arg(&exe)
        .args(["-O", "0", "--force-rebuild"])
        .current_dir(&worker)
        .env("SENGOO_MODULE_MAP", module_map(&worker))
        .output()
        .expect("build worker");
    assert!(
        output.status.success(),
        "worker build failed:\nstdout:{}\nstderr:{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    exe
}

fn framed(payload: &[u8]) -> Vec<u8> {
    let mut out = (payload.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(payload);
    out
}

fn run_worker(exe: &Path, input: &[u8]) -> Vec<u8> {
    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn worker");
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().expect("stdin");
        stdin.write_all(input).expect("write stdin");
    }
    let output = child.wait_with_output().expect("wait worker");
    assert!(
        output.status.success(),
        "worker failed status={:?} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn decode_frames(bytes: &[u8]) -> Vec<Value> {
    let mut values = Vec::new();
    let mut offset = 0usize;
    while offset + 4 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;
        let end = offset + len;
        assert!(end <= bytes.len(), "truncated frame");
        let value: Value = serde_json::from_slice(&bytes[offset..end]).expect("json frame");
        values.push(value);
        offset = end;
    }
    assert_eq!(offset, bytes.len(), "trailing bytes after frames");
    values
}

fn eligible_request() -> Value {
    let path = worker_root().join("fixtures/v1/cases/eligible-accept.request.json");
    serde_json::from_slice(&fs::read(path).expect("read fixture")).expect("parse fixture")
}

#[test]
fn worker_echoes_evaluation_id_and_rejects_invalid_binding_fields() {
    let temp = TempDir::new("bindings");
    let exe = build_worker(&temp);
    let base = eligible_request();

    // Host-owned binding integrity: the worker echoes a well-formed evaluation_id.
    // Final mismatch against host facts_binding remains Senline Rust authority.
    {
        let mut request = base.clone();
        request["context"]["evaluation_id"] = json!("ffffffffffffffffffffffffffffffff");
        let body = serde_json::to_vec(&request).expect("serialize");
        let frames = decode_frames(&run_worker(&exe, &framed(&body)));
        assert_eq!(
            frames[1]["kind"], "plan",
            "valid hex evaluation_id should plan"
        );
        assert_eq!(
            frames[1]["context"]["evaluation_id"],
            "ffffffffffffffffffffffffffffffff"
        );
    }

    let mut rejections: Vec<(&str, Value)> = Vec::new();
    {
        let mut v = base.clone();
        v["context"]["operation"] = json!("not-submit-envelope");
        rejections.push(("operation", v));
    }
    {
        let mut v = base.clone();
        v["context"]["operation_epoch"] = json!(9_007_199_254_740_992_i64);
        rejections.push(("operation_epoch", v));
    }
    {
        let mut v = base.clone();
        v["context"]["worker_generation"] = json!(-1);
        rejections.push(("worker_generation", v));
    }
    {
        let mut v = base.clone();
        v["context"]["contract_version"] = json!(2);
        rejections.push(("contract_version", v));
    }
    {
        let mut v = base.clone();
        v["facts"]["identifiers"]["envelope_ref"] = json!("");
        rejections.push(("identifier", v));
    }
    {
        let mut v = base.clone();
        v["facts"]["idempotency_status"] = json!("maybe");
        rejections.push(("unknown_enum", v));
    }
    {
        let mut v = base.clone();
        v["facts"]["source_device_capabilities"] = json!(["not_a_capability"]);
        rejections.push(("impossible_action", v));
    }
    {
        let mut v = base.clone();
        v["context"]["facts_binding"] = json!("not-hex");
        rejections.push(("facts_binding_shape", v));
    }
    {
        let mut v = base.clone();
        v["context"]["evaluation_id"] = json!("zz");
        rejections.push(("evaluation_id_shape", v));
    }

    for (label, request) in rejections {
        let body = serde_json::to_vec(&request).expect("serialize");
        let frames = decode_frames(&run_worker(&exe, &framed(&body)));
        assert!(
            frames[1]["kind"] == "error",
            "{label}: expected error rejection, got {}",
            frames[1]
        );
        let code = frames[1]["code"].as_str().unwrap_or("");
        assert!(
            !code.is_empty(),
            "{label}: empty error code in {}",
            frames[1]
        );
    }
}

#[test]
fn worker_rejects_oversized_output_path_by_enforcing_output_limit() {
    // The worker enforces an 8 KiB response ceiling before writing. Prove the
    // documented bound remains part of the public library surface used by hosts.
    let temp = TempDir::new("output-limit");
    let worker = worker_root();
    let probe = temp.path().join("output_limit.sg");
    fs::write(
        &probe,
        r#"
import senline_domain_worker;

def main() -> i64 {
    if worker_output_length_supported(8192) and not worker_output_length_supported(8193) { 0; } else { 1; };
}
"#,
    )
    .expect("write probe");
    let output = source_sgc_command()
        .arg("run")
        .arg(&probe)
        .args(["--force-rebuild"])
        .current_dir(&worker)
        .env("SENGOO_MODULE_MAP", module_map(&worker))
        .output()
        .expect("run probe");
    assert!(
        output.status.success(),
        "stdout:{}\nstderr:{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0));
}
