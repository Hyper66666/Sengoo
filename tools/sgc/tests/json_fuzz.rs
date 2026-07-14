mod common;

use common::source_sgc_command;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const EXPECT_ANY: u32 = u32::MAX;
const EXPECT_ACCEPT: u32 = u32::MAX - 1;
const MAX_MUTATIONS: usize = 10_000;
const MAX_FIXED_CASES: usize = 1_024;
const MAX_BATCH_CASES: usize = 20_000;
const MAX_BATCH_BYTES: usize = 32 * 1024 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

struct ProbeTempDir {
    path: PathBuf,
}

impl ProbeTempDir {
    fn new(tag: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sengoo-json-fuzz-{tag}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create JSON fuzz probe directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ProbeTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone)]
struct FuzzCase {
    name: String,
    expected: u32,
    bytes: Vec<u8>,
}

struct CorpusConfig {
    strict_max_bytes: usize,
    mutation_max_bytes: usize,
    mutation_count: usize,
    seed: u64,
    malformed: Vec<FuzzCase>,
}

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("json-malformed-v1.json")
}

fn required_u64(value: &Value, field: &str) -> u64 {
    value[field]
        .as_u64()
        .unwrap_or_else(|| panic!("JSON fuzz corpus field `{field}` must be a u64"))
}

fn required_usize(value: &Value, field: &str) -> usize {
    usize::try_from(required_u64(value, field))
        .unwrap_or_else(|_| panic!("JSON fuzz corpus field `{field}` does not fit usize"))
}

fn decode_hex(name: &str, text: &str) -> Vec<u8> {
    assert!(
        text.len().is_multiple_of(2),
        "corpus case `{name}` has odd-length hex"
    );
    assert!(
        text.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "corpus case `{name}` hex must be lowercase"
    );
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("hex pair should be ASCII");
            u8::from_str_radix(pair, 16)
                .unwrap_or_else(|_| panic!("corpus case `{name}` has invalid hex `{pair}`"))
        })
        .collect()
}

fn error_kind(name: &str) -> u32 {
    match name {
        "unclassified" => 1,
        "duplicate_field" => 2,
        "invalid_unicode" => 3,
        "trailing_bytes" => 4,
        other => panic!("unsupported strict JSON error kind `{other}`"),
    }
}

fn load_corpus() -> CorpusConfig {
    let path = corpus_path();
    let bytes = fs::read(&path)
        .unwrap_or_else(|error| panic!("read fixed JSON fuzz corpus {}: {error}", path.display()));
    let root: Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse JSON fuzz corpus {}: {error}", path.display()));
    assert_eq!(required_u64(&root, "schema_version"), 1);

    let strict_max_bytes = required_usize(&root, "strict_max_bytes");
    let mutation_max_bytes = required_usize(&root, "mutation_max_bytes");
    let mutation_count = required_usize(&root, "mutation_count");
    assert_eq!(strict_max_bytes, 1024 * 1024);
    assert!((1..=strict_max_bytes).contains(&mutation_max_bytes));
    assert!((1024..=MAX_MUTATIONS).contains(&mutation_count));

    let default_origin = root["default_origin"]
        .as_str()
        .expect("JSON fuzz corpus default_origin must be a string");
    assert_eq!(default_origin, "spec_seed");
    let retention_policy = root["retention_policy"]
        .as_str()
        .expect("JSON fuzz corpus retention_policy must be a string");
    assert!(retention_policy.contains("minimized bytes"));

    let seed_text = root["seed"]
        .as_str()
        .expect("JSON fuzz corpus seed must be a hexadecimal string");
    let seed = u64::from_str_radix(seed_text.trim_start_matches("0x"), 16)
        .expect("JSON fuzz corpus seed must fit u64");
    assert_ne!(seed, 0);

    let cases = root["cases"]
        .as_array()
        .expect("JSON fuzz corpus cases must be an array");
    assert!(
        (16..=MAX_FIXED_CASES).contains(&cases.len()),
        "malformed corpus size is outside the reviewed bounds"
    );
    let mut names = HashSet::new();
    let malformed = cases
        .iter()
        .map(|case| {
            let name = case["name"]
                .as_str()
                .expect("corpus case name must be a string");
            assert!(names.insert(name), "duplicate corpus case name `{name}`");
            let origin = case["origin"].as_str().unwrap_or(default_origin);
            assert!(matches!(origin, "spec_seed" | "fixed_crash"));
            if origin == "fixed_crash" {
                let regression = case["regression"]
                    .as_str()
                    .expect("fixed_crash corpus cases require a regression id");
                assert!(!regression.is_empty());
            }
            let hex = case["hex"]
                .as_str()
                .expect("corpus case hex must be a string");
            let bytes = decode_hex(name, hex);
            assert!(bytes.len() <= strict_max_bytes);
            FuzzCase {
                name: name.to_owned(),
                expected: error_kind(
                    case["error_kind"]
                        .as_str()
                        .expect("corpus error_kind must be a string"),
                ),
                bytes,
            }
        })
        .collect();

    CorpusConfig {
        strict_max_bytes,
        mutation_max_bytes,
        mutation_count,
        seed,
        malformed,
    }
}

