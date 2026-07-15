//! Runtime hardening integration tests (async, FFI, handles, platform, security).

use super::{
    compile_and_run_stdlib_import_program_with_native_runtime,
    compile_and_run_stdlib_import_program_with_stdin, compile_source, ensure_runtime_objects,
    expand_stdlib_imports_for_source, find_clang, find_runtime_c, link_native_binary_from_objects,
    temp_artifact,
};
use std::fs;
use std::process::Command;

#[test]
fn runtime_hardening_buffer_double_close_is_idempotent() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "buffer-double-close",
        r#"
import std::ffi;
import std::status;

def main() -> i64 {
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let first = buffer.free();
    let second = buffer.free();
    if first && second && ffi_last_error_code() == 0 {
        0
    } else {
        1
    }
}
"#,
    ) else {
        return;
    };
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ffi_hardening_missing_symbol_source_compiles() {
    let source = expand_stdlib_imports_for_source(
        r#"
import std::ffi;

def main() -> i64 {
    let lib = ffi_open("self://builtin");
    if lib.is_err() {
        1
    } else {
        let loaded = lib.unwrap_or(CLib { handle: 0 });
        let value = loaded.call_i64_0("missing_symbol_xyz");
        loaded.close();
        if value.is_err() { 0 } else { 1 }
    }
}
"#,
    )
    .expect("ffi missing-symbol program should expand");
    compile_source(&source, 0).expect("ffi missing-symbol program should type-check");
}

