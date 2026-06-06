//! Runtime hardening integration tests (async, FFI, handles, platform, security).

use super::{
    compile_and_run_stdlib_import_program_with_native_runtime,
    compile_and_run_stdlib_import_program_with_stdin, compile_source,
    expand_stdlib_imports_for_source, find_clang, temp_artifact,
};
use std::fs;
use std::process::Command;

#[test]
fn runtime_hardening_buffer_double_close_returns_invalid_handle() {
    let Some(output) = compile_and_run_stdlib_import_program_with_native_runtime(
        "buffer-double-close",
        r#"
import std::ffi;
import std::status;

def main() -> i64 {
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let first = buffer.free();
    let second = buffer.free();
    if first && second == false && ffi_status_from_raw(ffi_last_error_code()) == STATUS_INVALID_HANDLE() {
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
        "stdout:\n{}\nstderr:\n{}",
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
        "stdout:\n{}\nstderr:\n{}",
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
        "stdout:\n{}\nstderr:\n{}",
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
        "stdout:\n{}\nstderr:\n{}",
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