fn next_random(state: &mut u64) -> u64 {
    let mut value = *state;
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    *state = value;
    value
}

fn random_index(state: &mut u64, upper_exclusive: usize) -> usize {
    if upper_exclusive == 0 {
        0
    } else {
        (next_random(state) as usize) % upper_exclusive
    }
}

fn mutate(pool: &[Vec<u8>], state: &mut u64, max_len: usize) -> Vec<u8> {
    let mut bytes = pool[random_index(state, pool.len())].clone();
    bytes.truncate(max_len);
    let structural = b"{}[],:\\\"0123456789tfnueE+- \n\r\t";
    let steps = 1 + random_index(state, 8);
    for _ in 0..steps {
        match random_index(state, 8) {
            0 => {
                if bytes.is_empty() {
                    bytes.push(next_random(state) as u8);
                } else {
                    let index = random_index(state, bytes.len());
                    bytes[index] ^= 1 << random_index(state, 8);
                }
            }
            1 => {
                bytes.truncate(random_index(state, bytes.len() + 1));
            }
            2 => {
                if bytes.len() < max_len {
                    let index = random_index(state, bytes.len() + 1);
                    let value = if next_random(state) & 1 == 0 {
                        structural[random_index(state, structural.len())]
                    } else {
                        next_random(state) as u8
                    };
                    bytes.insert(index, value);
                }
            }
            3 => {
                if !bytes.is_empty() {
                    let index = random_index(state, bytes.len());
                    bytes.remove(index);
                }
            }
            4 => {
                if bytes.is_empty() {
                    bytes.push(structural[random_index(state, structural.len())]);
                } else {
                    let index = random_index(state, bytes.len());
                    bytes[index] = structural[random_index(state, structural.len())];
                }
            }
            5 => {
                if !bytes.is_empty() && bytes.len() < max_len {
                    let start = random_index(state, bytes.len());
                    let available = bytes.len() - start;
                    let take = 1 + random_index(state, available.min(32));
                    let fragment = bytes[start..start + take].to_vec();
                    let insert_at = random_index(state, bytes.len() + 1);
                    let remaining = max_len - bytes.len();
                    bytes.splice(insert_at..insert_at, fragment.into_iter().take(remaining));
                }
            }
            6 => {
                let other = &pool[random_index(state, pool.len())];
                let split = random_index(state, bytes.len() + 1);
                bytes.truncate(split);
                bytes.extend(other.iter().copied().take(max_len - bytes.len()));
            }
            _ => {
                if bytes.len() < max_len {
                    let padding = [b' ', b'\n', b'\r', b'\t', 0, 0xff];
                    let count = 1 + random_index(state, 8);
                    for _ in 0..count {
                        if bytes.len() == max_len {
                            break;
                        }
                        bytes.push(padding[random_index(state, padding.len())]);
                    }
                }
            }
        }
        bytes.truncate(max_len);
    }
    bytes
}

