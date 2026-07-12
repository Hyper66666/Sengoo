use crate::mir::Instruction;
use crate::{compile_to_ir, compile_to_mir, Parser, TypeChecker};

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

fn collections_async_stdlib_prefix() -> String {
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
        "collections.sg",
        "async.sg",
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
fn concurrent_send_sync_marker_bounds_are_importable() {
    let source = r#"
struct Token {
    value: i64,
}

struct AutoToken {
    value: i64,
    flag: bool,
}

impl Send for Token {}
impl Sync for Token {}

def require_send<T: Send>(value: T) -> i64 { 1 }
def require_sync<T: Sync>(value: T) -> i64 { 2 }

def main() -> i64 {
    let token = Token { value: 7 };
    require_send(token)
        + require_sync(Token { value: 8 })
        + require_send(AutoToken { value: 9, flag: true })
        + require_sync(AutoToken { value: 10, flag: false })
        + require_send(1)
        + require_send(1isize)
        + require_sync(true)
        + require_sync(2usize)
        + require_send(2.0)
}
"#;

    let ir = compile_with_async_stdlib(source);
    assert!(ir.contains("require_send_Token"));
    assert!(ir.contains("require_sync_Token"));
    assert!(ir.contains("require_send_AutoToken"));
    assert!(ir.contains("require_sync_AutoToken"));
    assert!(ir.contains("require_send_i64"));
    assert!(ir.contains("require_sync_bool"));
    assert!(ir.contains("require_send_f64"));
}

#[test]
fn concurrent_arc_i64_bool_surface_is_send_sync() {
    let source = r#"
def require_send<T: Send>(value: T) -> i64 { 1 }
def require_sync<T: Sync>(value: T) -> i64 { 2 }

def main() -> i64 {
    let shared = arc_new_i64(41);
    let cloned = shared.clone_arc();
    let flag = arc_new_bool(true);
    require_send(cloned)
        + require_sync(flag.clone_arc())
        + shared.get()
        + if flag.get() { 1 } else { 0 }
        + shared.strong_count()
}
"#;

    let ir = compile_with_async_stdlib(source);
    assert!(ir.contains("arc_new_i64"));
    assert!(ir.contains("Arc_i64_clone_arc"));
    assert!(ir.contains("Arc_i64_get"));
    assert!(ir.contains("Arc_bool_clone_arc"));
    assert!(ir.contains("Arc_bool_get"));
    assert!(ir.contains("sengoo_arc_strong_count"));
    assert!(ir.contains("require_send_Arc_i64"));
    assert!(ir.contains("require_sync_Arc_bool"));
    assert!(ir.contains("Arc_i64_Drop_drop"));
    assert!(ir.contains("Arc_bool_Drop_drop"));
}

#[test]
fn concurrent_mutex_guard_i64_surface_lowers_raii_unlock() {
    let source = r#"
async def update(mutex: MutexI64) -> i64 {
    let locked = await mutex_lock_guard_i64(mutex);
    if !locked.is_ok { return locked.error; }
    let guard = locked.value;
    let before = guard.get();
    guard.set(before + 4);
    guard.get()
}

async def main() -> i64 {
    let mutex = mutex_new_i64(5);
    await update(mutex)
}
"#;

    let ir = compile_with_async_stdlib(source);
    assert!(ir.contains("mutex_lock_guard_i64"));
    assert!(ir.contains("MutexGuardI64_get"));
    assert!(ir.contains("MutexGuardI64_set"));
    assert!(ir.contains("MutexGuardI64_Drop_drop"));
    assert!(ir.contains("sengoo_async_mutex_unlock_i64"));
}

#[test]
fn concurrent_rwlock_i64_guards_lower_raii_unlocks() {
    let source = r#"
def read_value(lock: RwLockI64) -> i64 {
    let locked = rwlock_try_read_guard_i64(lock);
    if !locked.is_ok { return locked.error; }
    let guard = locked.value;
    guard.get()
}

def write_value(lock: RwLockI64, value: i64) -> i64 {
    let locked = rwlock_try_write_guard_i64(lock);
    if !locked.is_ok { return locked.error; }
    let guard = locked.value;
    guard.set(value);
    guard.get()
}

def main() -> i64 {
    let lock = rwlock_new_i64(5);
    read_value(lock) + write_value(lock, 9)
}
"#;

    let ir = compile_with_async_stdlib(source);
    assert!(ir.contains("rwlock_try_read_guard_i64"));
    assert!(ir.contains("rwlock_try_write_guard_i64"));
    assert!(ir.contains("RwLockReadGuardI64_get"));
    assert!(ir.contains("RwLockWriteGuardI64_get"));
    assert!(ir.contains("RwLockWriteGuardI64_set"));
    assert!(ir.contains("RwLockReadGuardI64_Drop_drop"));
    assert!(ir.contains("RwLockWriteGuardI64_Drop_drop"));
    assert!(ir.contains("sengoo_async_rwlock_read_guard_unlock_i64"));
    assert!(ir.contains("sengoo_async_rwlock_write_guard_unlock_i64"));
}

#[test]
fn concurrent_send_sync_auto_marker_bounds_reject_runtime_handles() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
struct NotSend {
    buffer: Buffer,
}

def require_send<T: Send>(value: T) -> i64 { 1 }

def main() -> i64 {
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    require_send(NotSend { buffer: buffer })
}
"#
    );

    let err =
        compile_to_ir(&source).expect_err("Buffer-bearing struct should not auto-derive Send");
    let msg = err.to_string();
    assert!(
        msg.contains("not Send") || msg.contains("Send"),
        "expected Send marker diagnostic, got: {msg}"
    );
}

