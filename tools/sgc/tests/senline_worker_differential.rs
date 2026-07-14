mod common;

use common::source_sgc_command;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus, Stdio};
use std::sync::mpsc::sync_channel;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PLANNER_FIXTURE_REVISION: &str = "1de09ccafa7e8f182af68e82352e2d4be39496b0";
const GENERATOR_NAME: &str = "senline-worker-differential-v1";
const GENERATOR_VERSION: u64 = 1;
const FIXED_SEED: u64 = 0x6a09_e667_f3bc_c909;
const DETERMINISM_COUNT: u64 = 512;
const REVIEWED_BOUNDARY_COUNT: u64 = 10_000;
const SEEDED_ELIGIBLE_COUNT: u64 = 100_000;
const SEEDED_PROCESS_COUNT: u64 = 8;
const SPLITMIX_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;
const SEEDED_RANDOM_VALUES_PER_CASE: u64 = 23;
const JSON_SAFE_INTEGER_MAX: u64 = 9_007_199_254_740_991;
const INPUT_MAX_BYTES: usize = 32 * 1024;
const OUTPUT_MAX_BYTES: usize = 8 * 1024;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgc crate should live under tools/sgc")
        .to_path_buf()
}

fn fixture_root() -> PathBuf {
    workspace_root().join("examples/realworld/senline-domain-worker/fixtures/v1")
}

fn sha256(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

#[test]
fn differential_corpus_metadata_is_frozen_and_linked_to_rust_fixtures() {
    let root = fixture_root();
    let fixture_metadata = fs::read(root.join("metadata.json")).expect("read fixture metadata");
    let corpus_metadata = fs::read(root.join("differential-corpus-v1.json"))
        .expect("read reviewed differential corpus metadata");
    let metadata: Value = serde_json::from_slice(&corpus_metadata).expect("parse corpus metadata");

    assert_eq!(metadata["schema_version"], 1);
    assert_eq!(metadata["reference_kind"], "independent_rust_oracle");
    assert_eq!(
        metadata["reference_scope"],
        "linked_to_frozen_rust_fixtures_not_senline_production_reference"
    );
    assert_eq!(
        metadata["frozen_fixture_metadata_sha256"],
        sha256(fixture_metadata)
    );
    assert_eq!(
        metadata["planner_contract_fixture_revision"],
        PLANNER_FIXTURE_REVISION
    );
    assert_eq!(metadata["generator"]["name"], GENERATOR_NAME);
    assert_eq!(metadata["generator"]["version"], GENERATOR_VERSION);
    assert_eq!(
        metadata["generator"]["fixed_seed_hex"],
        format!("0x{FIXED_SEED:016x}")
    );
    assert_eq!(
        metadata["corpora"]["reviewed_boundary"]["count"],
        REVIEWED_BOUNDARY_COUNT
    );
    assert_eq!(
        metadata["corpora"]["seeded_eligible"]["count"],
        SEEDED_ELIGIBLE_COUNT
    );
    assert_eq!(
        metadata["corpora"]["seeded_eligible"]["fresh_processes"],
        SEEDED_PROCESS_COUNT
    );
    assert_eq!(
        metadata["corpora"]["seeded_eligible"]["cases_per_process"],
        SEEDED_ELIGIBLE_COUNT / SEEDED_PROCESS_COUNT
    );
    for corpus in ["determinism", "reviewed_boundary", "seeded_eligible"] {
        let digest = metadata["corpora"][corpus]["transcript_sha256"]
            .as_str()
            .unwrap_or_else(|| panic!("{corpus} transcript digest must be a string"));
        assert_eq!(digest.len(), 64, "{corpus} transcript digest length");
        assert!(
            digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "{corpus} transcript digest must be lowercase hex"
        );
    }
    assert_eq!(
        metadata["coverage"]["decisions"],
        serde_json::json!(["store_and_enqueue", "duplicate_noop", "reject"])
    );
    assert_eq!(
        metadata["coverage"]["reasons"],
        serde_json::json!([
            "accepted_new",
            "exact_duplicate",
            "idempotency_conflict",
            "recipient_queue_full",
            "application_budget_exhausted",
            "delivery_disabled"
        ])
    );
    assert_eq!(
        metadata["coverage"]["execution_modes"],
        serde_json::json!(["fixture", "shadow", "guarded-development", "internal-alpha"])
    );
    assert_eq!(
        metadata["coverage"]["opaque_ascii_ref_lengths"],
        serde_json::json!([1, 128])
    );
    assert_eq!(
        metadata["coverage"]["numeric_boundaries"],
        serde_json::json!([0, 1, 4294967295_u64, 9007199254740991_u64])
    );
    assert_eq!(
        metadata["ci_targets"],
        serde_json::json!(["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"])
    );
}

#[derive(Clone, Copy, Debug)]
enum IdempotencyStatus {
    New,
    ExactDuplicate,
    Conflict,
}

impl IdempotencyStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::ExactDuplicate => "exact_duplicate",
            Self::Conflict => "conflict",
        }
    }
}

