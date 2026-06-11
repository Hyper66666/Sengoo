use crate::mir::Instruction;
use crate::{compile_to_ir, compile_to_mir};

fn async_stdlib_prefix() -> String {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_root = manifest_dir
        .parent()
        .unwrap_or(manifest_dir)
        .join("tools")
        .join("stdlib");
    let modules = ["option.sg", "result.sg", "ffi.sg", "status.sg", "async.sg"];
    modules
        .iter()
        .map(|module| {
            std::fs::read_to_string(stdlib_root.join(module))
                .unwrap_or_else(|err| panic!("failed to read stdlib module {module}: {err}"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn net_async_stdlib_prefix() -> String {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_root = manifest_dir
        .parent()
        .unwrap_or(manifest_dir)
        .join("tools")
        .join("stdlib");
    let modules = [
        "option.sg",
        "result.sg",
        "ffi.sg",
        "status.sg",
        "string.sg",
        "async.sg",
        "net.sg",
    ];
    modules
        .iter()
        .map(|module| {
            std::fs::read_to_string(stdlib_root.join(module))
                .unwrap_or_else(|err| panic!("failed to read stdlib module {module}: {err}"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn compile_with_async_stdlib(program: &str) -> String {
    let source = format!("{}\n\n{}", async_stdlib_prefix(), program);
    compile_to_ir(&source)
        .unwrap_or_else(|err| panic!("concurrent async program should compile: {err}"))
}

#[test]
fn concurrent_spawn_blocking_rejects_non_send_buffer_capture() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
struct JsonValue {
    handle: i64,
}

def touch(buf: JsonValue) -> i64 {
    buf.handle
}

async def main() -> i64 {
    let buf = JsonValue { handle: 1 };
    let spawned = spawn_blocking_i64(| | touch(buf));
    0
}
"#
    );

    let err = compile_to_ir(&source).expect_err("Buffer capture should fail Send check");
    let msg = err.to_string();
    assert!(
        msg.contains("not Send") || msg.contains("JsonValue"),
        "expected Send diagnostic, got: {msg}"
    );
}

#[test]
fn concurrent_spawn_blocking_rejects_async_context_capture() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
struct Poll<T> {
    is_ready: bool,
    value: T,
}

struct AsyncContext {
    handle: i64,
}

trait Future<T> {
    def poll(&mut self, ctx: AsyncContext) -> Poll<T> {
        Poll { is_ready: false, value: 0 }
    }
}

struct BadFuture {}

impl Future<i64> for BadFuture {
    def poll(&mut self, ctx: AsyncContext) -> Poll<i64> {
        let spawned = spawn_blocking_i64(| | ctx.handle);
        Poll { is_ready: false, value: 0 }
    }
}

async def main() -> i64 {
    await BadFuture {}
}
"#
    );

    let err = compile_to_ir(&source).expect_err("AsyncContext capture should fail Send checking");
    let msg = err.to_string();
    assert!(
        msg.contains("not Send") || msg.contains("AsyncContext"),
        "expected AsyncContext Send diagnostic, got: {msg}"
    );
}

#[test]
fn concurrent_spawn_blocking_i64_lowers_runtime_start_when_pool_enabled() {
    let source = r#"
def worker() -> i64 { 42 }
async def main() -> i64 {
    let enabled = runtime_enable_thread_pool(2);
    if !enabled.is_ok { return 0; }
    let fut = spawn_blocking_future_i64(| | worker());
    await fut
}
"#;

    let mir = compile_to_mir(&(async_stdlib_prefix() + "\n\n" + source))
        .expect("spawn_blocking program should lower");
    let has_start = mir.iter().any(|mir_fn| {
        mir_fn.instructions.iter().any(|inst| {
            matches!(
                inst,
                Instruction::Call { func, .. } if func == "sengoo_async_spawn_blocking_i64__start"
            )
        })
    });
    assert!(
        has_start,
        "spawn_blocking lowering should reach runtime start"
    );
    compile_with_async_stdlib(source);
}

#[test]
fn async_http_server_next_request_lowers_runtime_start() {
    let source = r#"
def request_handle(request: HttpServerRequest) -> i64 {
    request.handle
}

async def main() -> i64 {
    let bound = http_server_bind("127.0.0.1", 0);
    if !bound.is_ok { return bound.error; }
    let outcome = await bound.value.next_request_async(1);
    if outcome.is_ok && outcome.value.handle == request_handle(outcome.value) { 0 } else { outcome.error }
}
"#;

    let mir = compile_to_mir(&(net_async_stdlib_prefix() + "\n\n" + source))
        .expect("async HTTP server source should lower");
    let has_start = mir.iter().any(|mir_fn| {
        mir_fn.instructions.iter().any(|inst| {
            matches!(
                inst,
                Instruction::Call { func, .. }
                    if func == "sengoo_http_server_next_request_async__start"
            )
        })
    });
    assert!(
        has_start,
        "HttpServer.next_request_async lowering should call runtime start"
    );

    let ir = compile_to_ir(&(net_async_stdlib_prefix() + "\n\n" + source))
        .expect("async HTTP server source should reach LLVM IR");
    assert!(
        ir.contains("declare i64 @sengoo_http_server_next_request_async__start(i64, i64)"),
        "IR should declare async HTTP start, got:\n{ir}"
    );
    assert!(
        ir.contains("@sengoo_http_server_next_request_async__result"),
        "IR should call async HTTP result, got:\n{ir}"
    );
    assert!(
        ir.contains("%HttpServerRequest = type { i64 }")
            && ir.contains("%HttpServerNextRequestOutcome = type { i1, %HttpServerRequest, i64 }"),
        "async HTTP outcome should preserve the request wrapper aggregate, got:\n{ir}"
    );
}

#[test]
fn async_http_server_rejects_awaiting_synchronous_next_request() {
    let source = r#"
async def main() -> i64 {
    let bound = http_server_bind("127.0.0.1", 0);
    if !bound.is_ok { return bound.error; }
    let outcome = await bound.value.next_request(1);
    if outcome.is_ok { 0 } else { outcome.error }
}
"#;

    let err = compile_to_ir(&(net_async_stdlib_prefix() + "\n\n" + source))
        .expect_err("the synchronous next_request result must not be awaitable");
    let message = err.to_string();
    assert!(
        message.contains("await requires a Future value"),
        "expected a non-Future await diagnostic, got: {message}"
    );
}