fn build_cases(config: &CorpusConfig) -> Vec<FuzzCase> {
    let mut cases = config.malformed.clone();
    cases.push(FuzzCase {
        name: "over-hardening-limit".to_owned(),
        expected: 1,
        bytes: vec![b' '; config.strict_max_bytes + 1],
    });

    let valid_seeds = vec![
        b"null".to_vec(),
        b"true".to_vec(),
        b"-9223372036854775808".to_vec(),
        br#"{"a":[1,false,null,"text"],"b":"\u4f60\u597d","c":"\ud83d\ude00"}"#.to_vec(),
        vec![b'[', b']'],
    ];
    for (index, bytes) in valid_seeds.iter().enumerate() {
        cases.push(FuzzCase {
            name: format!("valid-seed-{index}"),
            expected: EXPECT_ACCEPT,
            bytes: bytes.clone(),
        });
    }
    let mut hardening_boundary = Vec::with_capacity(config.strict_max_bytes);
    hardening_boundary.push(b'"');
    hardening_boundary.resize(config.strict_max_bytes - 1, b'a');
    hardening_boundary.push(b'"');
    cases.push(FuzzCase {
        name: "valid-at-hardening-limit".to_owned(),
        expected: EXPECT_ACCEPT,
        bytes: hardening_boundary,
    });

    let mut depth_63 = vec![b'['; 63];
    depth_63.extend_from_slice(b"null");
    depth_63.extend(std::iter::repeat_n(b']', 63));
    cases.push(FuzzCase {
        name: "valid-depth-63".to_owned(),
        expected: EXPECT_ACCEPT,
        bytes: depth_63,
    });
    let mut depth_64 = vec![b'['; 64];
    depth_64.extend_from_slice(b"null");
    depth_64.extend(std::iter::repeat_n(b']', 64));
    cases.push(FuzzCase {
        name: "reject-depth-64".to_owned(),
        expected: 1,
        bytes: depth_64,
    });

    let node_boundary = |items: usize| {
        let mut bytes = Vec::with_capacity(2 + items.saturating_mul(5));
        bytes.push(b'[');
        for index in 0..items {
            if index > 0 {
                bytes.push(b',');
            }
            bytes.extend_from_slice(b"null");
        }
        bytes.push(b']');
        bytes
    };
    cases.push(FuzzCase {
        name: "valid-4096-nodes".to_owned(),
        expected: EXPECT_ACCEPT,
        bytes: node_boundary(4095),
    });
    cases.push(FuzzCase {
        name: "reject-4097-nodes".to_owned(),
        expected: 1,
        bytes: node_boundary(4096),
    });

    let mut mutation_pool = valid_seeds;
    mutation_pool.extend(config.malformed.iter().map(|case| case.bytes.clone()));
    let mut state = config.seed;
    for index in 0..config.mutation_count {
        cases.push(FuzzCase {
            name: format!("mutation-{index:05}"),
            expected: EXPECT_ANY,
            bytes: mutate(&mutation_pool, &mut state, config.mutation_max_bytes),
        });
    }
    cases
}

fn encode_u32(output: &mut Vec<u8>, value: usize) {
    output.extend(
        u32::try_from(value)
            .expect("JSON fuzz batch value should fit u32")
            .to_be_bytes(),
    );
}

fn encode_batch(cases: &[FuzzCase]) -> Vec<u8> {
    assert!(
        !cases.is_empty() && cases.len() <= MAX_BATCH_CASES,
        "JSON fuzz batch case count is outside the reviewed bounds"
    );
    let encoded_len = cases.iter().fold(4usize, |total, case| {
        total
            .checked_add(8)
            .and_then(|value| value.checked_add(case.bytes.len()))
            .expect("JSON fuzz batch byte count overflowed")
    });
    assert!(
        encoded_len <= MAX_BATCH_BYTES,
        "JSON fuzz batch exceeds the reviewed byte budget"
    );
    let mut batch = Vec::with_capacity(encoded_len);
    encode_u32(&mut batch, cases.len());
    for case in cases {
        batch.extend(case.expected.to_be_bytes());
        encode_u32(&mut batch, case.bytes.len());
        batch.extend(&case.bytes);
    }
    assert_eq!(batch.len(), encoded_len);
    batch
}

fn stdlib_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("stdlib")
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

