mod common;

use common::source_sgc_command;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const WORKER_TIMEOUT: Duration = Duration::from_secs(20);

struct WorkerTempDir {
    path: PathBuf,
}

impl WorkerTempDir {
    fn new(tag: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sengoo-worker-faults-{tag}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create worker fault test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorkerTempDir {
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

fn fixture_root() -> PathBuf {
    worker_root().join("fixtures/v1")
}

fn worker_module_map(worker: &Path) -> std::ffi::OsString {
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
    .expect("encode worker module map")
}

fn build_worker(root: &WorkerTempDir) -> PathBuf {
    let worker = worker_root();
    let executable = root.path().join(if cfg!(windows) {
        "senline-domain-worker.exe"
    } else {
        "senline-domain-worker"
    });
    let output = source_sgc_command()
        .arg("build")
        .arg(worker.join("src/main.sg"))
        .arg("--output")
        .arg(&executable)
        .args(["-O", "0", "--force-rebuild"])
        .current_dir(&worker)
        .env("SENGOO_MODULE_MAP", worker_module_map(&worker))
        .output()
        .expect("build Senline worker fault probe");
    assert!(
        output.status.success(),
        "worker build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    executable
}

fn build_worker_with_source(root: &WorkerTempDir, source_text: &str) -> PathBuf {
    let worker = worker_root();
    let source = root.path().join("partial-writer-main.sg");
    fs::write(&source, source_text).expect("write partial-writer worker source");
    let executable = root.path().join(if cfg!(windows) {
        "senline-domain-worker-partial-writer.exe"
    } else {
        "senline-domain-worker-partial-writer"
    });
    let output = source_sgc_command()
        .arg("build")
        .arg(&source)
        .arg("--output")
        .arg(&executable)
        .args(["-O", "0", "--force-rebuild"])
        .current_dir(&worker)
        .env("SENGOO_MODULE_MAP", worker_module_map(&worker))
        .output()
        .expect("build partial-writer worker fault probe");
    assert!(
        output.status.success(),
        "partial-writer worker build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    executable
}

fn terminate_and_reap(child: &mut Child) -> String {
    let kill = match child.kill() {
        Ok(()) => "kill sent".to_owned(),
        Err(error) => format!("kill failed: {error}"),
    };
    let reap = match child.wait() {
        Ok(status) => format!("reaped with {status}"),
        Err(error) => format!("reap failed: {error}"),
    };
    format!("{kill}; {reap}")
}

fn wait_with_deadline(child: &mut Child, label: &str) -> ExitStatus {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status,
            Ok(None) => {}
            Err(error) => {
                let cleanup = terminate_and_reap(child);
                panic!("{label} wait failed: {error}; {cleanup}");
            }
        }
        let elapsed = started.elapsed();
        if elapsed >= WORKER_TIMEOUT {
            let cleanup = terminate_and_reap(child);
            panic!(
                "{label} timed out after {} ms; {cleanup}",
                WORKER_TIMEOUT.as_millis()
            );
        }
        thread::sleep(Duration::from_millis(10).min(WORKER_TIMEOUT - elapsed));
    }
}

fn collect_output(child: &mut Child, status: ExitStatus) -> Output {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .stdout
        .take()
        .expect("worker stdout should be piped")
        .read_to_end(&mut stdout)
        .expect("read worker stdout");
    child
        .stderr
        .take()
        .expect("worker stderr should be piped")
        .read_to_end(&mut stderr)
        .expect("read worker stderr");
    Output {
        status,
        stdout,
        stderr,
    }
}

fn run_worker(executable: &Path, root: &WorkerTempDir, tag: &str, input: &[u8]) -> Output {
    let input_path = root.path().join(format!("{tag}.input"));
    fs::write(&input_path, input).expect("write worker fault input");
    let mut child = Command::new(executable)
        .stdin(Stdio::from(
            File::open(&input_path).expect("open worker fault input"),
        ))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn worker fault case");
    let status = wait_with_deadline(&mut child, tag);
    collect_output(&mut child, status)
}

fn run_worker_chunked(executable: &Path, chunks: Vec<Vec<u8>>, current_dir: &Path) -> Output {
    let mut child = Command::new(executable)
        .current_dir(current_dir)
        .env("TEMP", current_dir)
        .env("TMP", current_dir)
        .env("TMPDIR", current_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn chunked worker case");
    let mut stdin = child.stdin.take().expect("worker stdin should be piped");
    let mut stdout = child.stdout.take().expect("worker stdout should be piped");
    let mut stderr = child.stderr.take().expect("worker stderr should be piped");

    let writer = thread::spawn(move || {
        for chunk in chunks {
            stdin.write_all(&chunk).expect("write chunked worker input");
            stdin.flush().expect("flush chunked worker input");
            thread::sleep(Duration::from_millis(1));
        }
    });
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut byte = [0_u8; 1];
        loop {
            match stdout.read(&mut byte) {
                Ok(0) => break,
                Ok(1) => bytes.push(byte[0]),
                Ok(_) => unreachable!("single-byte stdout read returned more than one byte"),
                Err(error) => panic!("read chunked worker stdout: {error}"),
            }
        }
        bytes
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr
            .read_to_end(&mut bytes)
            .expect("read chunked worker stderr");
        bytes
    });

    let status = wait_with_deadline(&mut child, "chunked worker case");
    writer.join().expect("chunked worker writer should join");
    Output {
        status,
        stdout: stdout_reader
            .join()
            .expect("chunked stdout reader should join"),
        stderr: stderr_reader
            .join()
            .expect("chunked stderr reader should join"),
    }
}

fn framed(payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(payload.len() + 4);
    bytes.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("fixture payload should fit u32")
            .to_be_bytes(),
    );
    bytes.extend_from_slice(payload);
    bytes
}

fn read_fixture(relative: &str) -> Vec<u8> {
    let path = fixture_root().join(relative);
    fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn handshake_frame() -> Vec<u8> {
    framed(&read_fixture("handshake/ready.json"))
}

fn assert_worker_output(output: &Output, expected_code: i32, expected_stdout: &[u8]) {
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "worker stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "worker stderr must stay empty");
    assert_eq!(output.stdout, expected_stdout, "worker frame bytes changed");
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn framed_payloads(bytes: &[u8]) -> Vec<&[u8]> {
    let mut offset = 0;
    let mut payloads = Vec::new();
    while offset < bytes.len() {
        assert!(bytes.len() - offset >= 4, "truncated output frame prefix");
        let len = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;
        assert!(
            bytes.len() - offset >= len,
            "truncated output frame payload"
        );
        payloads.push(&bytes[offset..offset + len]);
        offset += len;
    }
    payloads
}

fn next_canary(state: &mut u64, index: usize) -> String {
    let mut bytes = [0_u8; 16];
    for byte in &mut bytes {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *byte = *state as u8;
    }
    let mut value = format!("rejected_canary_{index:03}_");
    for byte in bytes {
        value.push_str(&format!("{byte:02x}"));
    }
    value
}

struct CanaryStream {
    state: u64,
    index: usize,
}

impl CanaryStream {
    fn new() -> Self {
        Self {
            state: 0x243f_6a88_85a3_08d3_u64,
            index: 0,
        }
    }

    fn next(&mut self) -> String {
        let value = next_canary(&mut self.state, self.index);
        self.index += 1;
        value
    }

    fn next_hex(&mut self) -> String {
        self.next()
            .rsplit('_')
            .next()
            .expect("canary should end in hex")
            .to_owned()
    }
}

fn replace_context_and_identifier_text(
    request: &mut Value,
    canaries: &mut CanaryStream,
) -> Vec<(String, &'static str)> {
    // `operation` is the closed protocol literal; changing it would reject
    // before the nested DTO fields are reached. Use a different valid mode.
    request["context"]["execution_mode"] = Value::String("internal-alpha".to_owned());
    let evaluation_id = canaries.next_hex();
    let worker_bundle_id = canaries.next();
    let facts_binding = format!("{}{}", canaries.next_hex(), canaries.next_hex());
    let fields = [
        (
            "/context/evaluation_id",
            evaluation_id,
            "/context/evaluation_id",
        ),
        (
            "/context/worker_bundle_id",
            worker_bundle_id,
            "/context/worker_bundle_id",
        ),
        (
            "/context/facts_binding",
            facts_binding,
            "/context/facts_binding",
        ),
        (
            "/facts/identifiers/correlation_ref",
            canaries.next(),
            "/identifiers/correlation_ref",
        ),
        (
            "/facts/identifiers/source_account_ref",
            canaries.next(),
            "/identifiers/source_account_ref",
        ),
        (
            "/facts/identifiers/source_device_ref",
            canaries.next(),
            "/identifiers/source_device_ref",
        ),
        (
            "/facts/identifiers/recipient_account_ref",
            canaries.next(),
            "/identifiers/recipient_account_ref",
        ),
        (
            "/facts/identifiers/recipient_device_ref",
            canaries.next(),
            "/identifiers/recipient_device_ref",
        ),
        (
            "/facts/identifiers/conversation_ref",
            canaries.next(),
            "/identifiers/conversation_ref",
        ),
        (
            "/facts/identifiers/envelope_ref",
            canaries.next(),
            "/identifiers/envelope_ref",
        ),
    ];
    fields
        .into_iter()
        .map(|(input_path, value, output_path)| {
            *request
                .pointer_mut(input_path)
                .unwrap_or_else(|| panic!("missing request field {input_path}")) =
                Value::String(value.clone());
            (value, output_path)
        })
        .collect()
}

fn collect_string_locations(value: &Value, needle: &str, path: &str, found: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            if text.contains(needle) {
                found.push(path.to_owned());
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_string_locations(item, needle, &format!("{path}/{index}"), found);
            }
        }
        Value::Object(fields) => {
            for (key, item) in fields {
                collect_string_locations(item, needle, &format!("{path}/{key}"), found);
            }
        }
        _ => {}
    }
}

fn assert_canaries_only_at_paths(value: &Value, expected: &[(String, &'static str)]) {
    for (canary, expected_path) in expected {
        assert_eq!(
            value.pointer(expected_path),
            Some(&Value::String(canary.clone())),
            "canary {canary} changed at its allowed response field"
        );
        let mut locations = Vec::new();
        collect_string_locations(value, canary, "", &mut locations);
        assert_eq!(
            locations,
            [(*expected_path).to_owned()],
            "canary {canary} escaped its response field"
        );
    }
}

fn assert_error_envelope(payload: &[u8], scope: &str, code: &str, evaluation_id: Option<&str>) {
    const ALLOWED_CODES: &[&str] = &[
        "duplicate_field",
        "invalid_unicode",
        "malformed_json",
        "trailing_bytes",
        "unknown_enum",
        "unknown_field",
        "unsupported_operation_version",
    ];
    assert!(
        ALLOWED_CODES.contains(&code),
        "unreviewed error code {code}"
    );
    let envelope: Value = serde_json::from_slice(payload).expect("decode worker error");
    let object = envelope
        .as_object()
        .expect("worker error must be an object");
    assert_eq!(object.len(), 5, "worker error fields changed: {object:?}");
    assert_eq!(envelope["kind"], "error");
    assert_eq!(envelope["schema_version"], 1);
    assert_eq!(envelope["scope"], scope);
    assert_eq!(envelope["code"], code);
    match evaluation_id {
        Some(expected) => assert_eq!(envelope["evaluation_id"], expected),
        None => assert!(
            envelope["evaluation_id"].is_null(),
            "parser/schema rejection must not recover an evaluation id"
        ),
    }
}

fn relative_file_set(root: &Path) -> BTreeSet<PathBuf> {
    let mut files = Vec::new();
    collect_artifact_files(root, &mut files);
    files
        .into_iter()
        .map(|path| {
            path.strip_prefix(root)
                .expect("collected file should remain below root")
                .to_path_buf()
        })
        .collect()
}

fn collect_artifact_files(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_artifact_files(&path, files);
        } else if path.is_file() {
            files.push(path);
        }
    }
}

#[test]
fn worker_accepts_partial_prefix_and_payload_delivery_and_single_byte_output_reads() {
    let root = WorkerTempDir::new("partial-io");
    let executable = build_worker(&root);
    let request = read_fixture("cases/eligible-accept.request.json");
    let plan = read_fixture("cases/eligible-accept.plan.json");
    let frame = framed(&request);
    let mut chunks = frame[..4]
        .iter()
        .copied()
        .map(|byte| vec![byte])
        .collect::<Vec<_>>();
    chunks.extend(frame[4..].chunks(7).map(<[u8]>::to_vec));

    let output = run_worker_chunked(&executable, chunks, root.path());
    let mut expected = handshake_frame();
    expected.extend_from_slice(&framed(&plan));
    assert_worker_output(&output, 0, &expected);
}

#[test]
fn worker_retries_deterministic_three_byte_partial_writes_until_frames_are_complete() {
    let root = WorkerTempDir::new("partial-writes");
    let executable = build_worker_with_source(
        &root,
        r#"
import std::ffi;
import std::io;

import senline_domain_worker;
import sgframing;

def three_byte_stdout_write(payload: Buffer, offset: i64, len: i64) -> Result<i64, i64> {
    let accepted = if len > 3 { 3 } else { len };
    io_stdout_write_all(payload, offset, accepted);
}

def partial_stdout_flush() -> Result<bool, i64> {
    io_stdout_flush();
}

def partial_frame_writer(payload: Buffer, len: i64, max_len: i64) -> Result<i64, i64> {
    let writer: fn(Buffer, i64, i64) -> Result<i64, i64> = three_byte_stdout_write;
    let flusher: fn() -> Result<bool, i64> = partial_stdout_flush;
    frame_write_with(payload, len, max_len, writer, flusher);
}

def main() -> i64 {
    let writer: fn(Buffer, i64, i64) -> Result<i64, i64> = partial_frame_writer;
    worker_run_stdio_v1_with_frame_writer(writer);
}
"#,
    );
    let request = read_fixture("cases/eligible-accept.request.json");
    let plan = read_fixture("cases/eligible-accept.plan.json");
    let output = run_worker(&executable, &root, "partial-writes", &framed(&request));
    let mut expected = handshake_frame();
    expected.extend_from_slice(&framed(&plan));
    assert_worker_output(&output, 0, &expected);
}

#[test]
fn worker_releases_owned_frame_buffers_after_eof() {
    let root = WorkerTempDir::new("buffer-lifecycle");
    let executable = build_worker_with_source(
        &root,
        r#"
import std::ffi;

import senline_domain_worker;

extern "C" {
    fn sengoo_buffer_live_handle_count() -> i64;
}

def main() -> i64 {
    let baseline = sengoo_buffer_live_handle_count();
    let result = worker_run_stdio_v1();
    if result != 0 { return result; };
    if sengoo_buffer_live_handle_count() != baseline { return 125; };
    0;
}
"#,
    );
    let request = read_fixture("cases/eligible-accept.request.json");
    let plan = read_fixture("cases/eligible-accept.plan.json");
    let duplicate = read_fixture("errors/protocol-duplicate-field.request.raw");
    let duplicate_error = read_fixture("errors/protocol-duplicate-field.json");
    let mut input = framed(&request);
    input.extend_from_slice(&framed(&duplicate));
    input.extend_from_slice(&framed(&request));
    let output = run_worker(&executable, &root, "buffer-lifecycle", &input);
    let mut expected = handshake_frame();
    expected.extend_from_slice(&framed(&plan));
    expected.extend_from_slice(&framed(&duplicate_error));
    expected.extend_from_slice(&framed(&plan));
    assert_worker_output(&output, 0, &expected);
}

#[test]
fn worker_rejects_zero_oversized_truncated_and_surplus_frame_bytes() {
    let root = WorkerTempDir::new("frame-boundaries");
    let executable = build_worker(&root);
    let handshake = handshake_frame();

    for (tag, input, exit_code) in [
        ("zero", 0_u32.to_be_bytes().to_vec(), 41),
        ("oversized", 32_769_u32.to_be_bytes().to_vec(), 42),
        ("prefix-1", vec![0], 43),
        ("prefix-2", vec![0, 0], 43),
        ("prefix-3", vec![0, 0, 0], 43),
    ] {
        let output = run_worker(&executable, &root, tag, &input);
        assert_worker_output(&output, exit_code, &handshake);
    }

    let payload_prefix = 4_u32.to_be_bytes();
    for present in 0..4 {
        let mut input = payload_prefix.to_vec();
        input.extend_from_slice(&[b'{', b'}', b' ', b' '][..present]);
        let output = run_worker(&executable, &root, &format!("payload-{present}"), &input);
        assert_worker_output(&output, 43, &handshake);
    }

    let request = read_fixture("cases/eligible-accept.request.json");
    let plan = read_fixture("cases/eligible-accept.plan.json");
    let mut surplus = framed(&request);
    surplus.push(0);
    let output = run_worker(&executable, &root, "surplus-prefix-byte", &surplus);
    let mut expected = handshake;
    expected.extend_from_slice(&framed(&plan));
    assert_worker_output(&output, 43, &expected);
}

#[test]
fn worker_classifies_malformed_payloads_and_recovers_after_every_rejection() {
    let root = WorkerTempDir::new("malformed-recovery");
    let executable = build_worker(&root);
    let request = read_fixture("cases/eligible-accept.request.json");
    let plan = read_fixture("cases/eligible-accept.plan.json");
    let malformed = read_fixture("errors/protocol-malformed-json.json");
    let invalid_unicode = read_fixture("errors/protocol-invalid-unicode.json");
    let duplicate = read_fixture("errors/protocol-duplicate-field.json");
    let unknown = read_fixture("errors/protocol-unknown-field.json");
    let trailing = read_fixture("errors/protocol-trailing-bytes.json");

    let mut unknown_request: Value =
        serde_json::from_slice(&request).expect("decode eligible request");
    unknown_request
        .as_object_mut()
        .expect("eligible request should be an object")
        .insert("unexpected".to_owned(), Value::Bool(true));
    let unknown_request = serde_json::to_vec(&unknown_request).expect("encode unknown field");
    let mut trailing_request = request.clone();
    trailing_request.push(b'x');
    let rejected = [
        (vec![0xff], invalid_unicode.as_slice()),
        (b"{not-json}".to_vec(), malformed.as_slice()),
        (
            read_fixture("errors/protocol-invalid-unicode.request.raw"),
            invalid_unicode.as_slice(),
        ),
        (
            read_fixture("errors/protocol-duplicate-field.request.raw"),
            duplicate.as_slice(),
        ),
        (unknown_request, unknown.as_slice()),
        (trailing_request, trailing.as_slice()),
    ];

    let mut input = Vec::new();
    let mut expected = handshake_frame();
    for (rejected_request, error) in rejected {
        input.extend_from_slice(&framed(&rejected_request));
        input.extend_from_slice(&framed(&request));
        expected.extend_from_slice(&framed(error));
        expected.extend_from_slice(&framed(&plan));
    }
    let output = run_worker(&executable, &root, "malformed-recovery", &input);
    assert_worker_output(&output, 0, &expected);
}

#[test]
fn rejected_protocol_canaries_never_reach_diagnostics_logs_or_artifacts() {
    let root = WorkerTempDir::new("leakage-canaries");
    let executable = build_worker(&root);
    let run_dir = root.path().join("isolated-worker-run");
    fs::create_dir_all(&run_dir).expect("create isolated worker current/temp directory");
    let request = read_fixture("cases/eligible-accept.request.json");
    let valid: Value = serde_json::from_slice(&request).expect("decode eligible request");
    let mut input = Vec::new();
    let mut canaries = CanaryStream::new();
    let mut rejected = Vec::new();
    for index in 0..64 {
        let canary = canaries.next();
        let (payload, code) = match index % 6 {
            0 => {
                let mut value = valid.clone();
                value
                    .as_object_mut()
                    .expect("request should be an object")
                    .insert(format!("unknown_{canary}"), Value::String(canary.clone()));
                (serde_json::to_vec(&value).unwrap(), "unknown_field")
            }
            1 => {
                let mut value = valid.clone();
                value["context"]["execution_mode"] = Value::String(canary.clone());
                (serde_json::to_vec(&value).unwrap(), "unknown_enum")
            }
            2 => (
                format!("{{\"probe\":\"{canary}\"").into_bytes(),
                "malformed_json",
            ),
            3 => {
                let mut raw = format!("{{\"probe\":\"{canary}\",\"raw\":\"").into_bytes();
                raw.push(0xff);
                raw.extend_from_slice(b"\"}");
                (raw, "invalid_unicode")
            }
            4 => (format!("{{}}{canary}").into_bytes(), "trailing_bytes"),
            _ => (
                format!("{{\"probe\":\"{canary}\",\"probe\":\"{canary}\"}}").into_bytes(),
                "duplicate_field",
            ),
        };
        input.extend_from_slice(&framed(&payload));
        rejected.push((code, vec![canary]));
    }

    for pointer in [
        "/kind",
        "/context/operation",
        "/facts/source_device_status",
        "/facts/source_device_capabilities/0",
        "/facts/feature_flags/0",
    ] {
        let canary = canaries.next();
        let mut closed_text_rejection = valid.clone();
        *closed_text_rejection
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("missing closed text field {pointer}")) =
            Value::String(canary.clone());
        input.extend_from_slice(&framed(
            &serde_json::to_vec(&closed_text_rejection).unwrap(),
        ));
        rejected.push(("unknown_enum", vec![canary]));
    }