#[derive(Clone, Debug)]
struct Identifiers {
    correlation_ref: String,
    source_account_ref: String,
    source_device_ref: String,
    recipient_account_ref: String,
    recipient_device_ref: String,
    conversation_ref: String,
    envelope_ref: String,
}

impl Identifiers {
    fn values(&self) -> [&str; 7] {
        [
            &self.correlation_ref,
            &self.source_account_ref,
            &self.source_device_ref,
            &self.recipient_account_ref,
            &self.recipient_device_ref,
            &self.conversation_ref,
            &self.envelope_ref,
        ]
    }

    fn json(&self) -> Value {
        serde_json::json!({
            "correlation_ref": self.correlation_ref,
            "source_account_ref": self.source_account_ref,
            "source_device_ref": self.source_device_ref,
            "recipient_account_ref": self.recipient_account_ref,
            "recipient_device_ref": self.recipient_device_ref,
            "conversation_ref": self.conversation_ref,
            "envelope_ref": self.envelope_ref,
        })
    }
}

#[derive(Clone, Debug)]
struct OracleCase {
    index: u64,
    evaluation_id: String,
    operation_epoch: u64,
    worker_generation: u64,
    execution_mode: &'static str,
    worker_bundle_id: String,
    identifiers: Identifiers,
    has_submit_envelope_v2: bool,
    ciphertext_length_bytes: u32,
    idempotency_status: IdempotencyStatus,
    recipient_pending_count: u32,
    recipient_pending_limit: u32,
    application_envelopes_used: u32,
    application_envelopes_limit: u32,
    ciphertext_limit_bytes: u32,
    enqueue_delivery_enabled: bool,
}

#[derive(Clone, Copy, Debug)]
struct OracleOutcome {
    decision: &'static str,
    reason: &'static str,
}

fn reference_outcome(case: &OracleCase) -> OracleOutcome {
    let ordered_rules = [
        (
            matches!(case.idempotency_status, IdempotencyStatus::ExactDuplicate),
            OracleOutcome {
                decision: "duplicate_noop",
                reason: "exact_duplicate",
            },
        ),
        (
            matches!(case.idempotency_status, IdempotencyStatus::Conflict),
            OracleOutcome {
                decision: "reject",
                reason: "idempotency_conflict",
            },
        ),
        (
            case.recipient_pending_count >= case.recipient_pending_limit,
            OracleOutcome {
                decision: "reject",
                reason: "recipient_queue_full",
            },
        ),
        (
            case.application_envelopes_used >= case.application_envelopes_limit,
            OracleOutcome {
                decision: "reject",
                reason: "application_budget_exhausted",
            },
        ),
        (
            !case.has_submit_envelope_v2 || !case.enqueue_delivery_enabled,
            OracleOutcome {
                decision: "reject",
                reason: "delivery_disabled",
            },
        ),
    ];
    ordered_rules
        .into_iter()
        .find_map(|(matched, outcome)| matched.then_some(outcome))
        .unwrap_or(OracleOutcome {
            decision: "store_and_enqueue",
            reason: "accepted_new",
        })
}

fn append_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn append_string(bytes: &mut Vec<u8>, value: &str) {
    append_u32(
        bytes,
        u32::try_from(value.len()).expect("contract string length fits u32"),
    );
    bytes.extend_from_slice(value.as_bytes());
}

fn append_string_array(bytes: &mut Vec<u8>, values: &[&str]) {
    append_u32(
        bytes,
        u32::try_from(values.len()).expect("contract array length fits u32"),
    );
    for value in values {
        append_string(bytes, value);
    }
}

fn facts_binding(case: &OracleCase) -> String {
    let mut bytes = b"senline.submit-envelope.binding.v1\0".to_vec();
    append_u32(&mut bytes, 1);
    append_string(&mut bytes, "submit-envelope");
    append_u32(&mut bytes, 1);
    append_string(&mut bytes, &case.evaluation_id);
    bytes.extend_from_slice(&case.operation_epoch.to_be_bytes());
    bytes.extend_from_slice(&case.worker_generation.to_be_bytes());
    append_string(&mut bytes, case.execution_mode);
    append_string(&mut bytes, &case.worker_bundle_id);

    append_u32(&mut bytes, 1);
    append_u32(&mut bytes, 1);
    for identifier in case.identifiers.values() {
        append_string(&mut bytes, identifier);
    }
    append_string(&mut bytes, "active");
    let capabilities = if case.has_submit_envelope_v2 {
        ["submit_envelope_v2"].as_slice()
    } else {
        [].as_slice()
    };
    append_string_array(&mut bytes, capabilities);
    append_u32(&mut bytes, 2);
    append_u32(&mut bytes, case.ciphertext_length_bytes);
    append_string(&mut bytes, case.idempotency_status.as_str());
    append_u32(&mut bytes, case.recipient_pending_count);
    append_u32(&mut bytes, case.recipient_pending_limit);
    append_u32(&mut bytes, case.application_envelopes_used);
    append_u32(&mut bytes, case.application_envelopes_limit);
    append_u32(&mut bytes, case.ciphertext_limit_bytes);
    let flags = if case.enqueue_delivery_enabled {
        ["enqueue_delivery"].as_slice()
    } else {
        [].as_slice()
    };
    append_string_array(&mut bytes, flags);
    sha256(bytes)
}

