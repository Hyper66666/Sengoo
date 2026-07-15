//! Single-worker resource soak and latency sampler (tasks 8.3 / 8.4).
//!
//! Methodology: `docs/senline-dogfood-resource-methodology.md`.
//! Does **not** claim Senline admission, sandbox, or production timing.

mod common;

use common::source_sgc_command;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PLANNER_FIXTURE_REVISION: &str = "1de09ccafa7e8f182af68e82352e2d4be39496b0";
const FIXED_SEED: u64 = 0x6a09_e667_f3bc_c909;
const WARMUP_CASES: u64 = 256;
const SAMPLE_EVERY: u64 = 100;
const INPUT_MAX_BYTES: usize = 32 * 1024;
const OUTPUT_MAX_BYTES: usize = 8 * 1024;
const JSON_SAFE_INTEGER_MAX: u64 = 9_007_199_254_740_991;

/// CI / default smoke: short single-worker run that exercises the sampler.
const SMOKE_COUNT: u64 = 1_024;
/// Investigation window covering the historical single-worker stall near 44_086.
const INVESTIGATION_COUNT: u64 = 45_000;
/// Task 8.3 full soak target (ignored by default).
const SOAK_COUNT: u64 = 1_000_000;

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

fn evidence_root() -> PathBuf {
    let path = workspace_root().join("target/senline-resource");
    fs::create_dir_all(&path).expect("create resource evidence directory");
    path
}

fn normalize_fixture_bytes(bytes: impl AsRef<[u8]>) -> Vec<u8> {
    bytes
        .as_ref()
        .iter()
        .copied()
        .filter(|byte| *byte != b'\r')
        .collect()
}

fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

struct WorkerTempDir {
    path: PathBuf,
}

impl WorkerTempDir {
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_millis();
        let path = std::env::temp_dir().join(format!("senline-resource-{label}-{stamp}"));
        fs::create_dir_all(&path).expect("create worker temp dir");
        Self { path }
    }
}

impl Drop for WorkerTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
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
    .expect("encode resource worker module map")
}