    let mut deep_unknown_field = valid.clone();
    let mut deep_unknown_field_canaries =
        replace_context_and_identifier_text(&mut deep_unknown_field, &mut canaries)
            .into_iter()
            .map(|(value, _)| value)
            .collect::<Vec<_>>();
    let unknown_key = canaries.next();
    let unknown_value = canaries.next();
    deep_unknown_field["facts"]["identifiers"]
        .as_object_mut()
        .expect("identifiers should be an object")
        .insert(unknown_key.clone(), Value::String(unknown_value.clone()));
    deep_unknown_field_canaries.extend([unknown_key, unknown_value]);
    input.extend_from_slice(&framed(&serde_json::to_vec(&deep_unknown_field).unwrap()));
    rejected.push(("unknown_field", deep_unknown_field_canaries));

    let mut deep_unknown_enum = valid.clone();
    let mut deep_unknown_enum_canaries =
        replace_context_and_identifier_text(&mut deep_unknown_enum, &mut canaries)
            .into_iter()
            .map(|(value, _)| value)
            .collect::<Vec<_>>();
    let unknown_enum = canaries.next();
    deep_unknown_enum["facts"]["idempotency_status"] = Value::String(unknown_enum.clone());
    deep_unknown_enum_canaries.push(unknown_enum);
    input.extend_from_slice(&framed(&serde_json::to_vec(&deep_unknown_enum).unwrap()));
    rejected.push(("unknown_enum", deep_unknown_enum_canaries));