fn failure_case_context(output: &Output, cases: &[FuzzCase]) -> String {
    if let Ok(stderr) = std::str::from_utf8(&output.stderr) {
        for line in stderr.lines() {
            if let Some(index) = line
                .strip_prefix("case ")
                .and_then(|rest| rest.split(':').next())
                .and_then(|value| value.parse::<usize>().ok())
            {
                return cases.get(index).map_or_else(
                    || format!("reported out-of-range case index {index}"),
                    |case| format!("case {index} `{}`", case.name),
                );
            }
        }
    }
    if output.stderr.len() == 8 {
        let index = u32::from_be_bytes(output.stderr[0..4].try_into().unwrap()) as usize;
        let reason = u32::from_be_bytes(output.stderr[4..8].try_into().unwrap());
        return cases.get(index).map_or_else(
            || format!("reported out-of-range case index {index}, reason {reason}"),
            |case| format!("case {index} `{}`, reason {reason}", case.name),
        );
    }
    "probe did not report a case index".to_owned()
}

fn assert_probe_success(label: &str, output: &Output, cases: &[FuzzCase]) {
    assert!(
        output.status.success(),
        "{label} failed with {:?}: {}\nstdout:\n{}\nstderr (lossy):\n{}",
        output.status.code(),
        failure_case_context(output, cases),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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

fn wait_with_deadline(
    child: &mut Child,
    label: &str,
    timeout: Duration,
) -> Result<ExitStatus, String> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(error) => {
                let cleanup = terminate_and_reap(child);
                return Err(format!("{label} wait failed: {error}; {cleanup}"));
            }
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            let cleanup = terminate_and_reap(child);
            return Err(format!(
                "{label} timed out after {} ms; {cleanup}",
                timeout.as_millis()
            ));
        }
        thread::sleep(Duration::from_millis(10).min(timeout - elapsed));
    }
}

fn run_probe(executable: &Path, batch_path: &Path, label: &str) -> Output {
    let mut child = Command::new(executable)
        .stdin(Stdio::from(
            File::open(batch_path).expect("open JSON fuzz batch for probe stdin"),
        ))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn {label}: {error}"));
    let status = wait_with_deadline(&mut child, label, PROBE_TIMEOUT)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .stdout
        .take()
        .expect("probe stdout should be piped")
        .read_to_end(&mut stdout)
        .expect("read probe stdout");
    child
        .stderr
        .take()
        .expect("probe stderr should be piped")
        .read_to_end(&mut stderr)
        .expect("read probe stderr");
    Output {
        status,
        stdout,
        stderr,
    }
}

fn native_probe_source() -> &'static str {
    r#"
#include "runtime_shared.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

long long sengoo_json_parse_text_strict(long long data, long long len);
long long sengoo_json_doc_close(long long handle);
long long sengoo_json_doc_live_handle_count(void);
long long sengoo_json_last_error_code(void);
long long sengoo_json_last_error_kind(void);
long long sengoo_io_protocol_binary_mode(void);

char* sengoo_copy_cstr_from_handle(long long value_ptr) {
    const char* value = (const char*)(intptr_t)value_ptr;
    if (!value) return NULL;
    size_t len = strlen(value);
    char* copy = (char*)malloc(len + 1);
    if (!copy) return NULL;
    memcpy(copy, value, len + 1);
    return copy;
}

#define EXPECT_ANY UINT32_MAX
#define EXPECT_ACCEPT (UINT32_MAX - 1U)
#define MAX_BATCH_CASE_BYTES (1024U * 1024U + 1U)

static int read_exact(unsigned char* target, size_t len) {
    size_t offset = 0;
    while (offset < len) {
        size_t count = fread(target + offset, 1, len - offset, stdin);
        if (count == 0) return 0;
        offset += count;
    }
    return 1;
}

static int read_u32(uint32_t* value) {
    unsigned char bytes[4];
    if (!read_exact(bytes, sizeof(bytes))) return 0;
    *value = ((uint32_t)bytes[0] << 24)
        | ((uint32_t)bytes[1] << 16)
        | ((uint32_t)bytes[2] << 8)
        | (uint32_t)bytes[3];
    return 1;
}