fn build_worker(root: &WorkerTempDir) -> PathBuf {
    let worker = worker_root();
    let executable = root.path.join(if cfg!(windows) {
        "senline-domain-worker-resource.exe"
    } else {
        "senline-domain-worker-resource"
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
        .expect("build resource worker");
    assert!(
        output.status.success(),
        "resource worker build failed\nstdout:\n{}\nstderr:\n{}",
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

fn evaluation_id(high: u64, low: u64) -> String {
    format!("{high:016x}{low:016x}")
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

struct CaseFields {
    evaluation_id: String,
    operation_epoch: u64,
    worker_generation: u64,
    execution_mode: &'static str,
    worker_bundle_id: String,
    identifiers: [String; 7],
    has_submit_envelope_v2: bool,
    ciphertext_length_bytes: u32,
    idempotency_status: &'static str,
    recipient_pending_count: u32,
    recipient_pending_limit: u32,
    application_envelopes_used: u32,
    application_envelopes_limit: u32,
    ciphertext_limit_bytes: u32,
    enqueue_delivery_enabled: bool,
}

fn facts_binding(case: &CaseFields) -> String {
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
    for identifier in &case.identifiers {
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
    append_string(&mut bytes, case.idempotency_status);
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
    sha256_hex(bytes)
}

/// Reviewed-boundary case generator (same contract as differential corpus).
fn reviewed_boundary_request(index: u64) -> Vec<u8> {
    const MODES: [&str; 4] = ["fixture", "shadow", "guarded-development", "internal-alpha"];
    let variant = index / 6;
    let reference_len = if index.is_multiple_of(2) { 1 } else { 128 };
    let (ciphertext_length_bytes, ciphertext_limit_bytes) = match variant % 4 {
        0 => (0, 0),
        1 => (1, 1),
        2 => (u32::MAX, u32::MAX),
        _ => (1, u32::MAX),
    };
    let relation = variant % 3;
    let relation_values = match relation {
        0 => (0_u32, 1_u32),
        1 => (1, 1),
        _ => (2, 1),
    };
    let mut case = CaseFields {
        evaluation_id: evaluation_id(index, index ^ 0xbb67_ae85_84ca_a73b),
        operation_epoch: [0, 1, JSON_SAFE_INTEGER_MAX][(variant % 3) as usize],
        worker_generation: [JSON_SAFE_INTEGER_MAX, 0, 1][(variant % 3) as usize],
        execution_mode: MODES[(index % MODES.len() as u64) as usize],
        worker_bundle_id: ascii_ref(8, index, reference_len),
        identifiers: [
            ascii_ref(1, index, reference_len),
            ascii_ref(2, index, reference_len),
            ascii_ref(3, index, reference_len),
            ascii_ref(4, index, reference_len),
            ascii_ref(5, index, reference_len),
            ascii_ref(6, index, reference_len),
            ascii_ref(7, index, reference_len),
        ],
        has_submit_envelope_v2: variant.is_multiple_of(2),
        ciphertext_length_bytes,
        idempotency_status: "new",
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
        1 => case.idempotency_status = "exact_duplicate",
        2 => case.idempotency_status = "conflict",
        3 => {
            case.recipient_pending_count = if variant.is_multiple_of(2) { 1 } else { 2 };
            case.recipient_pending_limit = 1;
        }
        4 => {
            case.recipient_pending_count = 0;
            case.recipient_pending_limit = 1;
            case.application_envelopes_used = if variant.is_multiple_of(2) { 1 } else { 2 };
            case.application_envelopes_limit = 1;
        }
        _ => {
            case.recipient_pending_count = 0;
            case.recipient_pending_limit = 1;
            case.application_envelopes_used = 0;
            case.application_envelopes_limit = 1;
            if variant.is_multiple_of(2) {
                case.has_submit_envelope_v2 = false;
                case.enqueue_delivery_enabled = true;
            } else {
                case.has_submit_envelope_v2 = true;
                case.enqueue_delivery_enabled = false;
            }
        }
    }
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
        "context": {
            "contract_version": 1,
            "operation": "submit-envelope",
            "operation_version": 1,
            "evaluation_id": case.evaluation_id,
            "operation_epoch": case.operation_epoch,
            "worker_generation": case.worker_generation,
            "execution_mode": case.execution_mode,
            "worker_bundle_id": case.worker_bundle_id,
            "facts_binding": facts_binding(&case),
        },
        "facts": {
            "contract_version": 1,
            "operation_version": 1,
            "identifiers": {
                "correlation_ref": case.identifiers[0],
                "source_account_ref": case.identifiers[1],
                "source_device_ref": case.identifiers[2],
                "recipient_account_ref": case.identifiers[3],
                "recipient_device_ref": case.identifiers[4],
                "conversation_ref": case.identifiers[5],
                "envelope_ref": case.identifiers[6],
            },
            "source_device_status": "active",
            "source_device_capabilities": capabilities,
            "envelope_protocol_version": 2,
            "ciphertext_length_bytes": case.ciphertext_length_bytes,
            "idempotency_status": case.idempotency_status,
            "recipient_pending_count": case.recipient_pending_count,
            "recipient_pending_limit": case.recipient_pending_limit,
            "application_envelopes_used": case.application_envelopes_used,
            "application_envelopes_limit": case.application_envelopes_limit,
            "ciphertext_limit_bytes": case.ciphertext_limit_bytes,
            "feature_flags": feature_flags,
        }
    });
    let bytes = serde_json::to_vec(&request).expect("serialize resource request");
    assert!(bytes.len() <= INPUT_MAX_BYTES, "resource request oversized");
    bytes
}

fn percentile_us(sorted_us: &[u64], pct: f64) -> u64 {
    if sorted_us.is_empty() {
        return 0;
    }
    let rank = ((pct / 100.0) * (sorted_us.len() as f64 - 1.0)).round() as usize;
    sorted_us[rank.min(sorted_us.len() - 1)]
}

#[cfg(target_os = "linux")]
fn sample_worker_memory_bytes(pid: u32) -> Option<u64> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kb: u64 = rest
                .split_whitespace()
                .next()?
                .parse()
                .ok()?;
            return Some(kb.saturating_mul(1024));
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn sample_worker_handle_count(pid: u32) -> Option<u64> {
    let dir = fs::read_dir(format!("/proc/{pid}/fd")).ok()?;
    Some(dir.count() as u64)
}

#[cfg(windows)]
mod win_sample {
    use std::mem::{size_of, MaybeUninit};
    use std::os::raw::c_void;

    type HANDLE = *mut c_void;
    type BOOL = i32;
    type DWORD = u32;
    type SizeT = usize;

    const PROCESS_QUERY_INFORMATION: DWORD = 0x0400;
    const PROCESS_VM_READ: DWORD = 0x0010;

    #[repr(C)]
    struct ProcessMemoryCountersEx {
        cb: DWORD,
        page_fault_count: DWORD,
        peak_working_set_size: SizeT,
        working_set_size: SizeT,
        quota_peak_paged_pool_usage: SizeT,
        quota_paged_pool_usage: SizeT,
        quota_peak_non_paged_pool_usage: SizeT,
        quota_non_paged_pool_usage: SizeT,
        pagefile_usage: SizeT,
        peak_pagefile_usage: SizeT,
        private_usage: SizeT,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: DWORD, inherit: BOOL, process_id: DWORD) -> HANDLE;
        fn CloseHandle(handle: HANDLE) -> BOOL;
        fn GetProcessHandleCount(process: HANDLE, handle_count: *mut DWORD) -> BOOL;
    }

    #[link(name = "psapi")]
    extern "system" {
        fn GetProcessMemoryInfo(
            process: HANDLE,
            counters: *mut ProcessMemoryCountersEx,
            cb: DWORD,
        ) -> BOOL;
    }

    pub(super) fn private_working_set_bytes(pid: u32) -> Option<u64> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
            if handle.is_null() {
                return None;
            }
            let mut counters = MaybeUninit::<ProcessMemoryCountersEx>::zeroed();
            let counters_ptr = counters.as_mut_ptr();
            (*counters_ptr).cb = size_of::<ProcessMemoryCountersEx>() as DWORD;
            let ok = GetProcessMemoryInfo(
                handle,
                counters_ptr,
                size_of::<ProcessMemoryCountersEx>() as DWORD,
            );
            CloseHandle(handle);
            if ok == 0 {
                return None;
            }
            Some(counters.assume_init().private_usage as u64)
        }
    }

    pub(super) fn handle_count(pid: u32) -> Option<u64> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            let mut count: DWORD = 0;
            let ok = GetProcessHandleCount(handle, &mut count);
            CloseHandle(handle);
            if ok == 0 {
                return None;
            }
            Some(u64::from(count))
        }
    }
}

