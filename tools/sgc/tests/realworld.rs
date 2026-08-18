mod common;

use common::source_sgc_command;
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgc crate should live under tools/sgc")
        .to_path_buf()
}

fn realworld(name: &str) -> PathBuf {
    workspace_root().join("examples/realworld").join(name)
}

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("sgc_realworld_{name}_{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn assert_sgc_check(path: &Path, module_map: Option<String>) {
    let mut command = source_sgc_command();
    command.arg("check").arg(path);
    if let Some(module_map) = module_map {
        command.env("SENGOO_MODULE_MAP", module_map);
    }
    let output = command.output().expect("run sgc check");
    assert!(
        output.status.success(),
        "sgc check failed for {}\nstdout:\n{}\nstderr:\n{}",
        path.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_with_binary_stdin(executable: &Path, input: &[u8]) -> Output {
    let mut child = std::process::Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| panic!("spawn {}: {err}", executable.display()));
    let mut stdin = child.stdin.take().expect("piped stdin should be available");
    stdin.write_all(input).expect("write binary test input");
    drop(stdin);
    child.wait_with_output().expect("wait for binary fixture")
}

fn framed(payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + payload.len());
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

fn unframe_exact(frame: &[u8]) -> &[u8] {
    assert!(frame.len() >= 4, "framed payload needs a complete prefix");
    let len = u32::from_be_bytes(frame[..4].try_into().expect("four-byte frame prefix")) as usize;
    assert_eq!(frame.len(), len + 4, "frame must contain one exact payload");
    &frame[4..]
}

fn senline_worker_module_map(worker: &Path) -> std::ffi::OsString {
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
    .expect("encode Senline worker module map")
}

fn build_senline_worker(test_name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let worker = realworld("senline-domain-worker");
    let fixtures = worker.join("fixtures/v1");
    let dir = temp_dir(test_name);
    let executable = dir.join(if cfg!(windows) {
        "senline_domain_worker.exe"
    } else {
        "senline_domain_worker"
    });
    let compile = source_sgc_command()
        .arg("build")
        .arg(worker.join("src/main.sg"))
        .arg("-o")
        .arg(&executable)
        .args(["-O", "0", "--force-rebuild"])
        .current_dir(&worker)
        .env("SENGOO_MODULE_MAP", senline_worker_module_map(&worker))
        .output()
        .expect("compile Senline domain worker");
    assert!(
        compile.status.success(),
        "compile stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );

    (fixtures, dir, executable)
}

fn assert_clean_worker_output(output: &Output, expected: &[u8]) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "worker stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "worker stderr must stay empty");
    assert_eq!(output.stdout, expected, "worker protocol bytes changed");
}

#[test]
fn senline_worker_emits_handshake_before_clean_eof() {
    let (fixtures, dir, executable) = build_senline_worker("senline_worker_handshake_eof");
    let handshake =
        fs::read(fixtures.join("handshake/ready.json")).expect("read handshake fixture");
    let output = run_with_binary_stdin(&executable, &[]);

    assert_clean_worker_output(&output, &framed(&handshake));

    assert!(dir.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn senline_worker_eligible_fixture_roundtrips_exact_frames() {
    let (fixtures, dir, executable) = build_senline_worker("senline_worker_eligible_roundtrip");
    let request = fs::read(fixtures.join("cases/eligible-accept.request.json"))
        .expect("read eligible request fixture");
    let handshake =
        fs::read(fixtures.join("handshake/ready.json")).expect("read handshake fixture");
    let plan = fs::read(fixtures.join("cases/eligible-accept.plan.json"))
        .expect("read eligible plan fixture");
    let output = run_with_binary_stdin(&executable, &framed(&request));
    let mut expected = framed(&handshake);
    expected.extend_from_slice(&framed(&plan));

    assert_clean_worker_output(&output, &expected);

    assert!(dir.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn senline_worker_all_v1_fixtures_roundtrip_in_one_process() {
    let (fixtures, dir, executable) = build_senline_worker("senline_worker_v1_roundtrip");
    let cases = [
        ("eligible-accept.request.json", "eligible-accept.plan.json"),
        ("exact-duplicate.request.json", "exact-duplicate.plan.json"),
        (
            "idempotency-conflict.request.json",
            "idempotency-conflict.plan.json",
        ),
        (
            "application-budget-rejection.request.json",
            "application-budget-rejection.plan.json",
        ),
        (
            "unknown-operation-version.request.json",
            "unknown-operation-version.error.json",
        ),
    ];
    let mut input = Vec::new();
    let handshake =
        fs::read(fixtures.join("handshake/ready.json")).expect("read handshake fixture");
    let mut expected = framed(&handshake);
    for (request_name, response_name) in cases {
        let request = fs::read(fixtures.join("cases").join(request_name))
            .unwrap_or_else(|err| panic!("read {request_name}: {err}"));
        let response = fs::read(fixtures.join("cases").join(response_name))
            .unwrap_or_else(|err| panic!("read {response_name}: {err}"));
        input.extend_from_slice(&framed(&request));
        expected.extend_from_slice(&framed(&response));
    }
    let output = run_with_binary_stdin(&executable, &input);

    assert_clean_worker_output(&output, &expected);

    assert!(dir.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn senline_worker_recovers_after_malformed_json_frame() {
    let (fixtures, dir, executable) = build_senline_worker("senline_worker_malformed_recovery");
    let malformed = b"{not-json}\n";
    let request = fs::read(fixtures.join("cases/eligible-accept.request.json"))
        .expect("read eligible request fixture");
    let handshake =
        fs::read(fixtures.join("handshake/ready.json")).expect("read handshake fixture");
    let protocol_error = fs::read(fixtures.join("errors/protocol-malformed-json.json"))
        .expect("read malformed JSON error fixture");
    let plan = fs::read(fixtures.join("cases/eligible-accept.plan.json"))
        .expect("read eligible plan fixture");
    let mut input = framed(malformed);
    input.extend_from_slice(&framed(&request));
    let mut expected = framed(&handshake);
    expected.extend_from_slice(&framed(&protocol_error));
    expected.extend_from_slice(&framed(&plan));
    let output = run_with_binary_stdin(&executable, &input);

    assert_clean_worker_output(&output, &expected);

    assert!(dir.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn senline_worker_recovers_after_extra_and_equal_count_unknown_fields() {
    let (fixtures, dir, executable) = build_senline_worker("senline_worker_unknown_field_recovery");
    let request = fs::read(fixtures.join("cases/eligible-accept.request.json"))
        .expect("read eligible request fixture");
    let valid: Value = serde_json::from_slice(&request).expect("decode request fixture");
    let mut extra = valid.clone();
    extra
        .as_object_mut()
        .expect("request fixture must be an object")
        .insert("unexpected".to_owned(), Value::Bool(true));
    let extra = serde_json::to_vec(&extra).expect("encode request with extra unknown field");
    let mut substituted = valid.clone();
    let substituted_object = substituted
        .as_object_mut()
        .expect("request fixture must be an object");
    substituted_object.remove("context");
    substituted_object.insert("unexpected".to_owned(), Value::Bool(true));
    let substituted =
        serde_json::to_vec(&substituted).expect("encode equal-count field substitution");
    let mut nested_substituted = valid;
    let nested_identifiers = nested_substituted["facts"]["identifiers"]
        .as_object_mut()
        .expect("identifiers fixture must be an object");
    nested_identifiers.remove("correlation_ref");
    nested_identifiers.insert("unexpected".to_owned(), Value::Bool(true));
    let nested_substituted = serde_json::to_vec(&nested_substituted)
        .expect("encode nested equal-count field substitution");
    let handshake =
        fs::read(fixtures.join("handshake/ready.json")).expect("read handshake fixture");
    let protocol_error = fs::read(fixtures.join("errors/protocol-unknown-field.json"))
        .expect("read unknown-field error fixture");
    let plan = fs::read(fixtures.join("cases/eligible-accept.plan.json"))
        .expect("read eligible plan fixture");
    let mut input = framed(&extra);
    input.extend_from_slice(&framed(&request));
    input.extend_from_slice(&framed(&substituted));
    input.extend_from_slice(&framed(&request));
    input.extend_from_slice(&framed(&nested_substituted));
    input.extend_from_slice(&framed(&request));
    let mut expected = framed(&handshake);
    expected.extend_from_slice(&framed(&protocol_error));
    expected.extend_from_slice(&framed(&plan));
    expected.extend_from_slice(&framed(&protocol_error));
    expected.extend_from_slice(&framed(&plan));
    expected.extend_from_slice(&framed(&protocol_error));
    expected.extend_from_slice(&framed(&plan));
    let output = run_with_binary_stdin(&executable, &input);

    assert_clean_worker_output(&output, &expected);

    assert!(dir.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn senline_worker_recovers_after_each_schema_rejection_class() {
    let (fixtures, dir, executable) = build_senline_worker("senline_worker_schema_recovery");
    let request = fs::read(fixtures.join("cases/eligible-accept.request.json"))
        .expect("read eligible request fixture");
    let valid: Value = serde_json::from_slice(&request).expect("decode request fixture");

    let mut missing = valid.clone();
    missing
        .as_object_mut()
        .expect("request fixture must be an object")
        .remove("facts");

    let mut wrong_kind = valid.clone();
    wrong_kind
        .as_object_mut()
        .expect("request fixture must be an object")
        .insert("schema_version".to_owned(), Value::String("1".to_owned()));

    let mut unknown_enum = valid;
    unknown_enum
        .as_object_mut()
        .expect("request fixture must be an object")
        .insert("kind".to_owned(), Value::String("future".to_owned()));

    let mut out_of_range = unknown_enum.clone();
    out_of_range["kind"] = Value::String("evaluation".to_owned());
    out_of_range["facts"]["ciphertext_length_bytes"] = Value::from(4_294_967_296_u64);

    let malformed_error = fs::read(fixtures.join("errors/protocol-malformed-json.json"))
        .expect("read malformed JSON error fixture");
    let unknown_enum_error = fs::read(fixtures.join("errors/protocol-unknown-enum.json"))
        .expect("read unknown-enum error fixture");
    let rejected = [
        (
            serde_json::to_vec(&missing).expect("encode request missing a field"),
            malformed_error.as_slice(),
        ),
        (
            serde_json::to_vec(&wrong_kind).expect("encode request with wrong field kind"),
            malformed_error.as_slice(),
        ),
        (
            serde_json::to_vec(&unknown_enum).expect("encode request with unknown enum"),
            unknown_enum_error.as_slice(),
        ),
        (
            serde_json::to_vec(&out_of_range).expect("encode request with out-of-range integer"),
            malformed_error.as_slice(),
        ),
    ];

    let handshake =
        fs::read(fixtures.join("handshake/ready.json")).expect("read handshake fixture");
    let plan = fs::read(fixtures.join("cases/eligible-accept.plan.json"))
        .expect("read eligible plan fixture");
    let mut input = Vec::new();
    let mut expected = framed(&handshake);
    for (rejected_request, error) in rejected {
        input.extend_from_slice(&framed(&rejected_request));
        input.extend_from_slice(&framed(&request));
        expected.extend_from_slice(&framed(error));
        expected.extend_from_slice(&framed(&plan));
    }
    let output = run_with_binary_stdin(&executable, &input);

    assert_clean_worker_output(&output, &expected);

    assert!(dir.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn senline_worker_recovers_after_unsupported_operation_version() {
    let (fixtures, dir, executable) =
        build_senline_worker("senline_worker_unsupported_version_recovery");
    let unsupported = fs::read(fixtures.join("cases/unknown-operation-version.request.json"))
        .expect("read unsupported-version request fixture");
    let unsupported_error = fs::read(fixtures.join("cases/unknown-operation-version.error.json"))
        .expect("read unsupported-version error fixture");
    let request = fs::read(fixtures.join("cases/eligible-accept.request.json"))
        .expect("read eligible request fixture");
    let plan = fs::read(fixtures.join("cases/eligible-accept.plan.json"))
        .expect("read eligible plan fixture");
    let handshake =
        fs::read(fixtures.join("handshake/ready.json")).expect("read handshake fixture");

    let mut input = framed(&unsupported);
    input.extend_from_slice(&framed(&request));
    let mut expected = framed(&handshake);
    expected.extend_from_slice(&framed(&unsupported_error));
    expected.extend_from_slice(&framed(&plan));
    let output = run_with_binary_stdin(&executable, &input);

    assert_clean_worker_output(&output, &expected);

    assert!(dir.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn senline_worker_classifies_and_recovers_after_each_strict_parser_error() {
    let (fixtures, dir, executable) = build_senline_worker("senline_worker_parser_recovery");
    let request = fs::read(fixtures.join("cases/eligible-accept.request.json"))
        .expect("read eligible request fixture");
    let cases = [
        (
            "protocol-duplicate-field.request.raw",
            "protocol-duplicate-field.json",
        ),
        (
            "protocol-invalid-unicode.request.raw",
            "protocol-invalid-unicode.json",
        ),
        (
            "protocol-trailing-bytes.request.raw",
            "protocol-trailing-bytes.json",
        ),
    ];
    let handshake =
        fs::read(fixtures.join("handshake/ready.json")).expect("read handshake fixture");
    let plan = fs::read(fixtures.join("cases/eligible-accept.plan.json"))
        .expect("read eligible plan fixture");
    let mut input = Vec::new();
    let mut expected = framed(&handshake);
    for (request_name, response_name) in cases {
        let rejected = fs::read(fixtures.join("errors").join(request_name))
            .unwrap_or_else(|err| panic!("read {request_name}: {err}"));
        let response = fs::read(fixtures.join("errors").join(response_name))
            .unwrap_or_else(|err| panic!("read {response_name}: {err}"));
        input.extend_from_slice(&framed(&rejected));
        input.extend_from_slice(&framed(&request));
        expected.extend_from_slice(&framed(&response));
        expected.extend_from_slice(&framed(&plan));
    }
    let output = run_with_binary_stdin(&executable, &input);

    assert_clean_worker_output(&output, &expected);

    assert!(dir.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn senline_worker_preserves_embedded_nul_in_owned_plan_strings() {
    let (fixtures, dir, executable) = build_senline_worker("senline_worker_embedded_nul");
    let request = fs::read(fixtures.join("cases/eligible-accept.request.json"))
        .expect("read eligible request fixture");
    let mut request: Value = serde_json::from_slice(&request).expect("decode request fixture");
    let correlation_ref = "corr_ref\0suffix";
    request["facts"]["identifiers"]["correlation_ref"] = Value::String(correlation_ref.to_owned());
    let request = serde_json::to_vec(&request).expect("encode embedded-NUL request");
    let handshake =
        fs::read(fixtures.join("handshake/ready.json")).expect("read handshake fixture");
    let handshake_frame = framed(&handshake);
    let output = run_with_binary_stdin(&executable, &framed(&request));

    assert_eq!(
        output.status.code(),
        Some(0),
        "worker stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "worker stderr must stay empty");
    assert!(
        output.stdout.starts_with(&handshake_frame),
        "worker handshake bytes changed"
    );
    let response = unframe_exact(&output.stdout[handshake_frame.len()..]);
    let plan: Value = serde_json::from_slice(response).expect("decode worker plan");
    assert_eq!(plan["kind"], "plan");
    assert_eq!(
        plan["identifiers"]["correlation_ref"].as_str(),
        Some(correlation_ref),
        "owned string bytes after U+0000 must not be truncated"
    );

    assert!(dir.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sgframing_binary_pipe_covers_boundaries_and_exact_output() {
    let package = realworld("senline-domain-worker").join("packages/sgframing");
    let dir = temp_dir("sgframing_binary_pipes");
    let source = dir.join("frame_pipe.sg");
    fs::copy(package.join("tests/frame_pipe.sg"), &source).expect("copy pipe fixture");
    let executable = dir.join(if cfg!(windows) {
        "sgframing_pipe.exe"
    } else {
        "sgframing_pipe"
    });
    let module_map = format!("sgframing={}", package.join("src/lib.sg").display());
    let compile = source_sgc_command()
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .args(["-O", "0", "--force-rebuild"])
        .current_dir(&package)
        .env("SENGOO_MODULE_MAP", module_map)
        .output()
        .expect("compile sgframing pipe fixture");
    assert!(
        compile.status.success(),
        "compile stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );

    let clean_eof = run_with_binary_stdin(&executable, &[]);
    assert_eq!(clean_eof.status.code(), Some(0));
    assert!(clean_eof.stdout.is_empty());
    assert!(clean_eof.stderr.is_empty());

    for payload in [
        vec![b'x'],
        vec![0x00, 0x0a, 0x0d, 0x1a, 0x80, 0xff, 0x41, 0x7f],
        vec![0x5a; 64],
    ] {
        let input = framed(&payload);
        let echoed = run_with_binary_stdin(&executable, &input);
        assert_eq!(
            echoed.status.code(),
            Some(0),
            "valid frame stderr: {}",
            String::from_utf8_lossy(&echoed.stderr)
        );
        assert_eq!(echoed.stdout, input, "frame output must be exact");
        assert!(echoed.stderr.is_empty());
    }

    let prefix = 4_u32.to_be_bytes();
    for split in 1..4 {
        let truncated = run_with_binary_stdin(&executable, &prefix[..split]);
        assert_eq!(truncated.status.code(), Some(24), "prefix split {split}");
        assert!(truncated.stdout.is_empty());
        assert!(truncated.stderr.is_empty());
    }

    let payload = [0x10, 0x20, 0x30, 0x40];
    let complete = framed(&payload);
    for payload_bytes in 0..payload.len() {
        let truncated = run_with_binary_stdin(&executable, &complete[..4 + payload_bytes]);
        assert_eq!(
            truncated.status.code(),
            Some(24),
            "payload bytes {payload_bytes}"
        );
        assert!(truncated.stdout.is_empty());
        assert!(truncated.stderr.is_empty());
    }

    for (prefix, expected_code) in [(0_u32.to_be_bytes(), 21), (65_u32.to_be_bytes(), 22)] {
        let rejected = run_with_binary_stdin(&executable, &prefix);
        assert_eq!(rejected.status.code(), Some(expected_code));
        assert!(rejected.stdout.is_empty());
        assert!(rejected.stderr.is_empty());
    }

    assert!(dir.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn realworld_sources_check_through_sgc_command() {
    assert_sgc_check(&realworld("async-channel-smoke").join("src/main.sg"), None);
    assert_sgc_check(&realworld("cli-json-audit").join("src/main.sg"), None);
    assert_sgc_check(
        &realworld("compressed-json-artifact").join("src/main.sg"),
        None,
    );
    let default_library = realworld("default-library-conformance");
    let default_library_map = format!(
        "default_library_conformance={}",
        default_library.join("src/lib.sg").display()
    );
    assert_sgc_check(
        &default_library.join("src/main.sg"),
        Some(default_library_map),
    );
    assert_sgc_check(&realworld("http-client-status").join("src/main.sg"), None);
    assert_sgc_check(&realworld("p0-foundations").join("src/main.sg"), None);

    let workspace_doc_loop = realworld("workspace-doc-loop");
    let module_map = format!(
        "workspace_doc_loop={}",
        workspace_doc_loop.join("src/lib.sg").display()
    );
    assert_sgc_check(&workspace_doc_loop.join("src/main.sg"), Some(module_map));
}

#[test]
fn default_library_conformance_runs_generic_string_keyed_map_natively() {
    let fixture = realworld("default-library-conformance");
    let module_map = format!(
        "default_library_conformance={}",
        fixture.join("src/lib.sg").display()
    );
    let output = source_sgc_command()
        .arg("run")
        .arg(fixture.join("src/main.sg"))
        .arg("--force-rebuild")
        .env("SENGOO_MODULE_MAP", module_map)
        .output()
        .expect("run default-library conformance fixture");
    assert!(
        output.status.success(),
        "default-library conformance failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn realworld_missing_import_check_reports_json_diagnostic() {
    let dir = temp_dir("missing_import");
    let source = dir.join("main.sg");
    fs::write(
        &source,
        "import definitely_missing_realworld_module;\n\ndef main() -> i64 {\n    0\n}\n",
    )
    .unwrap();

    let output = source_sgc_command()
        .arg("--error-format")
        .arg("json")
        .arg("check")
        .arg(&source)
        .output()
        .expect("run sgc json check");
    assert!(!output.status.success(), "missing import should fail");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let payload: Value = serde_json::from_str(stderr.trim()).unwrap_or_else(|err| {
        panic!(
            "stderr should be machine-readable JSON ({err})\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            stderr
        )
    });
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["kind"], "compile_error");
    assert_eq!(payload["stage"], "import");
    assert_eq!(payload["input"], source.to_string_lossy().as_ref());
    assert!(payload["message"]
        .as_str()
        .unwrap_or_default()
        .contains("definitely_missing_realworld_module"));
    assert!(payload["location"]["line"].as_u64().is_some());
    assert!(payload["hint"]
        .as_str()
        .unwrap_or_default()
        .contains("import"));

    let _ = fs::remove_dir_all(dir);
}