fn context_json(case: &OracleCase) -> Value {
    serde_json::json!({
        "contract_version": 1,
        "operation": "submit-envelope",
        "operation_version": 1,
        "evaluation_id": case.evaluation_id,
        "operation_epoch": case.operation_epoch,
        "worker_generation": case.worker_generation,
        "execution_mode": case.execution_mode,
        "worker_bundle_id": case.worker_bundle_id,
        "facts_binding": facts_binding(case),
    })
}

fn request_bytes(case: &OracleCase) -> Vec<u8> {
    let capabilities: Vec<&str> = if case.has_submit_envelope_v2 {
        vec!["submit_envelope_v2"]
    } else {
        Vec::new()
    };
    let feature_flags: Vec<&str> = if case.enqueue_delivery_enabled {
        vec!["enqueue_delivery"]
    } else {
        Vec::new()
    };
    let request = serde_json::json!({
        "kind": "evaluation",
        "schema_version": 1,
        "context": context_json(case),
        "facts": {
            "contract_version": 1,
            "operation_version": 1,
            "identifiers": case.identifiers.json(),
            "source_device_status": "active",
            "source_device_capabilities": capabilities,
            "envelope_protocol_version": 2,
            "ciphertext_length_bytes": case.ciphertext_length_bytes,
            "idempotency_status": case.idempotency_status.as_str(),
            "recipient_pending_count": case.recipient_pending_count,
            "recipient_pending_limit": case.recipient_pending_limit,
            "application_envelopes_used": case.application_envelopes_used,
            "application_envelopes_limit": case.application_envelopes_limit,
            "ciphertext_limit_bytes": case.ciphertext_limit_bytes,
            "feature_flags": feature_flags,
        }
    });
    let bytes = serde_json::to_vec(&request).expect("serialize typed oracle request");
    assert!(
        bytes.len() <= INPUT_MAX_BYTES,
        "oracle generated oversized input"
    );
    bytes
}

fn ascii_ref(domain: u64, index: u64, len: usize) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_-";
    let mut state = domain ^ index.rotate_left(17) ^ 0xa409_3822_299f_31d0;
    let mut value = String::with_capacity(len);
    for _ in 0..len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        value.push(ALPHABET[(state as usize) % ALPHABET.len()] as char);
    }
    value
}

fn identifiers(index: u64, len: usize) -> Identifiers {
    Identifiers {
        correlation_ref: ascii_ref(1, index, len),
        source_account_ref: ascii_ref(2, index, len),
        source_device_ref: ascii_ref(3, index, len),
        recipient_account_ref: ascii_ref(4, index, len),
        recipient_device_ref: ascii_ref(5, index, len),
        conversation_ref: ascii_ref(6, index, len),
        envelope_ref: ascii_ref(7, index, len),
    }
}

fn evaluation_id(high: u64, low: u64) -> String {
    format!("{high:016x}{low:016x}")
}

fn reviewed_boundary_case(index: u64) -> OracleCase {
    const MODES: [&str; 4] = ["fixture", "shadow", "guarded-development", "internal-alpha"];
    let variant = index / 6;
    let reference_len = if index % 2 == 0 { 1 } else { 128 };
    let (ciphertext_length_bytes, ciphertext_limit_bytes) = match variant % 4 {
        0 => (0, 0),
        1 => (1, 1),
        2 => (u32::MAX, u32::MAX),
        _ => (1, u32::MAX),
    };
    let relation = variant % 3;
    let relation_values = match relation {
        0 => (0, 1),
        1 => (1, 1),
        _ => (2, 1),
    };
    let mut case = OracleCase {
        index,
        evaluation_id: evaluation_id(index, index ^ 0xbb67_ae85_84ca_a73b),
        operation_epoch: [0, 1, JSON_SAFE_INTEGER_MAX][(variant % 3) as usize],
        worker_generation: [JSON_SAFE_INTEGER_MAX, 0, 1][(variant % 3) as usize],
        execution_mode: MODES[(index % MODES.len() as u64) as usize],
        worker_bundle_id: ascii_ref(8, index, reference_len),
        identifiers: identifiers(index, reference_len),
        has_submit_envelope_v2: variant % 2 == 0,
        ciphertext_length_bytes,
        idempotency_status: IdempotencyStatus::New,
        recipient_pending_count: relation_values.0,
        recipient_pending_limit: relation_values.1,
        application_envelopes_used: relation_values.0,
        application_envelopes_limit: relation_values.1,
        ciphertext_limit_bytes,
        enqueue_delivery_enabled: variant % 4 < 2,
    };
    match index % 6 {
        0 => {
            case.recipient_pending_count = 0;
            case.recipient_pending_limit = 1;
            case.application_envelopes_used = 0;
            case.application_envelopes_limit = 1;
            case.has_submit_envelope_v2 = true;
            case.enqueue_delivery_enabled = true;
        }
        1 => case.idempotency_status = IdempotencyStatus::ExactDuplicate,
        2 => case.idempotency_status = IdempotencyStatus::Conflict,
        3 => {
            case.recipient_pending_count = if variant % 2 == 0 { 1 } else { 2 };
            case.recipient_pending_limit = 1;
        }
        4 => {
            case.recipient_pending_count = 0;
            case.recipient_pending_limit = 1;
            case.application_envelopes_used = if variant % 2 == 0 { 1 } else { 2 };
            case.application_envelopes_limit = 1;
        }
        _ => {
            case.recipient_pending_count = 0;
            case.recipient_pending_limit = 1;
            case.application_envelopes_used = 0;
            case.application_envelopes_limit = 1;
            if variant % 2 == 0 {
                case.has_submit_envelope_v2 = false;
                case.enqueue_delivery_enabled = true;
            } else {
                case.has_submit_envelope_v2 = true;
                case.enqueue_delivery_enabled = false;
            }
        }
    }
    case
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(SPLITMIX_GAMMA);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn bool(&mut self) -> bool {
        self.next() & 1 == 1
    }

    fn at_case(seed: u64, index: u64) -> Self {
        Self::new(seed.wrapping_add(
            SPLITMIX_GAMMA.wrapping_mul(index.wrapping_mul(SEEDED_RANDOM_VALUES_PER_CASE)),
        ))
    }
}