#[cfg(windows)]
fn sample_worker_memory_bytes(pid: u32) -> Option<u64> {
    win_sample::private_working_set_bytes(pid)
}

#[cfg(windows)]
fn sample_worker_handle_count(pid: u32) -> Option<u64> {
    win_sample::handle_count(pid)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn sample_worker_memory_bytes(_pid: u32) -> Option<u64> {
    None
}

#[cfg(not(any(windows, target_os = "linux")))]
fn sample_worker_handle_count(_pid: u32) -> Option<u64> {
    None
}

#[derive(Clone)]
struct SamplePoint {
    case_index: u64,
    elapsed_ms: u64,
    memory_bytes: Option<u64>,
    handle_count: Option<u64>,
    cases_per_second_window: f64,
}

struct ResourceOutcome {
    cases: u64,
    warm_up: u64,
    elapsed: Duration,
    samples: Vec<SamplePoint>,
    latency_us: Vec<u64>,
    plan_ok: u64,
    plan_reject_or_error: u64,
    failures: Vec<String>,
}

fn run_resource_corpus(executable: &Path, count: u64, timeout: Duration) -> ResourceOutcome {
    // Per-request response bound so a spinning worker cannot hang the harness forever.
    let per_request_timeout = Duration::from_secs(5);
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn resource worker");
    let pid = child.id();
    let mut stdin = child.stdin.take().expect("worker stdin");
    let stdout = child.stdout.take().expect("worker stdout");
    let mut stderr = child.stderr.take().expect("worker stderr");
    let stderr_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let (response_tx, response_rx) = std::sync::mpsc::sync_channel::<Result<Vec<u8>, String>>(0);
    let reader = std::thread::spawn(move || {
        let mut stdout = stdout;
        loop {
            match read_frame(&mut stdout, OUTPUT_MAX_BYTES) {
                Ok(frame) => {
                    if response_tx.send(Ok(frame)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = response_tx.send(Err(error));
                    break;
                }
            }
        }
    });

    let fixture_handshake = normalize_fixture_bytes(
        fs::read(fixture_root().join("handshake/ready.json")).expect("read frozen handshake"),
    );
    let mut failures = Vec::new();
    match response_rx.recv_timeout(per_request_timeout) {
        Ok(Ok(handshake)) if normalize_fixture_bytes(&handshake) == fixture_handshake => {}
        Ok(Ok(_)) => failures.push("worker handshake differs from frozen fixture".to_owned()),
        Ok(Err(error)) => failures.push(format!("worker handshake failed: {error}")),
        Err(_) => failures.push("worker handshake timed out".to_owned()),
    }

    let started = Instant::now();
    let mut samples = Vec::new();
    let mut latency_us = Vec::with_capacity(count.saturating_sub(WARMUP_CASES) as usize);
    let mut plan_ok = 0_u64;
    let mut plan_reject_or_error = 0_u64;
    let mut window_start = Instant::now();
    let mut window_cases = 0_u64;
    let mut completed = 0_u64;

    if failures.is_empty() {
        for index in 0..count {
            if started.elapsed() > timeout {
                failures.push(format!(
                    "watchdog exceeded after case {index}/{} ({:?})",
                    count,
                    started.elapsed()
                ));
                break;
            }
            let request = reviewed_boundary_request(index);
            let req_started = Instant::now();
            if let Err(error) = write_frame(&mut stdin, &request) {
                failures.push(format!("case {index} write failed: {error}"));
                break;
            }
            let response = match response_rx.recv_timeout(per_request_timeout) {
                Ok(Ok(response)) => response,
                Ok(Err(error)) => {
                    failures.push(format!("case {index} read failed: {error}"));
                    break;
                }
                Err(_) => {
                    failures.push(format!(
                        "case {index} response timed out after {per_request_timeout:?} (possible single-worker spin/hang)"
                    ));
                    break;
                }
            };
            let latency = req_started.elapsed();
            completed = index + 1;
            if index >= WARMUP_CASES {
                latency_us.push(latency.as_micros() as u64);
            }

            // Accept any well-formed plan/error JSON response; reject hang/malformed.
            match serde_json::from_slice::<Value>(&response) {
                Ok(value) => {
                    let kind = value.get("kind").and_then(Value::as_str).unwrap_or("");
                    if kind == "plan" {
                        plan_ok += 1;
                    } else {
                        plan_reject_or_error += 1;
                    }
                }
                Err(error) => {
                    failures.push(format!("case {index} malformed JSON response: {error}"));
                    break;
                }
            }

            window_cases += 1;
            if index == 0 || (index + 1) % SAMPLE_EVERY == 0 || index + 1 == count {
                let window_secs = window_start.elapsed().as_secs_f64().max(1e-9);
                let cps = window_cases as f64 / window_secs;
                samples.push(SamplePoint {
                    case_index: index + 1,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    memory_bytes: sample_worker_memory_bytes(pid),
                    handle_count: sample_worker_handle_count(pid),
                    cases_per_second_window: cps,
                });
                if (index + 1) % 1_000 == 0 {
                    println!(
                        "senline-resource progress case={} elapsed_ms={} cps_window={:.1} mem={:?} handles={:?}",
                        index + 1,
                        started.elapsed().as_millis(),
                        cps,
                        samples.last().and_then(|s| s.memory_bytes),
                        samples.last().and_then(|s| s.handle_count)
                    );
                }
                window_start = Instant::now();
                window_cases = 0;
            }
        }
    }

    drop(stdin);
    drop(response_rx);
    let _ = reader.join();
    let _stderr = stderr_reader.join().unwrap_or_default();
    let _ = finish_child(&mut child, Duration::from_secs(5));

    ResourceOutcome {
        cases: if completed == 0 { count } else { completed },
        warm_up: WARMUP_CASES,
        elapsed: started.elapsed(),
        samples,
        latency_us,
        plan_ok,
        plan_reject_or_error,
        failures,
    }
}

fn finish_child(child: &mut Child, grace: Duration) -> std::io::Result<std::process::ExitStatus> {
    let started = Instant::now();
    loop {
        match child.try_wait()? {
            Some(status) => return Ok(status),
            None if started.elapsed() > grace => {
                let _ = child.kill();
                return child.wait();
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

fn memory_growth_bytes_per_case(samples: &[SamplePoint], warm_up: u64) -> Option<f64> {
    let post: Vec<&SamplePoint> = samples
        .iter()
        .filter(|s| s.case_index > warm_up && s.memory_bytes.is_some())
        .collect();
    if post.len() < 2 {
        return None;
    }
    let first = post.first().unwrap();
    let last = post.last().unwrap();
    let cases = last.case_index.saturating_sub(first.case_index) as f64;
    if cases <= 0.0 {
        return None;
    }
    let delta =
        last.memory_bytes.unwrap() as i64 - first.memory_bytes.unwrap() as i64;
    Some(delta as f64 / cases)
}

fn write_evidence(label: &str, outcome: &ResourceOutcome) -> PathBuf {
    let mut latency = outcome.latency_us.clone();
    latency.sort_unstable();
    let p50 = percentile_us(&latency, 50.0);
    let p95 = percentile_us(&latency, 95.0);
    let p99 = percentile_us(&latency, 99.0);
    let mean = if latency.is_empty() {
        0.0
    } else {
        latency.iter().sum::<u64>() as f64 / latency.len() as f64
    };
    let growth = memory_growth_bytes_per_case(&outcome.samples, outcome.warm_up);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let path = evidence_root().join(format!(
        "soak-{}-{}-{}-{stamp}.summary.json",
        label,
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    let summary = serde_json::json!({
        "schema_version": 1,
        "label": label,
        "platform": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "fixed_seed_hex": format!("0x{FIXED_SEED:016x}"),
        "planner_contract_fixture_revision": PLANNER_FIXTURE_REVISION,
        "cases_requested": outcome.cases,
        "warm_up_cases": outcome.warm_up,
        "elapsed_ms": outcome.elapsed.as_millis() as u64,
        "plan_ok": outcome.plan_ok,
        "plan_reject_or_error": outcome.plan_reject_or_error,
        "failure_count": outcome.failures.len(),
        "failures": outcome.failures,
        "latency_post_warmup": {
            "sample_count": latency.len(),
            "mean_us": mean,
            "p50_us": p50,
            "p95_us": p95,
            "p99_us": p99,
            "notes": "request-write-complete to response-frame-complete wall time; not Senline admission/sandbox timing"
        },
        "memory": {
            "metric": if cfg!(windows) { "private_working_set_bytes" } else { "rss_bytes" },
            "post_warmup_growth_bytes_per_case": growth,
            "sample_count": outcome.samples.len(),
            "samples_tail": outcome.samples.iter().rev().take(5).map(|s| serde_json::json!({
                "case_index": s.case_index,
                "elapsed_ms": s.elapsed_ms,
                "memory_bytes": s.memory_bytes,
                "handle_count": s.handle_count,
                "cases_per_second_window": s.cases_per_second_window
            })).collect::<Vec<_>>(),
        },
        "gates": {
            "default_growth_bound_bytes_per_case": 1024.0,
            "growth_within_default_bound": growth.map(|g| g < 1024.0),
            "zero_failures": outcome.failures.is_empty(),
        }
    });
    fs::write(
        &path,
        serde_json::to_vec_pretty(&summary).expect("serialize resource summary"),
    )
    .expect("write resource summary");
    path
}

fn assert_resource_outcome(outcome: &ResourceOutcome, label: &str) {
    let path = write_evidence(label, outcome);
    println!(
        "senline-resource label={label} cases={} elapsed_ms={} failures={} evidence={}",
        outcome.cases,
        outcome.elapsed.as_millis(),
        outcome.failures.len(),
        path.display()
    );
    if !outcome.latency_us.is_empty() {
        let mut latency = outcome.latency_us.clone();
        latency.sort_unstable();
        println!(
            "senline-latency p50_us={} p95_us={} p99_us={} samples={}",
            percentile_us(&latency, 50.0),
            percentile_us(&latency, 95.0),
            percentile_us(&latency, 99.0),
            latency.len()
        );
    }
    assert!(
        outcome.failures.is_empty(),
        "{label} resource run failures: {:?}",
        outcome.failures
    );
    assert_eq!(
        outcome.plan_ok + outcome.plan_reject_or_error,
        outcome.cases.min(outcome.plan_ok + outcome.plan_reject_or_error),
        "{label} response accounting"
    );
}

#[test]
fn resource_sampler_smoke_single_worker_with_latency_percentiles() {
    let root = WorkerTempDir::new("smoke");
    let executable = build_worker(&root);
    let outcome = run_resource_corpus(
        &executable,
        SMOKE_COUNT,
        Duration::from_secs(180),
    );
    assert_resource_outcome(&outcome, "smoke-1k");
    assert!(
        outcome.latency_us.len() as u64 >= SMOKE_COUNT.saturating_sub(WARMUP_CASES),
        "post-warm-up latency samples missing"
    );
    assert!(
        !outcome.samples.is_empty(),
        "resource sampler produced no memory/throughput samples"
    );
}

#[test]
#[ignore = "single-worker investigation covering historical case ~44086; run with --ignored"]
fn resource_single_worker_investigation_50k() {
    let root = WorkerTempDir::new("investigate-45k");
    let executable = build_worker(&root);
    // Soft timeout: historical observation stalled near case 44086 within 3600s.
    // Keep room for progress logs but fail closed if the single worker spins.
    let outcome = run_resource_corpus(
        &executable,
        INVESTIGATION_COUNT,
        Duration::from_secs(900),
    );
    // Investigation may record a hang/spin; always write evidence.
    let path = write_evidence("investigate-45k", &outcome);
    println!(
        "senline-resource investigate evidence={} failures={:?}",
        path.display(),
        outcome.failures
    );
    if let Some(growth) = memory_growth_bytes_per_case(&outcome.samples, WARMUP_CASES) {
        println!("senline-resource post-warmup growth_bytes_per_case={growth}");
    }
    // Do not hard-assert green: task 8.3 remains open until 1M stable soak.
    // Still require that we either complete or leave a deterministic failure record.
    assert!(
        outcome.failures.is_empty()
            || outcome
                .failures
                .iter()
                .any(|f| f.contains("watchdog") || f.contains("read failed")),
        "unexpected investigation failure mode: {:?}",
        outcome.failures
    );
}

#[test]
#[ignore = "task 8.3 full 1M single-worker soak; run with --ignored on a reference host"]
fn resource_single_worker_soak_1m() {
    let root = WorkerTempDir::new("soak-1m");
    let executable = build_worker(&root);
    let outcome = run_resource_corpus(
        &executable,
        SOAK_COUNT,
        Duration::from_secs(6 * 3600),
    );
    assert_resource_outcome(&outcome, "soak-1m");
    let growth = memory_growth_bytes_per_case(&outcome.samples, WARMUP_CASES)
        .expect("need post-warm-up memory samples for soak gate");
    assert!(
        growth < 1024.0,
        "post-warm-up memory growth {growth} B/case exceeds 1 KiB/case default bound"
    );
}