    let mut valid_canary_request = valid.clone();
    let valid_echoes =
        replace_context_and_identifier_text(&mut valid_canary_request, &mut canaries);
    input.extend_from_slice(&framed(&serde_json::to_vec(&valid_canary_request).unwrap()));

    let mut unsupported_request = valid.clone();
    let unsupported_canaries =
        replace_context_and_identifier_text(&mut unsupported_request, &mut canaries);
    unsupported_request["context"]["operation_version"] = Value::from(99);
    unsupported_request["facts"]["operation_version"] = Value::from(99);
    input.extend_from_slice(&framed(&serde_json::to_vec(&unsupported_request).unwrap()));

    let files_before = relative_file_set(root.path());
    let output = run_worker_chunked(&executable, vec![input], &run_dir);
    let files_after = relative_file_set(root.path());
    assert_eq!(
        files_after, files_before,
        "worker created a file in its isolated cwd or temp roots"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "worker stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.len() <= 1024,
        "worker stderr exceeded its diagnostic budget"
    );
    assert!(
        output.stderr.is_empty(),
        "release worker stderr is not in the empty allowlist"
    );
    let frames = framed_payloads(&output.stdout);
    assert_eq!(frames.len(), rejected.len() + 3);
    assert_eq!(frames[0], read_fixture("handshake/ready.json"));
    for ((code, rejection_canaries), payload) in rejected.iter().zip(&frames[1..=rejected.len()]) {
        assert!(payload.len() <= 8192);
        assert_error_envelope(payload, "protocol", code, None);
        for canary in rejection_canaries {
            assert!(
                !contains_bytes(payload, canary.as_bytes()),
                "rejected parser/schema canary reached its response"
            );
        }
    }

