use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

struct NativeProbeTempDir {
    path: PathBuf,
}

impl NativeProbeTempDir {
    fn new(tag: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sengoo-binary-io-{tag}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create binary I/O test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for NativeProbeTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
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

fn wait_for_probe_with_deadline(
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

fn wait_for_probe_success(child: &mut Child, label: &str) {
    let status = wait_for_probe_with_deadline(child, label, PROBE_TIMEOUT)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(status.success(), "{label} failed with {status}");
}

fn compile_native_probe(source_text: &str, tag: &str) -> (NativeProbeTempDir, PathBuf) {
    let clang = which::which("clang")
        .or_else(|_| which::which("clang.exe"))
        .expect("native write-all runtime probe requires clang");
    let root = NativeProbeTempDir::new(tag);
    let source = root.path().join("probe.c");
    let executable = root
        .path()
        .join(if cfg!(windows) { "probe.exe" } else { "probe" });
    fs::write(&source, source_text).expect("write native write-all runtime probe");

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
        .expect("clang should compile write-all runtime probe");
    assert_success("write-all runtime probe compilation", &compiled);
    (root, executable)
}

fn assert_native_probe_temp_directory_guard_cleans_up_on_drop() {
    let path;
    {
        let root = NativeProbeTempDir::new("drop-cleanup");
        path = root.path().to_path_buf();
        assert!(path.is_dir());
    }
    assert!(!path.exists(), "native probe temporary directory leaked");
}

fn assert_native_probe_watchdog_kills_and_reaps_a_hung_child() {
    let (root, executable) = compile_native_probe(
        r#"
int main(void) {
    for (;;) {}
}
"#,
        "watchdog-hang",
    );
    let mut child = Command::new(&executable)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn intentional watchdog hang probe");

    let error = wait_for_probe_with_deadline(
        &mut child,
        "intentional watchdog hang probe",
        Duration::from_millis(100),
    )
    .expect_err("watchdog must reject an intentionally hung child");
    assert!(
        error.contains("timed out"),
        "unexpected watchdog error: {error}"
    );
    assert!(
        child
            .try_wait()
            .expect("query reaped watchdog child")
            .is_some(),
        "watchdog killed the child without reaping it"
    );

    assert!(root.path().starts_with(std::env::temp_dir()));
}

#[test]
fn native_write_all_advances_offsets_and_rejects_every_short_or_failed_write() {
    let (root, executable) = compile_native_probe(
        r#"
#include "runtime_shared.h"
#include <limits.h>
#include <stddef.h>
#include <stdint.h>

typedef struct {
    size_t count;
    int error;
} ScriptStep;

typedef struct {
    const unsigned char* expected_source;
    size_t expected_len;
    size_t expected_offset;
    const ScriptStep* steps;
    size_t step_count;
    size_t step_index;
    int invalid_script;
} ScriptedWriter;

static SengooRuntimeWriteResult scripted_write(
    void* context,
    const unsigned char* source,
    size_t capacity) {
    ScriptedWriter* writer = (ScriptedWriter*)context;
    SengooRuntimeWriteResult result = {0, 0};
    if (writer->step_index >= writer->step_count) {
        writer->invalid_script = 1;
        return result;
    }
    if (source != writer->expected_source + writer->expected_offset
        || capacity != writer->expected_len - writer->expected_offset) {
        writer->invalid_script = 1;
        result.error = 1;
        return result;
    }

    ScriptStep step = writer->steps[writer->step_index++];
    if (step.count <= capacity) {
        writer->expected_offset += step.count;
    }
    result.count = step.count;
    result.error = step.error;
    return result;
}

static void fill_pattern(unsigned char* bytes, size_t len) {
    for (size_t index = 0; index < len; ++index) {
        bytes[index] = (unsigned char)((index * 37U + 11U) & 0xffU);
    }
}

static int expect_success(
    const unsigned char* source,
    size_t len,
    const size_t* chunks,
    size_t chunk_count) {
    ScriptStep steps[64];
    if (chunk_count > 64 || len > 64) return 0;
    for (size_t index = 0; index < chunk_count; ++index) {
        steps[index].count = chunks[index];
        steps[index].error = 0;
    }
    ScriptedWriter writer = {
        source, len, 0, steps, chunk_count, 0, 0
    };

    long long status = sengoo_runtime_write_all(
        scripted_write, &writer, source, len);
    return status == (long long)len
        && writer.invalid_script == 0
        && writer.step_index == chunk_count
        && writer.expected_offset == len;
}

static int expect_failure(
    const unsigned char* source,
    size_t len,
    const ScriptStep* steps,
    size_t step_count) {
    ScriptedWriter writer = {
        source, len, 0, steps, step_count, 0, 0
    };

    long long status = sengoo_runtime_write_all(
        scripted_write, &writer, source, len);
    return status == -SENGOO_STATUS_IO
        && writer.invalid_script == 0
        && writer.step_index == step_count
        && status != (long long)len;
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

static int test_zero_progress_and_native_errors(void) {
    unsigned char source[4] = {1, 2, 3, 4};
    ScriptStep zero_before[] = {{0, 0}};
    ScriptStep zero_after[] = {{2, 0}, {0, 0}};
    ScriptStep error_before[] = {{0, 1}};
    ScriptStep error_after[] = {{2, 0}, {0, 1}};
    ScriptStep progress_and_error[] = {{2, 1}};
    ScriptStep excessive_count[] = {{5, 0}};

    return expect_failure(source, 4, zero_before, 1)
        && expect_failure(source, 4, zero_after, 2)
        && expect_failure(source, 4, error_before, 1)
        && expect_failure(source, 4, error_after, 2)
        && expect_failure(source, 4, progress_and_error, 1)
        && expect_failure(source, 4, excessive_count, 1);
}

static int test_argument_boundaries(void) {
    unsigned char source = 0x5a;
    ScriptStep unexpected[] = {{1, 0}};
    ScriptedWriter zero_writer = {
        NULL, 0, 0, unexpected, 1, 0, 0
    };
    if (sengoo_runtime_write_all(
            scripted_write, &zero_writer, NULL, 0) != 0
        || zero_writer.step_index != 0) {
        return 0;
    }

    ScriptedWriter null_callback_writer = {
        &source, 1, 0, unexpected, 1, 0, 0
    };
    if (sengoo_runtime_write_all(
            NULL, &null_callback_writer, &source, 1) != -SENGOO_STATUS_INVALID_ARGUMENT
        || null_callback_writer.step_index != 0) {
        return 0;
    }

    ScriptedWriter null_source_writer = {
        &source, 1, 0, unexpected, 1, 0, 0
    };
    if (sengoo_runtime_write_all(
            scripted_write, &null_source_writer, NULL, 1)
            != -SENGOO_STATUS_INVALID_ARGUMENT
        || null_source_writer.step_index != 0) {
        return 0;
    }

#if SIZE_MAX > 9223372036854775807ULL
    size_t overflow_len = (size_t)LLONG_MAX + (size_t)1;
    ScriptedWriter overflow_writer = {
        &source, overflow_len, 0, unexpected, 1, 0, 0
    };
    if (sengoo_runtime_write_all(
            scripted_write, &overflow_writer, &source, overflow_len)
            != -SENGOO_STATUS_OVERFLOW
        || overflow_writer.step_index != 0) {
        return 0;
    }
#endif

    return 1;
}

int main(void) {
    if (!test_prefix_compositions()) return 1;
    if (!test_payload_splits()) return 2;
    if (!test_zero_progress_and_native_errors()) return 3;
    if (!test_argument_boundaries()) return 4;
    return 0;
}
"#,
        "write-all-scripted",
    );

    let mut child = Command::new(&executable)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn scripted write-all runtime probe");
    wait_for_probe_success(&mut child, "scripted write-all runtime probe");

    assert!(root.path().starts_with(std::env::temp_dir()));
}

fn run_closed_stdout_case(executable: &PathBuf, mode: &str) {
    let mut child = Command::new(executable)
        .arg(mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn closed-stdout write-all probe");

    let stdout_reader = match child.stdout.take() {
        Some(reader) => reader,
        None => {
            let cleanup = terminate_and_reap(&mut child);
            panic!("{mode}: child stdout pipe was unavailable; {cleanup}");
        }
    };
    drop(stdout_reader);

    let mut stdin_gate = match child.stdin.take() {
        Some(gate) => gate,
        None => {
            let cleanup = terminate_and_reap(&mut child);
            panic!("{mode}: child stdin gate was unavailable; {cleanup}");
        }
    };
    let gate_result = stdin_gate.write_all(b"G");
    drop(stdin_gate);
    if let Err(error) = gate_result {
        let cleanup = terminate_and_reap(&mut child);
        panic!("{mode}: failed to release child through stdin gate: {error}; {cleanup}");
    }

    wait_for_probe_success(&mut child, mode);
}

#[test]
fn native_closed_pipe_distinguishes_write_failure_from_flush_failure() {
    assert_native_probe_temp_directory_guard_cleans_up_on_drop();
    assert_native_probe_watchdog_kills_and_reaps_a_hung_child();

    let (root, executable) = compile_native_probe(
        r#"
#include "runtime_shared.h"
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#ifndef _WIN32
#include <signal.h>
#endif

long long sengoo_ffi_buffer_from_bytes(long long data_ptr, long long len);
long long sengoo_ffi_buffer_free(long long buffer_handle);
long long sengoo_io_stdout_write_all(long long buffer_handle, long long offset, long long len);
long long sengoo_io_stdout_flush(void);

static int wait_for_parent_gate(void) {
    return fgetc(stdin) == 'G';
}

int main(int argc, char** argv) {
    static const unsigned char payload[3] = {0x00, 0x7f, 0xff};
    static char stdout_buffer[4096];
    if (argc != 2 || !wait_for_parent_gate()) return 1;

#ifndef _WIN32
    if (signal(SIGPIPE, SIG_IGN) == SIG_ERR) return 2;
#endif

    long long buffer = sengoo_ffi_buffer_from_bytes(
        (long long)(intptr_t)payload,
        (long long)sizeof(payload));
    if (buffer == 0) return 3;

    if (strcmp(argv[1], "unbuffered-write-failure") == 0) {
        if (setvbuf(stdout, NULL, _IONBF, 0) != 0) return 4;
        long long write_status = sengoo_io_stdout_write_all(
            buffer, 0, (long long)sizeof(payload));
        sengoo_ffi_buffer_free(buffer);
        return write_status == -SENGOO_STATUS_IO ? 0 : 5;
    }

    if (strcmp(argv[1], "buffered-flush-failure") == 0) {
        if (setvbuf(stdout, stdout_buffer, _IOFBF, sizeof(stdout_buffer)) != 0) return 6;
        long long write_status = sengoo_io_stdout_write_all(
            buffer, 0, (long long)sizeof(payload));
        long long flush_status = sengoo_io_stdout_flush();
        sengoo_ffi_buffer_free(buffer);
        return write_status == (long long)sizeof(payload)
            && flush_status == -SENGOO_STATUS_IO ? 0 : 7;
    }

    sengoo_ffi_buffer_free(buffer);
    return 8;
}
"#,
        "write-all-closed-pipe",
    );

    run_closed_stdout_case(&executable, "unbuffered-write-failure");
    run_closed_stdout_case(&executable, "buffered-flush-failure");

    assert!(root.path().starts_with(std::env::temp_dir()));
}