int main(void) {
    if (sengoo_io_protocol_binary_mode() != 0) return 1;
    uint32_t total = 0;
    if (!read_u32(&total) || total == 0 || total > 100000U) return 2;
    long long baseline = sengoo_json_doc_live_handle_count();
    for (uint32_t index = 0; index < total; ++index) {
        uint32_t expected = 0;
        uint32_t len = 0;
        if (!read_u32(&expected) || !read_u32(&len) || len > MAX_BATCH_CASE_BYTES) return 3;
        unsigned char* bytes = len == 0 ? NULL : (unsigned char*)malloc(len);
        if (len > 0 && (!bytes || !read_exact(bytes, len))) {
            free(bytes);
            return 4;
        }

        long long handle = sengoo_json_parse_text_strict(
            (long long)(intptr_t)bytes,
            (long long)len);
        if (expected == EXPECT_ACCEPT && handle == 0) {
            fprintf(stderr, "case %u: expected accept, code=%lld kind=%lld\n",
                index, sengoo_json_last_error_code(), sengoo_json_last_error_kind());
            free(bytes);
            return 5;
        }
        if (expected != EXPECT_ANY && expected != EXPECT_ACCEPT) {
            if (handle != 0 || sengoo_json_last_error_code() == 0
                || sengoo_json_last_error_kind() != (long long)expected) {
                fprintf(stderr, "case %u: expected reject kind=%u, handle=%lld code=%lld kind=%lld\n",
                    index, expected, handle, sengoo_json_last_error_code(), sengoo_json_last_error_kind());
                if (handle != 0) sengoo_json_doc_close(handle);
                free(bytes);
                return 6;
            }
        } else if (expected == EXPECT_ANY && handle == 0
            && sengoo_json_last_error_code() == 0) {
            fprintf(stderr, "case %u: rejection had no error code\n", index);
            free(bytes);
            return 7;
        }
        if (handle != 0 && sengoo_json_doc_close(handle) != 0) {
            free(bytes);
            return 8;
        }
        free(bytes);
        if (sengoo_json_doc_live_handle_count() != baseline) {
            fprintf(stderr, "case %u: live handle count grew\n", index);
            return 9;
        }
    }
    return 0;
}
"#
}

fn sengoo_probe_source() -> &'static str {
    r#"
import std::ffi;
import std::io;
import std::json;

extern "C" {
    fn sengoo_json_doc_live_handle_count() -> i64;
}

def fuzz_fail(index: i64, code: i64) -> i64 {
    let diagnostic = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    if diagnostic.handle != 0 {
        let wrote_index = diagnostic.write_u32_be(0, index);
        let wrote_code = diagnostic.write_u32_be(4, code);
        if wrote_index.is_ok and wrote_code.is_ok {
            let ignored_write = io_stderr_write_raw(diagnostic.ptr(), 8);
            let ignored_flush = io_stderr_flush();
        };
        let ignored_free = diagnostic.free();
    };
    code;
}