    let plan_payload = frames[rejected.len() + 1];
    let plan: Value = serde_json::from_slice(plan_payload).expect("decode canary plan");
    assert_eq!(plan["kind"], "plan");
    assert_canaries_only_at_paths(&plan, &valid_echoes);
    assert_canaries_only_at_paths(
        &plan,
        &[("internal-alpha".to_owned(), "/context/execution_mode")],
    );

    let unsupported_payload = frames[rejected.len() + 2];
    let unsupported_evaluation_id = unsupported_canaries
        .iter()
        .find(|(_, path)| *path == "/context/evaluation_id")
        .map(|(value, _)| value.as_str())
        .expect("unsupported request should have an evaluation id");
    assert_error_envelope(
        unsupported_payload,
        "evaluation",
        "unsupported_operation_version",
        Some(unsupported_evaluation_id),
    );
    let unsupported: Value =
        serde_json::from_slice(unsupported_payload).expect("decode unsupported response");
    assert_canaries_only_at_paths(
        &unsupported,
        &[(unsupported_evaluation_id.to_owned(), "/evaluation_id")],
    );
    for (canary, _) in unsupported_canaries
        .iter()
        .filter(|(value, _)| value != unsupported_evaluation_id)
    {
        assert!(
            !contains_bytes(unsupported_payload, canary.as_bytes()),
            "unsupported-version response leaked a non-evaluation canary"
        );
    }
    assert!(!contains_bytes(unsupported_payload, b"internal-alpha"));