fn seeded_case(index: u64, random: &mut SplitMix64) -> OracleCase {
    const MODES: [&str; 4] = ["fixture", "shadow", "guarded-development", "internal-alpha"];
    let reference_len = (random.next() % 128 + 1) as usize;
    let ciphertext_limit_bytes = random.next() as u32;
    let ciphertext_length_bytes = (random.next() % (u64::from(ciphertext_limit_bytes) + 1)) as u32;
    let idempotency_status = match random.next() % 3 {
        0 => IdempotencyStatus::New,
        1 => IdempotencyStatus::ExactDuplicate,
        _ => IdempotencyStatus::Conflict,
    };
    OracleCase {
        index,
        evaluation_id: evaluation_id(random.next(), random.next()),
        operation_epoch: random.next() % (JSON_SAFE_INTEGER_MAX + 1),
        worker_generation: random.next() % (JSON_SAFE_INTEGER_MAX + 1),
        execution_mode: MODES[(random.next() % MODES.len() as u64) as usize],
        worker_bundle_id: ascii_ref(random.next(), index, reference_len),
        identifiers: Identifiers {
            correlation_ref: ascii_ref(random.next(), index, reference_len),
            source_account_ref: ascii_ref(random.next(), index, reference_len),
            source_device_ref: ascii_ref(random.next(), index, reference_len),
            recipient_account_ref: ascii_ref(random.next(), index, reference_len),
            recipient_device_ref: ascii_ref(random.next(), index, reference_len),
            conversation_ref: ascii_ref(random.next(), index, reference_len),
            envelope_ref: ascii_ref(random.next(), index, reference_len),
        },
        has_submit_envelope_v2: random.bool(),
        ciphertext_length_bytes,
        idempotency_status,
        recipient_pending_count: random.next() as u32,
        recipient_pending_limit: random.next() as u32,
        application_envelopes_used: random.next() as u32,
        application_envelopes_limit: random.next() as u32,
        ciphertext_limit_bytes,
        enqueue_delivery_enabled: random.bool(),
    }
}

#[derive(Clone, Copy)]
enum CorpusKind {
    ReviewedBoundary,
    SeededEligible,
}

#[derive(Clone, Copy)]
struct CorpusSpec {
    name: &'static str,
    kind: CorpusKind,
    start_index: u64,
    count: u64,
    timeout: Duration,
    collect_responses: bool,
}

struct WorkerTempDir {
    path: PathBuf,
}

impl WorkerTempDir {
    fn new(tag: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sengoo-worker-differential-{tag}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create differential temp directory");
        Self { path }
    }
}

impl Drop for WorkerTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn worker_root() -> PathBuf {
    workspace_root().join("examples/realworld/senline-domain-worker")
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
    .expect("encode differential worker module map")
}