def main() -> i64 {
    let binary = io_protocol_binary_mode();
    if binary.is_err() { return 1; };
    let total_buffer = ffi_buffer_new(4).unwrap_or(Buffer { handle: 0 });
    if total_buffer.handle == 0 { return 2; };
    let total_read = io_stdin_read_exact(total_buffer, 0, 4);
    if total_read.is_err() or total_read.value != 4 { total_buffer.free(); return 3; };
    let total_result = total_buffer.read_u32_be(0);
    total_buffer.free();
    if total_result.is_err() or total_result.value <= 0 or total_result.value > 100000 { return 4; };

    let baseline = sengoo_json_doc_live_handle_count();
    let mut index = 0;
    while index < total_result.value {
        let meta = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
        if meta.handle == 0 { return fuzz_fail(index, 5); };
        let meta_read = io_stdin_read_exact(meta, 0, 8);
        if meta_read.is_err() or meta_read.value != 8 { meta.free(); return fuzz_fail(index, 6); };
        let expected_result = meta.read_u32_be(0);
        let len_result = meta.read_u32_be(4);
        meta.free();
        if expected_result.is_err() or len_result.is_err() or len_result.value > 1048577 { return fuzz_fail(index, 7); };

        let allocation_len = if len_result.value == 0 { 1 } else { len_result.value };
        let payload = ffi_buffer_new(allocation_len).unwrap_or(Buffer { handle: 0 });
        if payload.handle == 0 { return fuzz_fail(index, 8); };
        if len_result.value > 0 {
            let payload_read = io_stdin_read_exact(payload, 0, len_result.value);
            if payload_read.is_err() or payload_read.value != len_result.value { payload.free(); return fuzz_fail(index, 9); };
        };

        let parsed = json_parse_buffer_strict(payload, len_result.value);
        let expected = expected_result.value;
        if expected == 4294967294 {
            if parsed.is_err() { payload.free(); return fuzz_fail(index, 10); };
        } else if expected != 4294967295 {
            if parsed.is_ok { parsed.value.close(); payload.free(); return fuzz_fail(index, 11); };
            if json_last_error_code() == 0 or json_last_error_kind() != expected { payload.free(); return fuzz_fail(index, 12); };
        } else if parsed.is_err() and json_last_error_code() == 0 {
            payload.free();
            return fuzz_fail(index, 13);
        };
        if parsed.is_ok and not parsed.value.close() { payload.free(); return fuzz_fail(index, 14); };
        payload.free();
        if sengoo_json_doc_live_handle_count() != baseline { return fuzz_fail(index, 15); };
        index = index + 1;
    };
    0;
}
"#
}

#[test]
fn native_runtime_strict_json_survives_fixed_corpus_and_seeded_mutations() {
    let config = load_corpus();
    let cases = build_cases(&config);
    let root = ProbeTempDir::new("native");
    let batch_path = root.path().join("batch.bin");
    fs::write(&batch_path, encode_batch(&cases)).expect("write native JSON fuzz batch");
    let source = root.path().join("probe.c");
    fs::write(&source, native_probe_source()).expect("write native JSON fuzz probe");
    let executable = root
        .path()
        .join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let clang = which::which("clang")
        .or_else(|_| which::which("clang.exe"))
        .expect("native JSON fuzz probe requires clang");
    let stdlib = stdlib_dir();
    let mut compile = Command::new(clang);
    compile
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg("-I")
        .arg(&stdlib)
        .arg(&source)
        .arg(stdlib.join("runtime.c"))
        .arg(stdlib.join("runtime_string.c"))
        .arg(stdlib.join("runtime_json.c"))
        .arg("-o")
        .arg(&executable);
    if cfg!(windows) {
        compile.args(["-Wno-unknown-pragmas", "-lws2_32", "-ladvapi32", "-lbcrypt"]);
    } else {
        compile.args(["-pthread", "-ldl", "-lm"]);
    }
    let compiled = compile
        .output()
        .expect("clang should compile native JSON fuzz probe");
    assert_success("native JSON fuzz probe compilation", &compiled);

    let output = run_probe(&executable, &batch_path, "native JSON fuzz probe");
    assert_probe_success("native JSON fuzz probe", &output, &cases);
}

#[test]
fn real_sgc_strict_json_wrapper_survives_the_same_bounded_batch() {
    let config = load_corpus();
    let cases = build_cases(&config);
    let root = ProbeTempDir::new("real-sgc");
    let batch_path = root.path().join("batch.bin");
    fs::write(&batch_path, encode_batch(&cases)).expect("write real-sgc JSON fuzz batch");
    let source = root.path().join("main.sg");
    fs::write(&source, sengoo_probe_source()).expect("write real-sgc JSON fuzz source");
    let executable = root.path().join(if cfg!(windows) {
        "json-fuzz-wrapper.exe"
    } else {
        "json-fuzz-wrapper"
    });
    let built = source_sgc_command()
        .arg("build")
        .arg(&source)
        .arg("--force-rebuild")
        .arg("--output")
        .arg(&executable)
        .output()
        .expect("real sgc should build JSON fuzz wrapper");
    assert_success("real-sgc JSON fuzz wrapper build", &built);

    let output = run_probe(&executable, &batch_path, "real-sgc JSON fuzz wrapper");
    assert_probe_success("real-sgc JSON fuzz wrapper", &output, &cases);
}