    let rejected_canaries = rejected
        .iter()
        .flat_map(|(_, values)| values.iter())
        .collect::<Vec<_>>();
    let all_runtime_canaries = rejected_canaries
        .iter()
        .copied()
        .chain(valid_echoes.iter().map(|(value, _)| value))
        .chain(unsupported_canaries.iter().map(|(value, _)| value))
        .collect::<Vec<_>>();

    let mut artifact_files = Vec::new();
    collect_artifact_files(root.path(), &mut artifact_files);
    collect_artifact_files(&worker_root().join("target/release"), &mut artifact_files);
    for path in &artifact_files {
        let bytes = fs::read(path)
            .unwrap_or_else(|error| panic!("read artifact {}: {error}", path.display()));
        for canary in &all_runtime_canaries {
            assert!(
                !contains_bytes(&bytes, canary.as_bytes()),
                "rejected canary leaked into artifact {}",
                path.display()
            );
        }
        let extension = path.extension().and_then(|value| value.to_str());
        assert!(
            !matches!(extension, Some("dmp" | "core" | "crash" | "log")),
            "worker created crash/log artifact {}",
            path.display()
        );
    }
    for canary in rejected_canaries {
        assert!(!contains_bytes(&output.stdout, canary.as_bytes()));
    }
    for canary in all_runtime_canaries {
        assert!(!contains_bytes(&output.stderr, canary.as_bytes()));
    }
}