fn build_worker(root: &WorkerTempDir) -> PathBuf {
    let worker = worker_root();
    let executable = root.path.join(if cfg!(windows) {
        "senline-domain-worker-differential.exe"
    } else {
        "senline-domain-worker-differential"
    });
    let output = source_sgc_command()
        .arg("build")
        .arg(worker.join("src/main.sg"))
        .arg("--output")
        .arg(&executable)
        .args(["-O", "3", "--force-rebuild"])
        .current_dir(&worker)
        .env("SENGOO_MODULE_MAP", worker_module_map(&worker))
        .output()
        .expect("build differential worker");
    assert!(
        output.status.success(),
        "differential worker build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    executable
}

fn write_frame(writer: &mut impl Write, payload: &[u8]) -> Result<(), String> {
    let len = u32::try_from(payload.len()).map_err(|_| "payload length exceeds u32".to_owned())?;
    writer
        .write_all(&len.to_be_bytes())
        .and_then(|()| writer.write_all(payload))
        .map_err(|error| format!("write worker frame: {error}"))
}

fn read_frame(reader: &mut impl Read, max_len: usize) -> Result<Vec<u8>, String> {
    let mut prefix = [0_u8; 4];
    reader
        .read_exact(&mut prefix)
        .map_err(|error| format!("read worker frame prefix: {error}"))?;
    let len = u32::from_be_bytes(prefix) as usize;
    if len == 0 || len > max_len {
        return Err(format!(
            "worker frame length {len} is outside 1..={max_len}"
        ));
    }
    let mut payload = vec![0_u8; len];
    reader
        .read_exact(&mut payload)
        .map_err(|error| format!("read worker frame payload: {error}"))?;
    Ok(payload)
}

fn exact_keys(value: &Value, expected: &[&str], label: &str) -> Result<(), String> {
    let fields = value
        .as_object()
        .ok_or_else(|| format!("{label} must be an object"))?;
    let actual = fields.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{label} exact keys differ: {actual:?}"))
    }
}

fn validate_plan(payload: &[u8], case: &OracleCase) -> Result<OracleOutcome, String> {
    if payload.len() > OUTPUT_MAX_BYTES {
        return Err(format!("case {} output exceeds 8 KiB", case.index));
    }
    if payload.last() != Some(&b'\n') {
        return Err(format!("case {} plan lacks one trailing LF", case.index));
    }
    if payload[..payload.len() - 1]
        .iter()
        .any(|byte| matches!(byte, b'\n' | b'\r' | b'\t'))
    {
        return Err(format!(
            "case {} plan is not normalized single-line JSON",
            case.index
        ));
    }
    let plan: Value = serde_json::from_slice(payload)
        .map_err(|error| format!("case {} malformed plan JSON: {error}", case.index))?;
    exact_keys(
        &plan,
        &[
            "kind",
            "schema_version",
            "context",
            "identifiers",
            "decision",
            "reason",
            "sengoo_module_revision",
        ],
        "SubmitEnvelopePlanV1",
    )?;
    exact_keys(
        &plan["context"],
        &[
            "contract_version",
            "operation",
            "operation_version",
            "evaluation_id",
            "operation_epoch",
            "worker_generation",
            "execution_mode",
            "worker_bundle_id",
            "facts_binding",
        ],
        "EvaluationContextV1",
    )?;
    exact_keys(
        &plan["identifiers"],
        &[
            "correlation_ref",
            "source_account_ref",
            "source_device_ref",
            "recipient_account_ref",
            "recipient_device_ref",
            "conversation_ref",
            "envelope_ref",
        ],
        "SubmitEnvelopeIdentifiersV1",
    )?;
    let outcome = reference_outcome(case);
    let expected = [
        ("kind", Value::String("plan".to_owned())),
        ("schema_version", Value::from(1)),
        ("context", context_json(case)),
        ("identifiers", case.identifiers.json()),
        ("decision", Value::String(outcome.decision.to_owned())),
        ("reason", Value::String(outcome.reason.to_owned())),
        (
            "sengoo_module_revision",
            Value::String(PLANNER_FIXTURE_REVISION.to_owned()),
        ),
    ];
    for (field, expected) in expected {
        if plan[field] != expected {
            return Err(format!(
                "case {} field {field} mismatch: actual={} expected={expected}",
                case.index, plan[field]
            ));
        }
    }
    Ok(outcome)
}

#[derive(Default)]
struct Coverage {
    decisions: BTreeMap<&'static str, u64>,
    reasons: BTreeMap<&'static str, u64>,
    execution_modes: BTreeSet<&'static str>,
    reference_lengths: BTreeSet<usize>,
    queue_relations: BTreeSet<&'static str>,
    application_relations: BTreeSet<&'static str>,
    capabilities: BTreeSet<bool>,
    feature_flags: BTreeSet<bool>,
    saw_u32_max: bool,
    saw_json_safe_max: bool,
}

fn relation(left: u32, right: u32) -> &'static str {
    match left.cmp(&right) {
        std::cmp::Ordering::Less => "below",
        std::cmp::Ordering::Equal => "equal",
        std::cmp::Ordering::Greater => "above",
    }
}

