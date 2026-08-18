use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const CONTEXT_FIELDS: &[&str] = &[
    "contract_version",
    "operation",
    "operation_version",
    "evaluation_id",
    "operation_epoch",
    "worker_generation",
    "execution_mode",
    "worker_bundle_id",
    "facts_binding",
];
const BINDING_CONTEXT_FIELDS: &[&str] = &[
    "contract_version",
    "operation",
    "operation_version",
    "evaluation_id",
    "operation_epoch",
    "worker_generation",
    "execution_mode",
    "worker_bundle_id",
];
const FACT_FIELDS: &[&str] = &[
    "contract_version",
    "operation_version",
    "identifiers",
    "source_device_status",
    "source_device_capabilities",
    "envelope_protocol_version",
    "ciphertext_length_bytes",
    "idempotency_status",
    "recipient_pending_count",
    "recipient_pending_limit",
    "application_envelopes_used",
    "application_envelopes_limit",
    "ciphertext_limit_bytes",
    "feature_flags",
];
const IDENTIFIER_FIELDS: &[&str] = &[
    "correlation_ref",
    "source_account_ref",
    "source_device_ref",
    "recipient_account_ref",
    "recipient_device_ref",
    "conversation_ref",
    "envelope_ref",
];

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgc crate should live under tools/sgc")
        .join("examples")
        .join("realworld")
        .join("senline-domain-worker")
        .join("fixtures")
        .join("v1")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(
        &fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

fn object<'a>(value: &'a Value, label: &str) -> &'a Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{label} must be an object"))
}

fn exact_keys(value: &Value, expected: &[&str], label: &str) {
    let actual = object(value, label)
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "{label} field set changed");
}

fn string<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("{field} must be a string"))
}

fn unsigned(value: &Value, field: &str) -> u64 {
    value[field]
        .as_u64()
        .unwrap_or_else(|| panic!("{field} must be a non-negative integer"))
}

fn assert_ascii_ref(value: &str, label: &str) {
    assert!(
        (1..=128).contains(&value.len()),
        "{label} must be 1..128 bytes"
    );
    assert!(value.is_ascii(), "{label} must be ASCII");
}

fn assert_lower_hex(value: &str, expected_len: usize, label: &str) {
    assert_eq!(value.len(), expected_len, "{label} length changed");
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{label} must be lowercase hexadecimal"
    );
}

fn append_u32(bytes: &mut Vec<u8>, value: u64, field: &str) {
    let value = u32::try_from(value).unwrap_or_else(|_| panic!("{field} exceeds u32"));
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn append_string(bytes: &mut Vec<u8>, value: &str) {
    append_u32(bytes, value.len() as u64, "string length");
    bytes.extend_from_slice(value.as_bytes());
}

fn append_string_array(bytes: &mut Vec<u8>, value: &Value, field: &str) {
    let values = value[field]
        .as_array()
        .unwrap_or_else(|| panic!("{field} must be an array"));
    append_u32(bytes, values.len() as u64, "array length");
    let mut previous: Option<&str> = None;
    for item in values {
        let item = item
            .as_str()
            .unwrap_or_else(|| panic!("{field} values must be strings"));
        if let Some(previous) = previous {
            assert!(previous < item, "{field} must be sorted and unique");
        }
        append_string(bytes, item);
        previous = Some(item);
    }
}

fn facts_binding(request: &Value) -> String {
    let context = &request["context"];
    let facts = &request["facts"];
    let identifiers = &facts["identifiers"];
    let mut bytes = b"senline.submit-envelope.binding.v1\0".to_vec();

    append_u32(
        &mut bytes,
        unsigned(context, "contract_version"),
        "context.contract_version",
    );
    append_string(&mut bytes, string(context, "operation"));
    append_u32(
        &mut bytes,
        unsigned(context, "operation_version"),
        "context.operation_version",
    );
    append_string(&mut bytes, string(context, "evaluation_id"));
    bytes.extend_from_slice(&unsigned(context, "operation_epoch").to_be_bytes());
    bytes.extend_from_slice(&unsigned(context, "worker_generation").to_be_bytes());
    append_string(&mut bytes, string(context, "execution_mode"));
    append_string(&mut bytes, string(context, "worker_bundle_id"));

    append_u32(
        &mut bytes,
        unsigned(facts, "contract_version"),
        "facts.contract_version",
    );
    append_u32(
        &mut bytes,
        unsigned(facts, "operation_version"),
        "facts.operation_version",
    );
    for field in IDENTIFIER_FIELDS {
        append_string(&mut bytes, string(identifiers, field));
    }
    append_string(&mut bytes, string(facts, "source_device_status"));
    append_string_array(&mut bytes, facts, "source_device_capabilities");
    for field in ["envelope_protocol_version", "ciphertext_length_bytes"] {
        append_u32(&mut bytes, unsigned(facts, field), field);
    }
    append_string(&mut bytes, string(facts, "idempotency_status"));
    for field in [
        "recipient_pending_count",
        "recipient_pending_limit",
        "application_envelopes_used",
        "application_envelopes_limit",
        "ciphertext_limit_bytes",
    ] {
        append_u32(&mut bytes, unsigned(facts, field), field);
    }
    append_string_array(&mut bytes, facts, "feature_flags");

    format!("{:x}", Sha256::digest(bytes))
}

fn assert_no_prohibited_fields(value: &Value, path: &str) {
    const PROHIBITED: &[&str] = &[
        "private_key",
        "session_key",
        "recovery_material",
        "plaintext",
        "ciphertext_bytes",
        "raw_signature",
        "signed_bytes",
        "auth_token",
        "idempotency_token",
        "credential",
        "connection_string",
        "sql",
        "database_row",
        "account_id",
        "device_id",
        "conversation_id",
        "envelope_id",
        "runtime_handle",
        "transaction_handle",
        "error_message",
        "log_message",
    ];
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                assert!(
                    !PROHIBITED.contains(&key.as_str()),
                    "prohibited field {path}.{key}"
                );
                assert_no_prohibited_fields(value, &format!("{path}.{key}"));
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                assert_no_prohibited_fields(value, &format!("{path}[{index}]"));
            }
        }
        Value::String(text) => {
            for marker in [
                "BEGIN PRIVATE KEY",
                "postgres://",
                "mysql://",
                "Authorization: Bearer",
                "SELECT ",
                "INSERT ",
                "UPDATE ",
                "DELETE ",
            ] {
                assert!(!text.contains(marker), "prohibited value marker at {path}");
            }
        }
        _ => {}
    }
}