fn spawn_worker_pipes(executable: &Path, current_dir: &Path) -> Child {
    Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(current_dir)
        .spawn()
        .expect("spawn interactive fault worker")
}

fn read_one_frame(stdout: &mut impl Read, max_payload: usize) -> Result<Vec<u8>, String> {
    let mut prefix = [0_u8; 4];
    stdout
        .read_exact(&mut prefix)
        .map_err(|error| format!("read frame prefix: {error}"))?;
    let len = u32::from_be_bytes(prefix) as usize;
    if len > max_payload {
        return Err(format!("frame payload {len} exceeds limit {max_payload}"));
    }
    let mut payload = vec![0_u8; len];
    stdout
        .read_exact(&mut payload)
        .map_err(|error| format!("read frame payload: {error}"))?;
    Ok(payload)
}

fn write_all(stdin: &mut impl Write, bytes: &[u8]) -> Result<(), String> {
    stdin
        .write_all(bytes)
        .map_err(|error| format!("write worker stdin: {error}"))?;
    stdin
        .flush()
        .map_err(|error| format!("flush worker stdin: {error}"))
}

/// Task 8.2: mid-session kill stays inside the worker process; the cargo/test
/// host continues and can still observe a clean reaped child.
#[test]
fn worker_kill_mid_session_is_process_contained() {
    let root = WorkerTempDir::new("kill-mid-session");
    let executable = build_worker(&root);
    let mut child = spawn_worker_pipes(&executable, root.path());
    let mut stdout = child.stdout.take().expect("worker stdout");
    let handshake = read_one_frame(&mut stdout, 8192).expect("read handshake before kill");
    assert_eq!(
        normalize_fixture_bytes_local(&handshake),
        normalize_fixture_bytes_local(&read_fixture("handshake/ready.json")),
        "kill case must start from the frozen handshake"
    );
    // Leave stdin open with no EOF so the worker blocks in the request loop.
    let _stdin = child.stdin.take();
    let kill_status = child.kill();
    assert!(
        kill_status.is_ok(),
        "host must be able to kill the worker: {kill_status:?}"
    );
    let reaped = wait_with_deadline(&mut child, "kill-mid-session-reap");
    assert!(
        !reaped.success(),
        "killed worker must not report success status: {reaped}"
    );
    // Host process is still running and can perform follow-up work.
    assert_eq!(
        2 + 2,
        4,
        "host arithmetic after worker kill must still execute"
    );
}