impl Coverage {
    fn observe(&mut self, case: &OracleCase, outcome: OracleOutcome) {
        *self.decisions.entry(outcome.decision).or_default() += 1;
        *self.reasons.entry(outcome.reason).or_default() += 1;
        self.execution_modes.insert(case.execution_mode);
        self.reference_lengths
            .insert(case.identifiers.correlation_ref.len());
        self.queue_relations.insert(relation(
            case.recipient_pending_count,
            case.recipient_pending_limit,
        ));
        self.application_relations.insert(relation(
            case.application_envelopes_used,
            case.application_envelopes_limit,
        ));
        self.capabilities.insert(case.has_submit_envelope_v2);
        self.feature_flags.insert(case.enqueue_delivery_enabled);
        self.saw_u32_max |= [
            case.ciphertext_length_bytes,
            case.ciphertext_limit_bytes,
            case.recipient_pending_count,
            case.recipient_pending_limit,
            case.application_envelopes_used,
            case.application_envelopes_limit,
        ]
        .contains(&u32::MAX);
        self.saw_json_safe_max |= case.operation_epoch == JSON_SAFE_INTEGER_MAX
            || case.worker_generation == JSON_SAFE_INTEGER_MAX;
    }

    fn assert_reviewed_boundary_coverage(&self) {
        assert_eq!(
            self.decisions.keys().copied().collect::<BTreeSet<_>>(),
            BTreeSet::from(["duplicate_noop", "reject", "store_and_enqueue"])
        );
        assert_eq!(
            self.reasons.keys().copied().collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "accepted_new",
                "exact_duplicate",
                "idempotency_conflict",
                "recipient_queue_full",
                "application_budget_exhausted",
                "delivery_disabled",
            ])
        );
        assert_eq!(
            self.execution_modes,
            BTreeSet::from(["fixture", "guarded-development", "internal-alpha", "shadow"])
        );
        assert_eq!(self.reference_lengths, BTreeSet::from([1, 128]));
        assert_eq!(
            self.queue_relations,
            BTreeSet::from(["above", "below", "equal"])
        );
        assert_eq!(
            self.application_relations,
            BTreeSet::from(["above", "below", "equal"])
        );
        assert_eq!(self.capabilities, BTreeSet::from([false, true]));
        assert_eq!(self.feature_flags, BTreeSet::from([false, true]));
        assert!(self.saw_u32_max, "reviewed corpus omitted u32::MAX");
        assert!(
            self.saw_json_safe_max,
            "reviewed corpus omitted the JSON-safe integer maximum"
        );
    }
}

struct WaitOutcome {
    status: ExitStatus,
    timed_out: bool,
}

fn wait_with_watchdog(mut child: Child, timeout: Duration) -> WaitOutcome {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return WaitOutcome {
                    status,
                    timed_out: false,
                };
            }
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let status = child.wait().expect("reap watchdog-killed worker");
                return WaitOutcome {
                    status,
                    timed_out: true,
                };
            }
            Err(error) => panic!("poll differential worker: {error}"),
        }
    }
}

struct CorpusOutcome {
    transcript_sha256: String,
    responses: Vec<Vec<u8>>,
    coverage: Coverage,
    elapsed: Duration,
}

fn update_transcript(hasher: &mut Sha256, request: &[u8], response: &[u8]) {
    hasher.update((request.len() as u32).to_be_bytes());
    hasher.update(request);
    hasher.update((response.len() as u32).to_be_bytes());
    hasher.update(response);
}