#[test]
fn ffi_hardening_shell_metacharacters_passed_as_literal_argv() {
    let Some(clang) = find_clang() else {
        return;
    };
    let child_c = temp_artifact("hardening-shell-literal-child", "c");
    let child_exe = temp_artifact(
        "hardening-shell-literal-child",
        if cfg!(windows) { "exe" } else { "" },
    );
    fs::write(
        &child_c,
        r#"
int main(int argc, char** argv) {
    const char* expected = "|&;<>";
    int index = 0;
    if (argc != 2) {
        return 8;
    }
    while (expected[index] != '\0' && argv[1][index] == expected[index]) {
        index++;
    }
    return expected[index] == '\0' && argv[1][index] == '\0' ? 0 : 8;
}
"#,
    )
    .unwrap();
    let status = Command::new(&clang)
        .arg(&child_c)
        .arg("-o")
        .arg(&child_exe)
        .status()
        .expect("shell-literal child fixture should compile");
    assert!(status.success());

    let executable = child_exe.to_string_lossy().replace('\\', "/");
    let source = format!(
        r#"
import std::process;

def main() -> i64 {{
    let code = process_run_1("{executable}", "|&;<>").unwrap_or(-1);
    if code == 0 {{ 0 }} else {{ 1 }}
}}
"#
    );
    let Some(output) =
        compile_and_run_stdlib_import_program_with_stdin("process-shell-literal", &source, "")
    else {
        return;
    };
    let _ = fs::remove_file(&child_c);
    let _ = fs::remove_file(&child_exe);
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn runtime_hardening_json_rejects_oversized_input() {
    let oversized = "a".repeat(20_000);
    let source = format!(
        r#"
import std::json;
import std::status;

def main() -> i64 {{
    let doc = json_parse("{oversized}");
    if doc.is_ok {{
        doc.value.close();
        1
    }} else if doc.error == STATUS_PARSE() {{
        0
    }} else {{
        1
    }}
}}
"#
    );
    let Some(output) =
        compile_and_run_stdlib_import_program_with_stdin("json-oversized", &source, "")
    else {
        return;
    };
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn runtime_hardening_config_rejects_oversized_input() {
    let oversized = "x".repeat(70_000);
    let source = format!(
        r#"
import std::config;

def main() -> i64 {{
    let doc = ini_parse("{oversized}");
    if doc.is_ok {{
        doc.value.drop();
        1
    }} else {{
        0
    }}
}}
"#
    );
    let Some(output) =
        compile_and_run_stdlib_import_program_with_stdin("config-oversized", &source, "")
    else {
        return;
    };
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn runtime_hardening_http_invalid_scheme_maps_unsupported() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "http-invalid-scheme",
        r#"
import std::http;

def main() -> i64 {
    let result = http_client_get("ftp://127.0.0.1/", 1);
    if result.is_ok {
        result.value.close();
        1
    } else if result.error == STATUS_UNSUPPORTED() {
        0
    } else {
        result.error
    }
}
"#,
    ) else {
        return;
    };
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn runtime_hardening_c_bundle_http_server_request_symbols_map_fallback_statuses() {
    let Some(output) = compile_and_run_stdlib_import_program_with_stdin(
        "http-server-request-c-bundle",
        r#"
import std::net;
import std::status;

def main() -> i64 {
    let buffer = ffi_buffer_new(16).unwrap_or(Buffer { handle: 0 });
    let request = HttpServerRequest { handle: 7 };

    let bind_result = http_server_bind("127.0.0.1", 0);
    let bind_supported_or_explicitly_unsupported = if bind_result.is_ok {
        bind_result.value.close()
    } else {
        bind_result.error == STATUS_UNSUPPORTED()
    };

    let server = HttpServer { handle: 7 };
    let next_result = server.next_request(1);
    let next_rejected = !next_result.is_ok
        && (next_result.error == STATUS_INVALID_HANDLE() || next_result.error == STATUS_UNSUPPORTED());

    let method_len_result = request.method_len();
    let method_len_invalid = !method_len_result.is_ok && method_len_result.error == STATUS_INVALID_HANDLE();
    let method_copy_result = request.method_copy(buffer);
    let method_copy_invalid = !method_copy_result.is_ok && method_copy_result.error == STATUS_INVALID_HANDLE();

    let path_len_result = request.path_len();
    let path_len_invalid = !path_len_result.is_ok && path_len_result.error == STATUS_INVALID_HANDLE();
    let path_copy_result = request.path_copy(buffer);
    let path_copy_invalid = !path_copy_result.is_ok && path_copy_result.error == STATUS_INVALID_HANDLE();

    let query_len_result = request.query_len();
    let query_len_invalid = !query_len_result.is_ok && query_len_result.error == STATUS_INVALID_HANDLE();
    let query_copy_result = request.query_copy(buffer);
    let query_copy_invalid = !query_copy_result.is_ok && query_copy_result.error == STATUS_INVALID_HANDLE();

    let version_len_result = request.version_len();
    let version_len_invalid = !version_len_result.is_ok && version_len_result.error == STATUS_INVALID_HANDLE();
    let version_copy_result = request.version_copy(buffer);
    let version_copy_invalid = !version_copy_result.is_ok && version_copy_result.error == STATUS_INVALID_HANDLE();

    let header_len_result = request.header_len("x-test");
    let header_len_invalid = !header_len_result.is_ok && header_len_result.error == STATUS_INVALID_HANDLE();
    let header_copy_result = request.header_copy("x-test", buffer);
    let header_copy_invalid = !header_copy_result.is_ok && header_copy_result.error == STATUS_INVALID_HANDLE();

    let body_len_result = request.body_len();
    let body_len_invalid = !body_len_result.is_ok && body_len_result.error == STATUS_INVALID_HANDLE();
    let body_copy_result = request.body_copy(buffer);
    let body_copy_invalid = !body_copy_result.is_ok && body_copy_result.error == STATUS_INVALID_HANDLE();

    let respond_result = request.respond(200, "x");
    let respond_invalid = !respond_result.is_ok && respond_result.error == STATUS_INVALID_HANDLE();
    let typed_result = request.respond_with_content_type(200, "text/plain", "x");
    let typed_invalid = !typed_result.is_ok && typed_result.error == STATUS_INVALID_HANDLE();

    let close_idempotent = request.close();
    buffer.free();

    if !bind_supported_or_explicitly_unsupported { return 1; }
    if !next_rejected { return 2; }
    if !method_len_invalid || !method_copy_invalid { return 3; }
    if !path_len_invalid || !path_copy_invalid { return 4; }
    if !query_len_invalid || !query_copy_invalid { return 5; }
    if !version_len_invalid || !version_copy_invalid { return 6; }
    if !header_len_invalid || !header_copy_invalid { return 7; }
    if !body_len_invalid || !body_copy_invalid { return 8; }
    if !respond_invalid || !typed_invalid { return 9; }
    if !close_idempotent { return 10; }
    0
}
"#,
        "",
    ) else {
        return;
    };
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn runtime_hardening_c_bundle_http_server_async_result_reports_unsupported() {
    let Some(clang) = find_clang() else {
        return;
    };
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    let source_path = temp_artifact("http-server-async-fallback-probe", "c");
    fs::write(
        &source_path,
        r#"
#include <stdint.h>

typedef struct {
    long long handle;
} SengooHttpServerRequestHandle;

typedef struct {
    unsigned char is_ok;
    SengooHttpServerRequestHandle value;
    long long error;
} SengooHttpServerNextRequestResult;

#ifdef _WIN32
void __main(void) {}
#endif

long long sengoo_http_server_next_request_async__start(long long handle, long long timeout_ms);
long long sengoo_http_server_next_request_async__poll(long long handle);
SengooHttpServerNextRequestResult sengoo_http_server_next_request_async__result(long long handle);
unsigned char sengoo_http_server_next_request_async__cancel(long long handle);
void sengoo_http_server_next_request_async__drop(long long handle);

int main(void) {
    long long handle = sengoo_http_server_next_request_async__start(7, 1);
    if (handle == 0) {
        return 10;
    }
    if (sengoo_http_server_next_request_async__poll(handle) != 1) {
        return 11;
    }
    SengooHttpServerNextRequestResult result =
        sengoo_http_server_next_request_async__result(handle);
    if (result.is_ok != 0 || result.value.handle != 0 || result.error != 8) {
        return 12;
    }

    long long cancel_handle = sengoo_http_server_next_request_async__start(7, 1);
    if (cancel_handle == 0 || !sengoo_http_server_next_request_async__cancel(cancel_handle)) {
        return 13;
    }
    long long drop_handle = sengoo_http_server_next_request_async__start(7, 1);
    if (drop_handle == 0) {
        return 14;
    }
    sengoo_http_server_next_request_async__drop(drop_handle);
    return 0;
}
"#,
    )
    .expect("probe source should be writable");

    let obj_ext = if cfg!(windows) { "obj" } else { "o" };
    let probe_obj = temp_artifact("http-server-async-fallback-probe", obj_ext);
    let status = Command::new(&clang)
        .arg("-c")
        .arg(&source_path)
        .arg("-o")
        .arg(&probe_obj)
        .status()
        .expect("clang should compile fallback probe");
    assert!(status.success(), "fallback probe should compile");

    let exe_path = temp_artifact(
        "http-server-async-fallback-probe",
        if cfg!(windows) { "exe" } else { "" },
    );
    let mut object_paths = vec![probe_obj.clone()];
    object_paths.extend(ensure_runtime_objects(&clang, &runtime_c, 1, None).unwrap());
    link_native_binary_from_objects(&clang, &object_paths, &exe_path, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("fallback probe should run");
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_file(&source_path);
    let _ = fs::remove_file(&probe_obj);
    let _ = fs::remove_file(&exe_path);
}

#[test]
fn async_hardening_spawn_and_timeout_sources_compile() {
    for source in [
        r#"
async def child() -> i64 {
    await sleep(5);
    7
}

async def main() -> i64 {
    let task = spawn_task(child());
    let pending = task_status(task);
    await sleep(15);
    let done = task_status(task);
    if pending == 1 {
        if done == 2 { 42 } else { 0 }
    } else {
        0
    }
}
"#,
        r#"
async def child() -> i64 {
    await sleep(20);
    42
}

async def main() -> i64 {
    let fut = child();
    let ready = await timeout(fut, 5);
    if ready {
        await fut
    } else {
        await sleep(25);
        await fut
    }
}
"#,
    ] {
        compile_source(source, 1).expect("async hardening source should compile to LLVM IR");
    }
}

#[test]
fn runtime_hardening_sgc_check_accepts_async_main() {
    let source = expand_stdlib_imports_for_source(
        r#"
async def main() -> i64 {
    await sleep(1);
    41 + 1
}
"#,
    )
    .expect("async main should expand");
    compile_source(&source, 0).expect("sgc check path should accept async main");
}

#[test]
fn runtime_hardening_parser_decoder_bounded_fuzz_smoke() {
    let Some(clang) = find_clang() else {
        return;
    };
    let Some(runtime_c) = find_runtime_c() else {
        return;
    };

    let source_path = temp_artifact("runtime-parser-bounded-fuzz", "c");
    fs::write(
        &source_path,
        r#"
#include <stdint.h>

#ifdef _WIN32
void __main(void) {}
#endif

long long sengoo_json_parse_text(long long data, long long len);
long long sengoo_json_doc_close(long long handle);
long long sengoo_json_last_error_code(void);
long long sengoo_config_ini_parse(long long data, long long len);
long long sengoo_config_ini_free(long long handle);
long long sengoo_config_toml_parse(long long data, long long len);
long long sengoo_config_toml_free(long long handle);

int main(void) {
    unsigned char bytes[256];
    uint64_t state = UINT64_C(0x243f6a8885a308d3);
    for (int case_id = 0; case_id < 512; case_id++) {
        long long len = case_id % 256;
        for (long long index = 0; index < len; index++) {
            state = state * UINT64_C(6364136223846793005) + UINT64_C(1442695040888963407);
            bytes[index] = (unsigned char)(state >> 32);
        }

        long long json = sengoo_json_parse_text((long long)(intptr_t)bytes, len);
        if (json > 0 && sengoo_json_doc_close(json) != 0) {
            return 10;
        }
        long long ini = sengoo_config_ini_parse((long long)(intptr_t)bytes, len);
        if (ini > 0 && sengoo_config_ini_free(ini) < 0) {
            return 11;
        }
        long long toml = sengoo_config_toml_parse((long long)(intptr_t)bytes, len);
        if (toml > 0 && sengoo_config_toml_free(toml) < 0) {
            return 12;
        }
    }

    if (sengoo_json_parse_text(0, 1) != 0 || sengoo_json_last_error_code() == 0) return 20;
    if (sengoo_json_parse_text((long long)(intptr_t)bytes, -1) != 0 || sengoo_json_last_error_code() == 0) return 21;
    if (sengoo_config_ini_parse(0, 1) >= 0) return 22;
    if (sengoo_config_toml_parse((long long)(intptr_t)bytes, -1) >= 0) return 23;
    return 0;
}
"#,
    )
    .expect("bounded runtime parser probe should be writable");

    let obj_ext = if cfg!(windows) { "obj" } else { "o" };
    let probe_obj = temp_artifact("runtime-parser-bounded-fuzz", obj_ext);
    let status = Command::new(&clang)
        .arg("-c")
        .arg(&source_path)
        .arg("-o")
        .arg(&probe_obj)
        .status()
        .expect("clang should compile bounded runtime parser probe");
    assert!(
        status.success(),
        "bounded runtime parser probe should compile"
    );

    let exe_path = temp_artifact(
        "runtime-parser-bounded-fuzz",
        if cfg!(windows) { "exe" } else { "" },
    );
    let mut object_paths = vec![probe_obj.clone()];
    object_paths.extend(ensure_runtime_objects(&clang, &runtime_c, 1, None).unwrap());
    link_native_binary_from_objects(&clang, &object_paths, &exe_path, None, None).unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("bounded runtime parser probe should run");
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_file(&source_path);
    let _ = fs::remove_file(&probe_obj);
    let _ = fs::remove_file(&exe_path);
}