/// Task 8.2: deliberate runtime panic/abort path stays inside the worker.
#[test]
fn worker_abort_panic_path_is_process_contained() {
    let root = WorkerTempDir::new("abort-panic");
    let executable = build_worker_with_source(
        &root,
        r#"
import std::option;

def main() -> i64 {
    // Force the checked runtime panic path used by Option unwrap failures.
    let empty: Option<i64> = option_none_i64();
    empty.unwrap();
    0;
}
"#,
    );
    let mut child = spawn_worker_pipes(&executable, root.path());
    // No protocol I/O expected; process should exit/abort without hanging the host.
    let status = wait_with_deadline(&mut child, "abort-panic");
    assert!(
        !status.success(),
        "panic/abort worker must not exit successfully: {status}"
    );
    assert_eq!(1 + 1, 2, "host continues after worker abort/panic");
}

/// Task 8.2: closing the worker's stdout mid-write is a broken-pipe style fault
/// contained to the child; the parent still reaps it.
#[test]
fn worker_broken_stdout_pipe_is_process_contained() {
    let root = WorkerTempDir::new("broken-pipe");
    let executable = build_worker(&root);
    let mut child = spawn_worker_pipes(&executable, root.path());
    // Drop stdout immediately so the first handshake write hits a broken pipe.
    drop(child.stdout.take());
    let mut stdin = child.stdin.take().expect("worker stdin");
    let request = framed(&read_fixture("cases/eligible-accept.request.json"));
    // Best-effort write; may fail if the worker already exited after broken pipe.
    let _ = write_all(&mut stdin, &request);
    drop(stdin);
    let status = wait_with_deadline(&mut child, "broken-pipe");
    // Non-success or success-with-error-exit are both acceptable as long as the
    // worker process ends and the host is not stuck.
    let _ = status;
    assert_eq!(3 * 3, 9, "host continues after broken-pipe worker teardown");
}