fn run_corpus(executable: &Path, spec: CorpusSpec) -> CorpusOutcome {
    let started = Instant::now();
    let mut child = std::process::Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn differential worker");
    let mut stdin = child.stdin.take().expect("worker stdin pipe");
    let mut stdout = child.stdout.take().expect("worker stdout pipe");
    let mut stderr = child.stderr.take().expect("worker stderr pipe");
    let (expected_sender, expected_receiver) = sync_channel::<(OracleCase, Vec<u8>)>(0);
    let writer = thread::spawn(move || -> Result<(), String> {
        let mut random = SplitMix64::at_case(FIXED_SEED, spec.start_index);
        for index in spec.start_index..spec.start_index + spec.count {
            let case = match spec.kind {
                CorpusKind::ReviewedBoundary => reviewed_boundary_case(index),
                CorpusKind::SeededEligible => seeded_case(index, &mut random),
            };
            let request = request_bytes(&case);
            write_frame(&mut stdin, &request)?;
            expected_sender
                .send((case, request))
                .map_err(|_| "differential oracle receiver closed".to_owned())?;
        }
        Ok(())
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr
            .read_to_end(&mut bytes)
            .expect("drain differential worker stderr");
        bytes
    });
    let watchdog = thread::spawn(move || wait_with_watchdog(child, spec.timeout));

    let fixture_handshake =
        fs::read(fixture_root().join("handshake/ready.json")).expect("read frozen handshake");
    let mut failure = match read_frame(&mut stdout, OUTPUT_MAX_BYTES) {
        Ok(handshake) if handshake == fixture_handshake => None,
        Ok(_) => Some("worker handshake differs from frozen fixture".to_owned()),
        Err(error) => Some(format!("worker handshake failed: {error}")),
    };
    let mut transcript = Sha256::new();
    transcript.update(b"senline-worker-differential-transcript-v1\0");
    transcript.update(GENERATOR_NAME.as_bytes());
    transcript.update(GENERATOR_VERSION.to_be_bytes());
    transcript.update(FIXED_SEED.to_be_bytes());
    transcript.update(spec.start_index.to_be_bytes());
    transcript.update(spec.count.to_be_bytes());
    let mut coverage = Coverage::default();
    let mut responses = if spec.collect_responses {
        Vec::with_capacity(spec.count as usize)
    } else {
        Vec::new()
    };
    let mut received = 0_u64;
    while received < spec.count {
        let (case, request) = match expected_receiver.recv() {
            Ok(expected) => expected,
            Err(_) => {
                failure.get_or_insert_with(|| {
                    format!("oracle stream ended after {received}/{} cases", spec.count)
                });
                break;
            }
        };
        let response = match read_frame(&mut stdout, OUTPUT_MAX_BYTES) {
            Ok(response) => response,
            Err(error) => {
                failure.get_or_insert_with(|| {
                    format!("case {} response frame failed: {error}", case.index)
                });
                break;
            }
        };
        update_transcript(&mut transcript, &request, &response);
        match validate_plan(&response, &case) {
            Ok(outcome) => coverage.observe(&case, outcome),
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
        if spec.collect_responses {
            responses.push(response);
        }
        received += 1;
    }
    drop(expected_receiver);
    let writer_result = writer.join().expect("differential writer thread panicked");
    let mut trailing_stdout = Vec::new();
    stdout
        .read_to_end(&mut trailing_stdout)
        .expect("drain trailing worker stdout");
    let wait = watchdog.join().expect("differential watchdog panicked");
    let stderr = stderr_reader
        .join()
        .expect("differential stderr reader panicked");

    if let Err(error) = writer_result {
        failure.get_or_insert(error);
    }
    if wait.timed_out {
        failure.get_or_insert_with(|| format!("{} worker timed out", spec.name));
    }
    if !wait.status.success() {
        failure.get_or_insert_with(|| {
            format!("{} worker exited with {:?}", spec.name, wait.status.code())
        });
    }
    if !stderr.is_empty() {
        failure.get_or_insert_with(|| {
            format!(
                "{} worker stderr was not empty: {}",
                spec.name,
                String::from_utf8_lossy(&stderr)
            )
        });
    }
    if !trailing_stdout.is_empty() {
        failure.get_or_insert_with(|| {
            format!(
                "{} worker emitted {} surplus stdout bytes",
                spec.name,
                trailing_stdout.len()
            )
        });
    }
    if received != spec.count {
        failure.get_or_insert_with(|| {
            format!("{} completed {received}/{} cases", spec.name, spec.count)
        });
    }
    if let Some(failure) = failure {
        panic!("{} differential failure: {failure}", spec.name);
    }

    CorpusOutcome {
        transcript_sha256: format!("{:x}", transcript.finalize()),
        responses,
        coverage,
        elapsed: started.elapsed(),
    }
}

fn corpus_metadata() -> Value {
    serde_json::from_slice(
        &fs::read(fixture_root().join("differential-corpus-v1.json"))
            .expect("read differential corpus metadata"),
    )
    .expect("parse differential corpus metadata")
}

fn expected_transcript(metadata: &Value, corpus: &str) -> String {
    metadata["corpora"][corpus]["transcript_sha256"]
        .as_str()
        .unwrap_or_else(|| panic!("missing {corpus} transcript digest"))
        .to_owned()
}

fn write_evidence(spec: CorpusSpec, process_count: u64, outcome: &CorpusOutcome) {
    let directory = workspace_root().join("target/senline-differential");
    fs::create_dir_all(&directory).expect("create differential evidence directory");
    let evidence = serde_json::json!({
        "schema_version": 1,
        "corpus": spec.name,
        "generator": GENERATOR_NAME,
        "generator_version": GENERATOR_VERSION,
        "fixed_seed_hex": format!("0x{FIXED_SEED:016x}"),
        "case_count": spec.count,
        "fresh_processes": process_count,
        "transcript_sha256": outcome.transcript_sha256,
        "platform": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "worker_status": "clean_exit",
        "semantic_mismatches": 0,
        "crashes": 0,
        "hangs": 0,
        "malformed_plans": 0,
        "nondeterministic_plans": 0,
        "elapsed_millis": outcome.elapsed.as_millis(),
    });
    let path = directory.join(format!(
        "{}-{}-{}.json",
        spec.name,
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    fs::write(
        path,
        serde_json::to_vec_pretty(&evidence).expect("serialize differential evidence"),
    )
    .expect("write differential evidence");
}

#[test]
fn identical_inputs_have_identical_raw_plan_bytes_across_fresh_processes() {
    let root = WorkerTempDir::new("determinism");
    let executable = build_worker(&root);
    let spec = CorpusSpec {
        name: "determinism",
        kind: CorpusKind::ReviewedBoundary,
        start_index: 0,
        count: DETERMINISM_COUNT,
        timeout: Duration::from_secs(120),
        collect_responses: true,
    };
    let first = run_corpus(&executable, spec);
    let second = run_corpus(&executable, spec);
    assert_eq!(first.responses.len(), DETERMINISM_COUNT as usize);
    assert_eq!(
        first.responses, second.responses,
        "fresh workers emitted different raw normalized plan bytes"
    );
    assert_eq!(first.transcript_sha256, second.transcript_sha256);
    let expected = expected_transcript(&corpus_metadata(), spec.name);
    assert_eq!(
        first.transcript_sha256, expected,
        "determinism transcript changed"
    );
    println!(
        "senline-determinism-transcript-sha256={} cases={} fresh_processes=2",
        first.transcript_sha256, spec.count
    );
    write_evidence(spec, 2, &first);
}

#[test]
#[ignore = "release differential corpus: 10,000 reviewed boundary cases"]
fn reviewed_boundary_corpus_matches_independent_rust_oracle() {
    let root = WorkerTempDir::new("reviewed-boundary");
    let executable = build_worker(&root);
    let spec = CorpusSpec {
        name: "reviewed_boundary",
        kind: CorpusKind::ReviewedBoundary,
        start_index: 0,
        count: REVIEWED_BOUNDARY_COUNT,
        timeout: Duration::from_secs(300),
        collect_responses: false,
    };
    let outcome = run_corpus(&executable, spec);
    outcome.coverage.assert_reviewed_boundary_coverage();
    let expected = expected_transcript(&corpus_metadata(), spec.name);
    assert_eq!(
        outcome.transcript_sha256, expected,
        "reviewed boundary transcript changed"
    );
    println!(
        "senline-reviewed-boundary-transcript-sha256={} cases={} elapsed_ms={}",
        outcome.transcript_sha256,
        spec.count,
        outcome.elapsed.as_millis()
    );
    write_evidence(spec, 1, &outcome);
}

fn run_seeded_corpus(executable: &Path) -> CorpusOutcome {
    assert_eq!(SEEDED_ELIGIBLE_COUNT % SEEDED_PROCESS_COUNT, 0);
    let started = Instant::now();
    let cases_per_process = SEEDED_ELIGIBLE_COUNT / SEEDED_PROCESS_COUNT;
    let mut workers = Vec::new();
    for shard in 0..SEEDED_PROCESS_COUNT {
        let executable = executable.to_path_buf();
        let spec = CorpusSpec {
            name: "seeded_eligible_shard",
            kind: CorpusKind::SeededEligible,
            start_index: shard * cases_per_process,
            count: cases_per_process,
            timeout: Duration::from_secs(1200),
            collect_responses: false,
        };
        workers.push(thread::spawn(move || run_corpus(&executable, spec)));
    }
    let outcomes = workers
        .into_iter()
        .enumerate()
        .map(|(shard, worker)| {
            worker
                .join()
                .unwrap_or_else(|_| panic!("seeded differential shard {shard} panicked"))
        })
        .collect::<Vec<_>>();
    let mut transcript = Sha256::new();
    transcript.update(b"senline-worker-differential-shards-v1\0");
    transcript.update(GENERATOR_NAME.as_bytes());
    transcript.update(GENERATOR_VERSION.to_be_bytes());
    transcript.update(FIXED_SEED.to_be_bytes());
    transcript.update(SEEDED_ELIGIBLE_COUNT.to_be_bytes());
    transcript.update(SEEDED_PROCESS_COUNT.to_be_bytes());
    for (shard, outcome) in outcomes.iter().enumerate() {
        transcript.update((shard as u64 * cases_per_process).to_be_bytes());
        transcript.update(cases_per_process.to_be_bytes());
        transcript.update(outcome.transcript_sha256.as_bytes());
    }
    CorpusOutcome {
        transcript_sha256: format!("{:x}", transcript.finalize()),
        responses: Vec::new(),
        coverage: Coverage::default(),
        elapsed: started.elapsed(),
    }
}

#[test]
#[ignore = "release differential corpus: 100,000 fixed-seed eligible cases"]
fn seeded_eligible_corpus_matches_independent_rust_oracle() {
    let root = WorkerTempDir::new("seeded-eligible");
    let executable = build_worker(&root);
    let spec = CorpusSpec {
        name: "seeded_eligible",
        kind: CorpusKind::SeededEligible,
        start_index: 0,
        count: SEEDED_ELIGIBLE_COUNT,
        timeout: Duration::from_secs(1200),
        collect_responses: false,
    };
    let outcome = run_seeded_corpus(&executable);
    let expected = expected_transcript(&corpus_metadata(), spec.name);
    assert_eq!(
        outcome.transcript_sha256, expected,
        "seeded eligible transcript changed"
    );
    println!(
        "senline-seeded-eligible-transcript-sha256={} cases={} elapsed_ms={}",
        outcome.transcript_sha256,
        spec.count,
        outcome.elapsed.as_millis()
    );
    write_evidence(spec, SEEDED_PROCESS_COUNT, &outcome);
}
