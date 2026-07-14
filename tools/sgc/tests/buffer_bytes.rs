mod common;

use common::source_sgc_command;
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
        "sengoo-buffer-bytes-{tag}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create Buffer byte test directory");
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
fn native_runtime_buffer_byte_access_checks_every_boundary() {
    let clang = which::which("clang")
        .or_else(|_| which::which("clang.exe"))
        .expect("native Buffer byte runtime probe requires clang");
    let root = temp_dir("native");
    let source = root.join("probe.c");
    let executable = root.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    fs::write(
        &source,
        r#"
#include "runtime_shared.h"
#include <limits.h>

long long sengoo_ffi_buffer_new(long long capacity);
long long sengoo_ffi_buffer_used_len(long long buffer_handle);
long long sengoo_ffi_buffer_get_u8(long long buffer_handle, long long index);
long long sengoo_ffi_buffer_set_u8(long long buffer_handle, long long index, long long value);
long long sengoo_ffi_buffer_free(long long buffer_handle);

int main(void) {
    long long handle = sengoo_ffi_buffer_new(2);
    if (handle <= 0) return 1;

    if (sengoo_ffi_buffer_set_u8(handle, 0, 0) != 1) return 2;
    if (sengoo_ffi_buffer_set_u8(handle, 1, 255) != 1) return 3;
    if (sengoo_ffi_buffer_get_u8(handle, 0) != 0) return 4;
    if (sengoo_ffi_buffer_get_u8(handle, 1) != 255) return 5;

    if (sengoo_ffi_buffer_get_u8(handle, -1) != -SENGOO_STATUS_INVALID_ARGUMENT) return 6;
    if (sengoo_ffi_buffer_set_u8(handle, -1, 1) != -SENGOO_STATUS_INVALID_ARGUMENT) return 7;
    if (sengoo_ffi_buffer_get_u8(handle, 2) != -SENGOO_STATUS_INVALID_ARGUMENT) return 8;
    if (sengoo_ffi_buffer_set_u8(handle, 2, 1) != -SENGOO_STATUS_INVALID_ARGUMENT) return 9;

    if (sengoo_ffi_buffer_get_u8(0, 0) != -SENGOO_STATUS_INVALID_HANDLE) return 10;
    if (sengoo_ffi_buffer_set_u8(0, 0, 1) != -SENGOO_STATUS_INVALID_HANDLE) return 11;
    if (sengoo_ffi_buffer_set_u8(handle, 0, -1) != -SENGOO_STATUS_INVALID_ARGUMENT) return 12;
    if (sengoo_ffi_buffer_set_u8(handle, 0, 256) != -SENGOO_STATUS_INVALID_ARGUMENT) return 13;

    if (sengoo_ffi_buffer_get_u8(handle, LLONG_MAX) != -SENGOO_STATUS_INVALID_ARGUMENT) return 14;
    if (sengoo_ffi_buffer_set_u8(handle, LLONG_MAX, 1) != -SENGOO_STATUS_INVALID_ARGUMENT) return 15;
    if (sengoo_ffi_buffer_set_u8(handle, 0, LLONG_MAX) != -SENGOO_STATUS_INVALID_ARGUMENT) return 16;

    if (sengoo_ffi_buffer_used_len(handle) != 2) return 17;
    if (sengoo_ffi_buffer_get_u8(handle, 0) != 0) return 18;
    if (sengoo_ffi_buffer_get_u8(handle, 1) != 255) return 19;
    if (sengoo_ffi_buffer_free(handle) != 0) return 20;
    if (sengoo_ffi_buffer_get_u8(handle, 0) != -SENGOO_STATUS_INVALID_HANDLE) return 21;
    if (sengoo_ffi_buffer_set_u8(handle, 0, 1) != -SENGOO_STATUS_INVALID_HANDLE) return 22;
    return 0;
}
"#,
    )
    .expect("write Buffer byte runtime probe");

    let stdlib = stdlib_dir();
    let mut compile = Command::new(clang);
    compile
        .arg("-std=c11")
        .arg("-I")
        .arg(&stdlib)
        .arg(&source)
        .arg(stdlib.join("runtime.c"))
        .arg(stdlib.join("runtime_string.c"))
        .arg("-o")
        .arg(&executable);
    if cfg!(windows) {
        compile.args(["-lws2_32", "-ladvapi32", "-lbcrypt"]);
    } else {
        compile.args(["-pthread", "-ldl", "-lm"]);
    }
    let compiled = compile
        .output()
        .expect("clang should compile Buffer byte runtime probe");
    assert_success("Buffer byte runtime probe compilation", &compiled);

    let output = Command::new(&executable)
        .output()
        .expect("Buffer byte runtime probe should run");
    assert_success("Buffer byte runtime probe", &output);

    assert!(root.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn real_sgc_buffer_byte_access_checks_every_boundary() {
    let root = temp_dir("real-sgc");
    let source = root.join("main.sg");
    fs::write(
        &source,
        r#"
import std::ffi;
import std::status;

def main() -> i64 {
    let buffer = ffi_buffer_new(2).unwrap_or(Buffer { handle: 0 });
    let first_set = buffer.set_u8(0, 0);
    let last_set = buffer.set_u8(1, 255);
    let first_get = buffer.get_u8(0);
    let last_get = buffer.get_u8(1);

    let negative_get = buffer.get_u8(-1);
    let negative_set = buffer.set_u8(-1, 1);
    let out_of_range_get = buffer.get_u8(2);
    let out_of_range_set = buffer.set_u8(2, 1);

    let invalid = Buffer { handle: 0 };
    let invalid_get = invalid.get_u8(0);
    let invalid_set = invalid.set_u8(0, 1);
    let negative_byte = buffer.set_u8(0, -1);
    let high_byte = buffer.set_u8(0, 256);

    let overflow_get = buffer.get_u8(9223372036854775807);
    let overflow_set = buffer.set_u8(9223372036854775807, 1);
    let overflow_byte = buffer.set_u8(0, 9223372036854775807);

    let unchanged = buffer.used_len() == 2
        && buffer.get_u8(0).unwrap_or(-1) == 0
        && buffer.get_u8(1).unwrap_or(-1) == 255;
    let ok = first_set.unwrap_or(false)
        && last_set.unwrap_or(false)
        && first_get.unwrap_or(-1) == 0
        && last_get.unwrap_or(-1) == 255
        && negative_get.is_err() && negative_get.error == STATUS_INVALID_ARGUMENT()
        && negative_set.is_err() && negative_set.error == STATUS_INVALID_ARGUMENT()
        && out_of_range_get.is_err() && out_of_range_get.error == STATUS_INVALID_ARGUMENT()
        && out_of_range_set.is_err() && out_of_range_set.error == STATUS_INVALID_ARGUMENT()
        && invalid_get.is_err() && invalid_get.error == STATUS_INVALID_HANDLE()
        && invalid_set.is_err() && invalid_set.error == STATUS_INVALID_HANDLE()
        && negative_byte.is_err() && negative_byte.error == STATUS_INVALID_ARGUMENT()
        && high_byte.is_err() && high_byte.error == STATUS_INVALID_ARGUMENT()
        && overflow_get.is_err() && overflow_get.error == STATUS_INVALID_ARGUMENT()
        && overflow_set.is_err() && overflow_set.error == STATUS_INVALID_ARGUMENT()
        && overflow_byte.is_err() && overflow_byte.error == STATUS_INVALID_ARGUMENT()
        && unchanged;
    buffer.free();
    if ok { 0 } else { 1 }
}
"#,
    )
    .expect("write real-sgc Buffer byte program");

    let output = source_sgc_command()
        .arg("run")
        .arg(&source)
        .arg("--force-rebuild")
        .output()
        .expect("real sgc should run Buffer byte program");
    assert_success("real-sgc Buffer byte program", &output);

    assert!(root.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn native_runtime_buffer_u32_be_is_network_order_and_failures_are_atomic() {
    let clang = which::which("clang")
        .or_else(|_| which::which("clang.exe"))
        .expect("native Buffer u32 runtime probe requires clang");
    let root = temp_dir("native-u32");
    let source = root.join("probe.c");
    let executable = root.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    fs::write(
        &source,
        r#"
#include "runtime_shared.h"
#include <limits.h>
#include <stdint.h>

long long sengoo_ffi_buffer_new(long long capacity);
long long sengoo_ffi_buffer_used_len(long long buffer_handle);
long long sengoo_ffi_buffer_get_u8(long long buffer_handle, long long index);
long long sengoo_ffi_buffer_set_u8(long long buffer_handle, long long index, long long value);
long long sengoo_ffi_buffer_read_u32_be(long long buffer_handle, long long offset);
long long sengoo_ffi_buffer_write_u32_be(long long buffer_handle, long long offset, long long value);
long long sengoo_ffi_buffer_free(long long buffer_handle);

static int bytes_match(long long handle) {
    static const long long expected[] = {165, 165, 1, 2, 3, 4, 165, 165};
    for (long long index = 0; index < 8; ++index) {
        if (sengoo_ffi_buffer_get_u8(handle, index) != expected[index]) return 0;
    }
    return 1;
}

int main(void) {
    long long handle = sengoo_ffi_buffer_new(8);
    if (handle <= 0) return 1;
    for (long long index = 0; index < 8; ++index) {
        if (sengoo_ffi_buffer_set_u8(handle, index, 165) != 1) return 2;
    }

    if (sengoo_ffi_buffer_write_u32_be(handle, 2, 0x01020304LL) != 1) return 3;
    if (sengoo_ffi_buffer_read_u32_be(handle, 2) != 0x01020304LL) return 4;
    if (!bytes_match(handle)) return 5;
    if (sengoo_ffi_buffer_used_len(handle) != 8) return 6;

    if (sengoo_ffi_buffer_read_u32_be(handle, 5) != -SENGOO_STATUS_INVALID_ARGUMENT) return 7;
    if (sengoo_ffi_buffer_write_u32_be(handle, 5, 0) != -SENGOO_STATUS_INVALID_ARGUMENT) return 8;
    if (sengoo_ffi_buffer_read_u32_be(handle, -1) != -SENGOO_STATUS_INVALID_ARGUMENT) return 9;
    if (sengoo_ffi_buffer_write_u32_be(handle, -1, 0) != -SENGOO_STATUS_INVALID_ARGUMENT) return 10;
    if (sengoo_ffi_buffer_read_u32_be(handle, LLONG_MAX) != -SENGOO_STATUS_OVERFLOW) return 11;
    if (sengoo_ffi_buffer_write_u32_be(handle, LLONG_MAX, 0) != -SENGOO_STATUS_OVERFLOW) return 12;
    if (sengoo_ffi_buffer_write_u32_be(handle, 0, -1) != -SENGOO_STATUS_INVALID_ARGUMENT) return 13;
    if (sengoo_ffi_buffer_write_u32_be(handle, 0, (long long)UINT32_MAX + 1) != -SENGOO_STATUS_OVERFLOW) return 14;

    if (sengoo_ffi_buffer_used_len(handle) != 8) return 15;
    if (!bytes_match(handle)) return 16;
    if (sengoo_ffi_buffer_free(handle) != 0) return 17;
    return 0;
}
"#,
    )
    .expect("write Buffer u32 runtime probe");

    let stdlib = stdlib_dir();
    let mut compile = Command::new(clang);
    compile
        .arg("-std=c11")
        .arg("-I")
        .arg(&stdlib)
        .arg(&source)
        .arg(stdlib.join("runtime.c"))
        .arg(stdlib.join("runtime_string.c"))
        .arg("-o")
        .arg(&executable);
    if cfg!(windows) {
        compile.args(["-lws2_32", "-ladvapi32", "-lbcrypt"]);
    } else {
        compile.args(["-pthread", "-ldl", "-lm"]);
    }
    let compiled = compile
        .output()
        .expect("clang should compile Buffer u32 runtime probe");
    assert_success("Buffer u32 runtime probe compilation", &compiled);

    let output = Command::new(&executable)
        .output()
        .expect("Buffer u32 runtime probe should run");
    assert_success("Buffer u32 runtime probe", &output);

    assert!(root.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn real_sgc_buffer_u32_be_is_network_order_and_failures_are_atomic() {
    let root = temp_dir("real-sgc-u32");
    let source = root.join("main.sg");
    fs::write(
        &source,
        r#"
import std::ffi;
import std::status;

def main() -> i64 {
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let initialized = buffer.set_u8(0, 165).unwrap_or(false)
        && buffer.set_u8(1, 165).unwrap_or(false)
        && buffer.set_u8(2, 165).unwrap_or(false)
        && buffer.set_u8(3, 165).unwrap_or(false)
        && buffer.set_u8(4, 165).unwrap_or(false)
        && buffer.set_u8(5, 165).unwrap_or(false)
        && buffer.set_u8(6, 165).unwrap_or(false)
        && buffer.set_u8(7, 165).unwrap_or(false);

    let wrote = buffer.write_u32_be(2, 16909060).unwrap_or(false);
    let network_order = buffer.get_u8(2).unwrap_or(-1) == 1
        && buffer.get_u8(3).unwrap_or(-1) == 2
        && buffer.get_u8(4).unwrap_or(-1) == 3
        && buffer.get_u8(5).unwrap_or(-1) == 4;
    let round_trip = buffer.read_u32_be(2).unwrap_or(-1) == 16909060;

    let short_read = buffer.read_u32_be(5);
    let short_write = buffer.write_u32_be(5, 0);
    let negative_read = buffer.read_u32_be(-1);
    let negative_write = buffer.write_u32_be(-1, 0);
    let overflow_read = buffer.read_u32_be(9223372036854775807);
    let overflow_write = buffer.write_u32_be(9223372036854775807, 0);
    let negative_value = buffer.write_u32_be(0, -1);
    let overflow_value = buffer.write_u32_be(0, 4294967296);

    let statuses = short_read.is_err() && short_read.error == STATUS_INVALID_ARGUMENT()
        && short_write.is_err() && short_write.error == STATUS_INVALID_ARGUMENT()
        && negative_read.is_err() && negative_read.error == STATUS_INVALID_ARGUMENT()
        && negative_write.is_err() && negative_write.error == STATUS_INVALID_ARGUMENT()
        && overflow_read.is_err() && overflow_read.error == STATUS_OVERFLOW()
        && overflow_write.is_err() && overflow_write.error == STATUS_OVERFLOW()
        && negative_value.is_err() && negative_value.error == STATUS_INVALID_ARGUMENT()
        && overflow_value.is_err() && overflow_value.error == STATUS_OVERFLOW();
    let unchanged = buffer.used_len() == 8
        && buffer.get_u8(0).unwrap_or(-1) == 165
        && buffer.get_u8(1).unwrap_or(-1) == 165
        && buffer.get_u8(2).unwrap_or(-1) == 1
        && buffer.get_u8(3).unwrap_or(-1) == 2
        && buffer.get_u8(4).unwrap_or(-1) == 3
        && buffer.get_u8(5).unwrap_or(-1) == 4
        && buffer.get_u8(6).unwrap_or(-1) == 165
        && buffer.get_u8(7).unwrap_or(-1) == 165;

    let ok = initialized && wrote && network_order && round_trip && statuses && unchanged;
    buffer.free();
    if ok { 0 } else { 1 }
}
"#,
    )
    .expect("write real-sgc Buffer u32 program");

    let output = source_sgc_command()
        .arg("run")
        .arg(&source)
        .arg("--force-rebuild")
        .output()
        .expect("real sgc should run Buffer u32 program");
    assert_success("real-sgc Buffer u32 program", &output);

    assert!(root.starts_with(std::env::temp_dir()));
    let _ = fs::remove_dir_all(root);
}