fn file_hash(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

fn assert_frozen_enums(metadata: &Value) {
    assert_eq!(
        metadata["enums"],
        serde_json::json!({
            "execution_mode": ["fixture", "shadow", "guarded-development", "internal-alpha"],
            "source_device_status": ["active"],
            "source_device_capabilities": ["submit_envelope_v2"],
            "idempotency_status": ["new", "exact_duplicate", "conflict"],
            "feature_flags": ["enqueue_delivery"],
            "decision": ["store_and_enqueue", "duplicate_noop", "reject"],
            "reason": [
                "accepted_new",
                "exact_duplicate",
                "idempotency_conflict",
                "recipient_queue_full",
                "application_budget_exhausted",
                "delivery_disabled"
            ],
            "worker_error": [
                "malformed_json",
                "unknown_field",
                "duplicate_field",
                "invalid_unicode",
                "trailing_bytes",
                "unknown_enum",
                "unsupported_operation_version"
            ]
        }),
        "frozen V1 enum table changed"
    );
}

fn assert_strict_parser_error_inputs(root: &Path, metadata: &Value) {
    let inputs = metadata["strict_parser_error_inputs"]
        .as_array()
        .expect("strict_parser_error_inputs must be an array");
    let expected_codes = ["duplicate_field", "invalid_unicode", "trailing_bytes"];
    assert_eq!(inputs.len(), expected_codes.len());

    for (input, expected_code) in inputs.iter().zip(expected_codes) {
        exact_keys(input, &["path", "code", "sha256"], "strict parser input");
        assert_eq!(input["code"], expected_code);
        let relative = string(input, "path");
        assert!(relative.starts_with("errors/"));
        assert!(!relative.contains(".."));
        let path = root.join(relative);
        let bytes = fs::read(&path)
            .unwrap_or_else(|error| panic!("read strict parser input {}: {error}", path.display()));
        assert!(!bytes.is_empty(), "strict parser input must not be empty");
        assert!(bytes.len() <= 32 * 1024, "strict parser input is oversized");
        assert_eq!(file_hash(&path), string(input, "sha256"));
    }
}

#[test]
fn fixture_validator_rejects_frozen_enum_drift() {
    let mut metadata = read_json(&fixture_root().join("metadata.json"));
    metadata["enums"]["worker_error"][0] = Value::String("changed".to_owned());

    assert!(
        std::panic::catch_unwind(|| assert_frozen_enums(&metadata)).is_err(),
        "frozen enum drift must fail fixture validation"
    );
}

#[test]
fn fixture_validator_rejects_strict_parser_input_hash_drift() {
    let root = fixture_root();
    let mut metadata = read_json(&root.join("metadata.json"));
    metadata["strict_parser_error_inputs"][0]["sha256"] = Value::String("0".repeat(64));

    assert!(
        std::panic::catch_unwind(|| assert_strict_parser_error_inputs(&root, &metadata)).is_err(),
        "strict parser fixture hash drift must fail validation"
    );
}

#[test]
fn senline_v1_fixture_corpus_is_closed_bounded_and_safe() {
    let root = fixture_root();
    let metadata_path = root.join("metadata.json");
    let metadata = read_json(&metadata_path);
    assert_eq!(metadata["fixture_set_version"], 1);
    assert_eq!(metadata["protocol"]["input_max_bytes"], 32 * 1024);
    assert_eq!(metadata["protocol"]["output_max_bytes"], 8 * 1024);
    assert_eq!(metadata["protocol"]["max_in_flight"], 1);
    assert_eq!(metadata["protocol"]["stdout"], "protocol_only");
    assert_eq!(metadata["bounds"]["opaque_ascii_ref_bytes"]["min"], 1);
    assert_eq!(metadata["bounds"]["opaque_ascii_ref_bytes"]["max"], 128);
    assert_eq!(metadata["bounds"]["evaluation_id_lower_hex_bytes"], 32);
    assert_eq!(metadata["bounds"]["facts_binding_lower_hex_bytes"], 64);
    assert_eq!(
        metadata["bounds"]["sengoo_module_revision_lower_hex_bytes"],
        40
    );
    assert_eq!(metadata["bounds"]["u32_field_max"], 4_294_967_295_u64);
    assert_eq!(
        metadata["bounds"]["json_safe_u64_field_max"],
        9_007_199_254_740_991_u64
    );
    assert_eq!(
        metadata["bounds"]["source_device_capabilities_items"],
        serde_json::json!({ "min": 0, "max": 1 })
    );
    assert_eq!(
        metadata["bounds"]["feature_flags_items"],
        serde_json::json!({ "min": 0, "max": 1 })
    );
    assert_eq!(
        metadata["revision_semantics"]["sengoo_module_revision"],
        "planner_contract_fixture_revision"
    );
    assert_eq!(
        metadata["revision_semantics"]["sengoo_source_revision"],
        "immutable_bundle_source_revision"
    );
    assert_eq!(
        metadata["binding"]["context_fields"],
        serde_json::json!(BINDING_CONTEXT_FIELDS)
    );
    assert_eq!(
        metadata["binding"]["facts_fields"],
        serde_json::json!(FACT_FIELDS)
    );
    assert_eq!(
        metadata["binding"]["identifier_fields"],
        serde_json::json!(IDENTIFIER_FIELDS)
    );
    assert_frozen_enums(&metadata);
    assert_strict_parser_error_inputs(&root, &metadata);

    let expected_cases = [
        ("eligible_accept", "store_and_enqueue", "accepted_new"),
        ("exact_duplicate", "duplicate_noop", "exact_duplicate"),
        ("idempotency_conflict", "reject", "idempotency_conflict"),
        (
            "application_budget_rejection",
            "reject",
            "application_budget_exhausted",
        ),
        (
            "unknown_operation_version",
            "error",
            "unsupported_operation_version",
        ),
    ];
    let cases = metadata["cases"]
        .as_array()
        .expect("cases must be an array");
    assert_eq!(cases.len(), expected_cases.len());
    let mut binding_mismatches = Vec::new();
    let mut hash_mismatches = Vec::new();
    for (case, (name, decision, reason)) in cases.iter().zip(expected_cases) {
        assert_eq!(case["name"], name);
        assert_eq!(case["decision"], decision);
        assert_eq!(case["reason"], reason);
        let request_path = root.join(string(case, "request"));
        let response_path = root.join(string(case, "response"));
        let request_bytes = fs::read(&request_path).unwrap();
        let response_bytes = fs::read(&response_path).unwrap();
        assert!(request_bytes.len() <= 32 * 1024);
        assert!(response_bytes.len() <= 8 * 1024);

        let request = read_json(&request_path);
        exact_keys(
            &request,
            &["kind", "schema_version", "context", "facts"],
            name,
        );
        exact_keys(&request["context"], CONTEXT_FIELDS, "context");
        exact_keys(&request["facts"], FACT_FIELDS, "facts");
        exact_keys(
            &request["facts"]["identifiers"],
            IDENTIFIER_FIELDS,
            "identifiers",
        );
        assert_eq!(request["kind"], "evaluation");
        assert_eq!(request["schema_version"], 1);
        assert_eq!(request["context"]["contract_version"], 1);
        assert_eq!(request["context"]["operation"], "submit-envelope");
        assert_lower_hex(
            string(&request["context"], "evaluation_id"),
            32,
            "context.evaluation_id",
        );
        assert_lower_hex(
            string(&request["context"], "facts_binding"),
            64,
            "context.facts_binding",
        );
        assert_ascii_ref(
            string(&request["context"], "worker_bundle_id"),
            "context.worker_bundle_id",
        );
        assert!(unsigned(&request["context"], "operation_epoch") <= 9_007_199_254_740_991);
        assert!(unsigned(&request["context"], "worker_generation") <= 9_007_199_254_740_991);
        for field in IDENTIFIER_FIELDS {
            assert_ascii_ref(
                string(&request["facts"]["identifiers"], field),
                &format!("identifiers.{field}"),
            );
        }
        let actual_binding = facts_binding(&request);
        if request["context"]["facts_binding"] != actual_binding {
            binding_mismatches.push(format!("{name}: {actual_binding}"));
        }
        assert_no_prohibited_fields(&request, name);
        for (path, expected_hash) in [
            (&request_path, string(case, "request_sha256")),
            (&response_path, string(case, "response_sha256")),
        ] {
            let actual_hash = file_hash(path);
            if actual_hash != expected_hash {
                hash_mismatches.push(format!("{}: {actual_hash}", path.display()));
            }
        }

        let response = read_json(&response_path);
        assert_no_prohibited_fields(&response, name);
        if decision == "error" {
            exact_keys(
                &response,
                &["kind", "schema_version", "scope", "code", "evaluation_id"],
                "error",
            );
            assert_eq!(response["kind"], "error");
            assert_eq!(response["code"], reason);
            assert_eq!(
                response["evaluation_id"],
                request["context"]["evaluation_id"]
            );
        } else {
            exact_keys(
                &response,
                &[
                    "kind",
                    "schema_version",
                    "context",
                    "identifiers",
                    "decision",
                    "reason",
                    "sengoo_module_revision",
                ],
                "plan",
            );
            assert_eq!(response["kind"], "plan");
            assert_eq!(response["context"], request["context"]);
            assert_eq!(response["identifiers"], request["facts"]["identifiers"]);
            assert_eq!(response["decision"], decision);
            assert_eq!(response["reason"], reason);
            assert_lower_hex(
                string(&response, "sengoo_module_revision"),
                40,
                "plan.sengoo_module_revision",
            );
        }
    }
    assert!(
        binding_mismatches.is_empty(),
        "facts_binding mismatches:\n{}",
        binding_mismatches.join("\n")
    );
    assert!(
        hash_mismatches.is_empty(),
        "fixture hash mismatches:\n{}",
        hash_mismatches.join("\n")
    );

    let handshake = &metadata["handshake"];
    let handshake_path = root.join(string(handshake, "path"));
    assert_eq!(file_hash(&handshake_path), string(handshake, "sha256"));
    let handshake_json = read_json(&handshake_path);
    exact_keys(
        &handshake_json,
        &[
            "kind",
            "protocol_version",
            "sengoo_source_revision",
            "toolchain_version",
            "application_version",
            "build_manifest_id",
        ],
        "handshake",
    );

    let protocol_errors = metadata["protocol_errors"]
        .as_array()
        .expect("protocol_errors must be an array");
    assert!(!protocol_errors.is_empty());
    for error in protocol_errors {
        let path = root.join(string(error, "path"));
        assert_eq!(file_hash(&path), string(error, "sha256"));
        exact_keys(
            &read_json(&path),
            &["kind", "schema_version", "scope", "code", "evaluation_id"],
            "protocol error",
        );
    }

    let rust_only = metadata["rust_only_no_worker"]
        .as_array()
        .expect("rust_only_no_worker must be an array");
    assert_eq!(rust_only.len(), 4);
    for fixture in rust_only {
        let path = root.join(string(fixture, "path"));
        assert_eq!(file_hash(&path), string(fixture, "sha256"));
        let value = read_json(&path);
        assert_eq!(value["kind"], "rust_only_rejection");
        assert!(value.get("context").is_none());
        assert!(value.get("facts").is_none());
        assert_no_prohibited_fields(&value, "rust_only_rejection");
    }

    let generated_doc = fs::read_to_string(root.join("docs/generated/protocol-v1.md")).unwrap();
    for required in CONTEXT_FIELDS
        .iter()
        .chain(FACT_FIELDS)
        .chain(IDENTIFIER_FIELDS)
        .chain(
            [
                "32768",
                "8192",
                "protocol_only",
                "1..128",
                "4294967295",
                "9007199254740991",
                "planner contract fixture revision",
            ]
            .iter(),
        )
    {
        assert!(
            generated_doc.contains(required),
            "generated doc omits {required}"
        );
    }
}
