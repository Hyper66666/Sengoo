use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(tag: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "sengoo-binary-io-{tag}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create binary I/O test directory");
    root
}

fn stdlib_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("stdlib")
}

fn assert_success(label: &str, output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{label} failed with {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_exact_read_handles_partial_progress_and_every_failure_boundary_atomically() {
    let clang = which::which("clang")
        .or_else(|_| which::which("clang.exe"))
        .expect("native exact-read runtime probe requires clang");
    let root = temp_dir("exact-read");
    let source = root.join("probe.c");
    let executable = root.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    fs::write(
        &source,
        r#"
#include "runtime_shared.h"
#include <stddef.h>
#include <stdint.h>
#include <string.h>

typedef struct {
    size_t count;
    int eof;
    int error;
} ScriptStep;

typedef struct {
    const unsigned char* input;
    size_t input_len;
    size_t input_offset;
    const ScriptStep* steps;
    size_t step_count;
    size_t step_index;
    int invalid_script;
} ScriptedReader;

static SengooRuntimeReadResult scripted_read(
    void* context,
    unsigned char* destination,
    size_t capacity) {
    ScriptedReader* reader = (ScriptedReader*)context;
    SengooRuntimeReadResult result = {0, 0, 0};
    if (reader->step_index >= reader->step_count) {
        reader->invalid_script = 1;
        return result;
    }

    ScriptStep step = reader->steps[reader->step_index++];
    if (step.count > capacity || step.count > reader->input_len - reader->input_offset) {
        reader->invalid_script = 1;
        result.error = 1;
        return result;
    }
    if (step.count > 0) {
        memcpy(destination, reader->input + reader->input_offset, step.count);
        reader->input_offset += step.count;
    }
    result.count = step.count;
    result.eof = step.eof;
    result.error = step.error;
    return result;
}

static void fill_pattern(unsigned char* bytes, size_t len) {
    for (size_t index = 0; index < len; ++index) {
        bytes[index] = (unsigned char)((index * 37U + 11U) & 0xffU);
    }
}

static int bytes_are(const unsigned char* bytes, size_t len, unsigned char value) {
    for (size_t index = 0; index < len; ++index) {
        if (bytes[index] != value) return 0;
    }
    return 1;
}

static int expect_success(
    const unsigned char* input,
    size_t len,
    const size_t* chunks,
    size_t chunk_count) {
    ScriptStep steps[64];
    unsigned char destination[66];
    if (chunk_count > 64 || len > 64) return 0;
    for (size_t index = 0; index < chunk_count; ++index) {
        steps[index].count = chunks[index];
        steps[index].eof = 0;
        steps[index].error = 0;
    }
    memset(destination, 0xa5, sizeof(destination));
    ScriptedReader reader = {
        input, len, 0, steps, chunk_count, 0, 0
    };

    long long status = sengoo_runtime_read_exact(
        scripted_read, &reader, destination + 1, len);
    return status == (long long)len
        && reader.invalid_script == 0
        && reader.step_index == chunk_count
        && reader.input_offset == len
        && destination[0] == 0xa5
        && destination[len + 1] == 0xa5
        && memcmp(destination + 1, input, len) == 0;
}

static int expect_failure_unchanged(
    const unsigned char* input,
    size_t input_len,
    size_t expected_len,
    const ScriptStep* steps,
    size_t step_count,
    long long expected_status) {
    unsigned char destination[66];
    if (expected_len > 64) return 0;
    memset(destination, 0xa5, sizeof(destination));
    ScriptedReader reader = {
        input, input_len, 0, steps, step_count, 0, 0
    };

    long long status = sengoo_runtime_read_exact(
        scripted_read, &reader, destination + 1, expected_len);
    return status == expected_status
        && reader.invalid_script == 0
        && bytes_are(destination, sizeof(destination), 0xa5);
}

static int test_prefix_compositions(void) {
    static const unsigned char prefix[4] = {0x00, 0x00, 0x80, 0x00};
    static const size_t chunks[][4] = {
        {1, 3, 0, 0},
        {2, 2, 0, 0},
        {3, 1, 0, 0},
        {1, 1, 2, 0},
        {1, 2, 1, 0},
        {2, 1, 1, 0},
        {1, 1, 1, 1}
    };
    static const size_t chunk_counts[] = {2, 2, 2, 3, 3, 3, 4};
    for (size_t index = 0; index < 7; ++index) {
        if (!expect_success(prefix, 4, chunks[index], chunk_counts[index])) return 0;
    }
    return 1;
}

static int test_payload_splits(void) {
    unsigned char payload[64];
    fill_pattern(payload, sizeof(payload));

    size_t one[] = {1};
    if (!expect_success(payload, 1, one, 1)) return 0;

    for (size_t len_index = 0; len_index < 2; ++len_index) {
        size_t len = len_index == 0 ? 8 : 64;
        for (size_t split = 1; split < len; ++split) {
            size_t chunks[] = {split, len - split};
            if (!expect_success(payload, len, chunks, 2)) return 0;
        }
    }
    return 1;
}

static int test_clean_eof_and_truncation(void) {
    unsigned char input[64];
    fill_pattern(input, sizeof(input));

    ScriptStep clean_eof[] = {{0, 1, 0}};
    if (!expect_failure_unchanged(input, 0, 4, clean_eof, 1, 0)) return 0;

    for (size_t supplied = 1; supplied < 4; ++supplied) {
        ScriptStep steps[] = {{supplied, 0, 0}, {0, 1, 0}};
        if (!expect_failure_unchanged(
                input, supplied, 4, steps, 2, -SENGOO_STATUS_IO)) return 0;
    }

    for (size_t len_index = 0; len_index < 3; ++len_index) {
        size_t len = len_index == 0 ? 1 : (len_index == 1 ? 8 : 64);
        for (size_t supplied = 1; supplied < len; ++supplied) {
            ScriptStep steps[] = {{supplied, 0, 0}, {0, 1, 0}};
            if (!expect_failure_unchanged(
                    input, supplied, len, steps, 2, -SENGOO_STATUS_IO)) return 0;
        }
    }
    return 1;
}

static int test_zero_progress_and_native_errors(void) {
    unsigned char input[4] = {1, 2, 3, 4};
    ScriptStep zero_before[] = {{0, 0, 0}};
    ScriptStep zero_after[] = {{2, 0, 0}, {0, 0, 0}};
    ScriptStep error_before[] = {{0, 0, 1}};
    ScriptStep error_after[] = {{2, 0, 0}, {0, 0, 1}};
    ScriptStep progress_and_error[] = {{2, 0, 1}};

    return expect_failure_unchanged(input, 0, 4, zero_before, 1, -SENGOO_STATUS_IO)
        && expect_failure_unchanged(input, 2, 4, zero_after, 2, -SENGOO_STATUS_IO)
        && expect_failure_unchanged(input, 0, 4, error_before, 1, -SENGOO_STATUS_IO)
        && expect_failure_unchanged(input, 2, 4, error_after, 2, -SENGOO_STATUS_IO)
        && expect_failure_unchanged(input, 2, 4, progress_and_error, 1, -SENGOO_STATUS_IO);
}

int main(void) {
    if (!test_prefix_compositions()) return 1;
    if (!test_payload_splits()) return 2;
    if (!test_clean_eof_and_truncation()) return 3;
    if (!test_zero_progress_and_native_errors()) return 4;
    return 0;
}
"#,
    )
    .expect("write native exact-read runtime probe");

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
        .arg("-o")
        .arg(&executable);
    if cfg!(windows) {
        compile.args(["-Wno-unknown-pragmas", "-lws2_32", "-ladvapi32", "-lbcrypt"]);
    } else {
        compile.args(["-pthread", "-ldl", "-lm"]);
    }
    let compiled = compile
        .output()
        .expect("clang should compile exact-read runtime probe");
    assert_success("exact-read runtime probe compilation", &compiled);

    let output = Command::new(&executable)
        .output()
        .expect("exact-read runtime probe should run");
    assert_success("exact-read runtime probe", &output);

    assert!(root.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(root);
}