#[test]
fn concurrent_send_sync_auto_marker_bounds_cover_enum_payloads() {
    let safe_source = r#"
enum Message<T> {
    Empty,
    Value(T),
}

def require_send<T: Send>(value: T) -> i64 { 1 }
def require_sync<T: Sync>(value: T) -> i64 { 2 }

def main() -> i64 {
    let number: Message<i64> = Message::Value(7);
    let flag: Message<bool> = Message::Value(true);
    require_send(number) + require_sync(flag)
}
"#;
    let safe_source = format!("{}\n\n{}", async_stdlib_prefix(), safe_source);
    let program = Parser::parse(&safe_source).expect("safe enum marker source should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("enum payloads containing Send/Sync values should satisfy marker bounds");

    let unsafe_source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
enum UnsafeMessage {
    Bytes(Buffer),
}

def require_send<T: Send>(value: T) -> i64 { 1 }

def main() -> i64 {
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    require_send(UnsafeMessage::Bytes(buffer))
}
"#
    );
    let err = compile_to_ir(&unsafe_source)
        .expect_err("an enum carrying Buffer must not auto-derive Send");
    let msg = err.to_string();
    assert!(
        msg.contains("not Send") || msg.contains("Send"),
        "expected enum payload Send diagnostic, got: {msg}"
    );
}

#[test]
fn concurrent_send_sync_auto_marker_bounds_reject_rc_handles() {
    let source = format!(
        "{}\n\n{}",
        collections_async_stdlib_prefix(),
        r#"
def require_send<T: Send>(value: T) -> i64 { 1 }
def require_sync<T: Sync>(value: T) -> i64 { 2 }

def main() -> i64 {
    let shared = rc_new_i64(1);
    require_send(shared) + require_sync(rc_new_bool(true))
}
"#
    );

    let err = compile_to_ir(&source).expect_err("Rc<T> should not auto-derive Send/Sync");
    let msg = err.to_string();
    assert!(
        msg.contains("Rc") || msg.contains("not Send") || msg.contains("Send"),
        "expected Rc Send/Sync marker diagnostic, got: {msg}"
    );
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

impl !Send for JsonValue {}
impl !Sync for JsonValue {}

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

impl !Send for AsyncContext {}
impl !Sync for AsyncContext {}

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
fn concurrent_shared_state_spawn_rejects_non_send_argument() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
async def main() -> i64 {
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let spawned = spawn_shared_counter_i64(buffer, 1, 1);
    if spawned.is_ok { 0 } else { spawned.error }
}
"#
    );

    let err =
        compile_to_ir(&source).expect_err("!Send shared state must not cross a worker boundary");
    let msg = err.to_string();
    assert!(
        msg.contains("shared state") && msg.contains("not Send"),
        "expected stable shared-state Send diagnostic, got: {msg}"
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