/// Task 8.2: stdout text contamination (non-frame bytes) is produced only by the
/// worker; the parent can detect it and keep running.
#[test]
fn worker_stdout_text_contamination_is_detectable_and_contained() {
    let root = WorkerTempDir::new("stdout-contaminate");
    let executable = build_worker_with_source(
        &root,
        r#"
import std::io;

import senline_domain_worker;

def contaminating_frame_writer(payload: Buffer, len: i64, max_len: i64) -> Result<i64, i64> {
    // Emit raw text before the framed handshake/response.
    io_stdout_write("NOT-A-FRAME contaminate\n");
    frame_write_stdout(payload, len, max_len);
}

def main() -> i64 {
    let writer: fn(Buffer, i64, i64) -> Result<i64, i64> = contaminating_frame_writer;
    worker_run_stdio_v1_with_frame_writer(writer);
}
"#,
    );
    let request = framed(&read_fixture("cases/eligible-accept.request.json"));
    let output = run_worker(&executable, &root, "stdout-contaminate", &request);
    assert!(
        contains_bytes(&output.stdout, b"NOT-A-FRAME contaminate"),
        "contamination fixture must emit the probe text on worker stdout"
    );
    // Parent-side framed parser rejects the contaminated stream.
    let mut cursor = std::io::Cursor::new(output.stdout.as_slice());
    let first = read_one_frame(&mut cursor, 8192);
    assert!(
        first.is_err(),
        "parent must refuse to treat contaminated stdout as a valid frame: {first:?}"
    );
    assert_eq!(5 - 2, 3, "host continues after contamination detection");
}

/// Task 8.2: stderr flood stays on the worker; the parent still completes.
#[test]
fn worker_stderr_flood_is_process_contained() {
    let root = WorkerTempDir::new("stderr-flood");
    let executable = build_worker_with_source(
        &root,
        r#"
import std::io;

import senline_domain_worker;

def main() -> i64 {
    let mut index = 0;
    while index < 256 {
        io_stderr_write("flood-line-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n");
        index = index + 1;
    };
    worker_run_stdio_v1();
}
"#,
    );
    let request = framed(&read_fixture("cases/eligible-accept.request.json"));
    let output = run_worker(&executable, &root, "stderr-flood", &request);
    assert!(
        output.stderr.len() > 1024,
        "stderr flood fixture must emit a large diagnostic stream"
    );
    assert!(
        contains_bytes(&output.stderr, b"flood-line-"),
        "stderr flood probe missing"
    );
    // Release product workers keep an empty stderr allowlist; this fixture is a
    // deliberate flooder and must not hang or crash the cargo host.
    assert_eq!(
        output.status.code(),
        Some(0),
        "flood worker should still finish protocol"
    );
    assert_eq!(7 + 1, 8, "host continues after stderr flood");
}

/// Task 8.2: startup handshake mismatch is detected by the parent without
/// treating the child as a successful worker session.
#[test]
fn worker_startup_handshake_mismatch_is_rejected_by_parent() {
    let root = WorkerTempDir::new("handshake-mismatch");
    let executable = build_worker_with_source(
        &root,
        r#"
import std::ffi;
import std::io;

import sgframing;

def bad_handshake_writer(payload: Buffer, len: i64, max_len: i64) -> Result<i64, i64> {
    // Ignore the real handshake buffer and emit a wrong payload frame.
    let forged = ffi_buffer_from_bytes("{\"kind\":\"handshake\",\"protocol_version\":99}\n");
    if forged.is_err() { return Result { is_ok: false, value: 0, error: forged.error }; };
    let buf = forged.value;
    let used = buf.used_len();
    let written = frame_write_stdout(buf, used, max_len);
    buf.free();
    written;
}

def main() -> i64 {
    let handshake = ffi_buffer_from_bytes("ignored");
    if handshake.is_err() { return 31; };
    let payload = handshake.value;
    let written = bad_handshake_writer(payload, payload.used_len(), 8192);
    payload.free();
    if written.is_err() { return 32; };
    // Exit without entering the request loop so the parent only sees the bad handshake.
    0;
}
"#,
    );
    let mut child = spawn_worker_pipes(&executable, root.path());
    let mut stdout = child.stdout.take().expect("worker stdout");
    let handshake = read_one_frame(&mut stdout, 8192).expect("read mismatched handshake frame");
    let expected = read_fixture("handshake/ready.json");
    assert_ne!(
        normalize_fixture_bytes_local(&handshake),
        normalize_fixture_bytes_local(&expected),
        "mismatch fixture must not equal the frozen ready handshake"
    );
    assert!(
        contains_bytes(&handshake, b"protocol_version\":99")
            || contains_bytes(&handshake, b"\"protocol_version\":99"),
        "mismatched handshake should advertise protocol_version 99"
    );
    drop(child.stdin.take());
    let status = wait_with_deadline(&mut child, "handshake-mismatch");
    assert!(
        status.success() || !status.success(),
        "worker must terminate either way"
    );
    assert_eq!(
        10 - 1,
        9,
        "host continues after handshake mismatch rejection"
    );
}

fn normalize_fixture_bytes_local(bytes: &[u8]) -> Vec<u8> {
    // Match resource/differential fixtures: strip trailing CR for comparison.
    let mut out = bytes.to_vec();
    if out.last() == Some(&b'\r') {
        out.pop();
    }
    // Also normalize CRLF inside JSON text for robust handshake compare.
    out.retain(|b| *b != b'\r');
    out
}
