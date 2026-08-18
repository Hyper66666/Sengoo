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
        "collections.sg",
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
fn concurrent_generic_arc_mutex_surface_is_send_sync_and_lowers_raii() {
    let source = r#"
struct Payload {
    value: i64,
}

impl Copy for Payload {}

impl Payload {
    def read(&self) -> i64 { self.value }
}

def require_send<T: Send>(value: T) -> i64 { 1 }
def require_sync<T: Sync>(value: T) -> i64 { 2 }

async def update(shared: Arc<Mutex<Payload>>) -> i64 {
    let locked = await mutex_lock_guard(shared.borrow());
    let mut guard = match locked {
        Ok(value) => value,
        Err(error) => return error,
    };
    let mut before_snapshot = Payload { value: 0 };
    if !mutex_guard_copy_into(&guard, &mut before_snapshot) { return 90; }
    let before = before_snapshot.read();
    guard.set(Payload { value: before + 4 });
    let mut after_snapshot = Payload { value: 0 };
    if !mutex_guard_copy_into(&guard, &mut after_snapshot) { return 91; }
    after_snapshot.read()
}

async def main() -> i64 {
    let shared = arc_new(mutex_new(Payload { value: 5 }));
    require_send(shared.clone_arc()) + require_sync(shared.clone_arc()) + await update(shared)
}
"#;

    let ir = compile_with_async_stdlib(source);
    assert!(ir.contains("sengoo_arc_new"));
    assert!(ir.contains("sengoo_arc_borrow_ptr"));
    assert!(ir.contains("sengoo_async_mutex_new"));
    assert!(ir.contains("sengoo_async_mutex_lock__start"));
    assert!(ir.contains("sengoo_async_mutex_guard_copy_into"));
    assert!(ir.contains("sengoo_async_mutex_guard_set"));
    assert!(ir.contains("Arc_Mutex_Payload_clone_arc"));
    assert!(ir.contains("MutexGuard_Payload_Drop_drop"));
}

#[test]
fn concurrent_arc_mutex_public_shared_counter_uses_generic_composition() {
    let source = r#"
async def main() -> i64 {
    let shared: Arc<Mutex<i64>> = arc_new(mutex_new(2));
    let enabled = runtime_enable_thread_pool(4);
    match enabled {
        Ok(_) => 0,
        Err(error) => return error,
    };

    let first = spawn_shared_counter_i64(shared.clone_arc(), 1, 5);
    let second = spawn_shared_counter_i64(shared.clone_arc(), 1, 5);
    let first_job = match first { Ok(job) => job, Err(_) => return 1 };
    let second_job = match second { Ok(job) => job, Err(_) => return 1 };
    first_job.join();
    second_job.join();

    let locked = await mutex_lock_guard(shared.borrow());
    match locked {
        Ok(guard) => guard.get(),
        Err(error) => error,
    }
}
"#;

    let ir = compile_with_async_stdlib(source);
    assert!(ir.contains("Arc_Mutex_i64_clone_arc"));
    assert!(ir.contains("spawn_shared_counter_i64"));
    assert!(ir.contains("sengoo_async_shared_counter_spawn_add_i64"));
    assert!(ir.contains("sengoo_async_mutex_guard_get"));
    let generic_poll = ir
        .split("; Function: mutex_lock_guard__generic_i64__poll")
        .nth(1)
        .and_then(|tail| tail.split("; Function:").next())
        .expect("generic mutex guard poll helper should be emitted");
    assert!(
        generic_poll.contains("call i64 @sengoo_async_mutex_lock__poll"),
        "generic Mutex<T> must use the descriptor-backed lock lifecycle:\n{generic_poll}"
    );
    assert!(
        !generic_poll.contains("call i64 @sengoo_async_mutex_lock_i64__poll"),
        "generic Mutex<i64> must not fall back to the legacy scalar lock lifecycle:\n{generic_poll}"
    );
}

#[test]
fn concurrent_generic_mutex_guard_get_requires_copy_payload() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
struct OwnedPayload {
    value: i64,
}

impl Drop for OwnedPayload {
    def drop(&mut self) {}
}

async def main() -> i64 {
    let shared = arc_new(mutex_new(OwnedPayload { value: 5 }));
    let locked = await mutex_lock_guard(shared.borrow());
    if !locked.is_ok { return locked.error; }
    let mut output = OwnedPayload { value: 0 };
    mutex_guard_copy_into(&locked.value, &mut output);
    output.value
}
"#
    );

    let error = compile_to_ir(&source)
        .expect_err("non-Copy mutex payloads must not expose a borrowed get result");
    let message = error.to_string();
    assert!(
        message.contains("Copy") || message.contains("method 'get' not found"),
        "expected Copy-bound guard getter diagnostic, got: {message}"
    );
}

#[test]
fn concurrent_generic_arc_mutex_rejects_non_send_payload() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
struct LocalOnly {
    value: i64,
}

impl !Send for LocalOnly {}

def require_send<T: Send>(value: T) -> i64 { 1 }

def main() -> i64 {
    require_send(arc_new(mutex_new(LocalOnly { value: 1 })))
}
"#
    );

    let err = compile_to_ir(&source).expect_err("Arc<Mutex<!Send>> should fail a Send bound");
    let msg = err.to_string();
    assert!(
        msg.contains("not Send") || msg.contains("does not implement `Send`"),
        "expected Send marker diagnostic, got: {msg}"
    );
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
fn concurrent_generic_rwlock_guards_lower_typed_payload_and_raii() {
    let source = r#"
struct Payload {
    value: i64,
}

impl Copy for Payload {}

def read_pair(lock: &RwLock<Payload>) -> i64 {
    let first_result = rwlock_try_read_guard(lock);
    if !first_result.is_ok { return first_result.error; }
    let first = first_result.value;
    let second_result = rwlock_try_read_guard(lock);
    if !second_result.is_ok { return second_result.error; }
    let second = second_result.value;
    let mut left = Payload { value: 0 };
    let mut right = Payload { value: 0 };
    if !rwlock_read_guard_copy_into(&first, &mut left) { return 80; }
    if !rwlock_read_guard_copy_into(&second, &mut right) { return 81; }
    left.value + right.value
}

def write_value(lock: &RwLock<Payload>, value: i64) -> i64 {
    let result = rwlock_try_write_guard(lock);
    if !result.is_ok { return result.error; }
    let mut guard = result.value;
    let replacement = Payload { value: value };
    let wrote = guard.set(replacement);
    if !wrote { return 82; }
    let mut output = Payload { value: 0 };
    if !rwlock_write_guard_copy_into(&guard, &mut output) { return 83; }
    output.value
}

def main() -> i64 {
    let lock = rwlock_new(Payload { value: 5 });
    read_pair(&lock) + write_value(&lock, 9)
}
"#;

    let ir = compile_with_async_stdlib(source);
    assert!(ir.contains("sengoo_async_rwlock_new_parts"));
    assert!(ir.contains("sengoo_async_rwlock_try_read"));
    assert!(ir.contains("sengoo_async_rwlock_try_write"));
    assert!(ir.contains("sengoo_async_rwlock_read_guard_copy_into"));
    assert!(ir.contains("sengoo_async_rwlock_write_guard_copy_into"));
    assert!(ir.contains("sengoo_async_rwlock_write_guard_set"));
    assert!(ir.contains("RwLock_Payload_Drop_drop"));
    assert!(ir.contains("RwLockReadGuard_Payload_Drop_drop"));
    assert!(ir.contains("RwLockWriteGuard_Payload_Drop_drop"));
}

#[test]
fn concurrent_generic_rwlock_copy_requires_copy_payload() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
struct OwnedPayload { value: i64 }
impl Drop for OwnedPayload { def drop(&mut self) {} }

def main() -> i64 {
    let lock = rwlock_new(OwnedPayload { value: 5 });
    let result = rwlock_try_read_guard(&lock);
    if !result.is_ok { return result.error; }
    let mut output = OwnedPayload { value: 0 };
    rwlock_read_guard_copy_into(&result.value, &mut output);
    output.value
}
"#
    );

    let error = compile_to_ir(&source).expect_err("non-Copy rwlock payload must not copy out");
    assert!(error.to_string().contains("Copy"), "error: {error}");
}

#[test]
fn concurrent_generic_rwlock_rejects_non_send_payload() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
struct LocalOnly { value: i64 }
impl !Send for LocalOnly {}

def require_send<T: Send>(value: T) -> i64 { 1 }

def main() -> i64 {
    require_send(rwlock_new(LocalOnly { value: 1 }))
}
"#
    );

    let error = compile_to_ir(&source).expect_err("RwLock<!Send> must fail a Send bound");
    let message = error.to_string();
    assert!(
        message.contains("not Send") || message.contains("does not implement `Send`"),
        "error: {message}"
    );
}

#[test]
fn concurrent_generic_channel_moves_typed_payload_through_async_send_and_recv() {
    let source = r#"
struct Payload {
    value: i64,
}

impl Drop for Payload {
    def drop(&mut self) {}
}

async def round_trip() -> i64 {
    let pair: ChannelPair<Payload> = channel(2);
    let sender = channel_sender(&pair);
    let receiver = channel_receiver(&pair);

    let sent = await channel_send(&sender, Payload { value: 41 });
    if !sent.is_ok { return sent.error; }

    let mut output = Payload { value: 0 };
    let received = await channel_recv_into(&receiver, &mut output);
    if !received.is_ok { return received.error; }
    output.value
}

async def main() -> i64 {
    await round_trip()
}
"#;

    let ir = compile_with_async_stdlib(source);
    assert!(ir.contains("sengoo_async_channel_bounded_parts"));
    assert!(ir.contains("sengoo_async_channel_send__start"));
    assert!(ir.contains("sengoo_async_channel_send__poll"));
    assert!(ir.contains("sengoo_async_channel_send__result"));
    assert!(ir.contains("sengoo_async_channel_recv__start"));
    assert!(ir.contains("sengoo_async_channel_recv__poll"));
    assert!(ir.contains("sengoo_async_channel_recv__result"));
    assert!(ir.contains("sengoo_async_channel_value_move_into"));
    assert!(ir.contains("ChannelPair_Payload_Drop_drop"));
    assert!(
        ir.contains("ChannelSender_Payload_Drop_drop"),
        "generic channel sender must receive scope-exit Drop glue; functions: {:?}",
        ir.lines()
            .filter(|line| line.contains("Function:") && line.contains("Channel"))
            .collect::<Vec<_>>()
    );
    assert!(ir.contains("ChannelReceiver_Payload_Drop_drop"));
}

#[test]
fn concurrent_generic_channel_send_rejects_non_send_payload() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
struct LocalOnly {
    value: i64,
}

impl !Send for LocalOnly {}

async def main() -> i64 {
    let pair: ChannelPair<LocalOnly> = channel(1);
    let sender = channel_sender(&pair);
    let sent = await channel_send(&sender, LocalOnly { value: 1 });
    if sent.is_ok { 0 } else { sent.error }
}
"#
    );

    let error = compile_to_ir(&source).expect_err("channel send must require a Send payload");
    let message = error.to_string();
    assert!(
        message.contains("not Send") || message.contains("does not implement `Send`"),
        "expected Send marker diagnostic, got: {message}"
    );
}

#[test]
fn concurrent_generic_channel_raw_send_cannot_bypass_send_bound() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
struct LocalOnly {
    value: i64,
}

impl !Send for LocalOnly {}

async def main() -> i64 {
    let pair: ChannelPair<LocalOnly> = channel(1);
    let sender = channel_sender(&pair);
    await raw_channel_send(&sender, LocalOnly { value: 1 })
}
"#
    );

    let error = compile_to_ir(&source).expect_err("raw channel send must enforce Send");
    let message = error.to_string();
    assert!(
        message.contains("not Send") || message.contains("does not implement `Send`"),
        "expected Send marker diagnostic, got: {message}"
    );
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
    let server = match bound { Ok(server) => server, Err(error) => return error };
    let outcome = await server.next_request_async(1);
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
    let server = match bound { Ok(server) => server, Err(error) => return error };
    let outcome = await server.next_request(1);
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

#[test]
fn concurrent_rwlock_guard_prevents_moving_its_lock() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
struct Payload { value: i64 }

def consume(lock: RwLock<Payload>) -> i64 { 0 }

def main() -> i64 {
    let lock = rwlock_new(Payload { value: 1 });
    let acquired = rwlock_try_read_guard(&lock);
    if !acquired.is_ok { return acquired.error; }
    let guard = acquired.value;
    consume(lock)
}
"#
    );

    let error = compile_to_ir(&source).expect_err("a lock must outlive its guard");
    assert!(
        error.to_string().contains("cannot move borrowed value"),
        "expected lock-outlives-guard diagnostic, got: {error}"
    );
}

#[test]
fn concurrent_rwlock_guard_cannot_escape_a_borrowed_lock() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
struct Payload { value: i64 }

def leak_guard(lock: &RwLock<Payload>) -> Result<RwLockReadGuard<Payload>, i64> {
    rwlock_try_read_guard(lock)
}

def main() -> i64 { 0 }
"#
    );

    let error = compile_to_ir(&source).expect_err("a guard must not escape a borrowed lock");
    let message = error.to_string();
    assert!(
        message.contains("guard") && (message.contains("escape") || message.contains("outlive")),
        "expected guard escape diagnostic, got: {message}"
    );
}

#[test]
fn concurrent_async_generic_rwlock_guards_lower_read_and_write_lifecycles() {
    let source = r#"
struct Payload { value: i64 }
impl Copy for Payload {}

async def read_once(lock: &RwLock<Payload>) -> i64 {
    let acquired = await rwlock_read_guard(lock);
    if !acquired.is_ok { return acquired.error; }
    let guard = acquired.value;
    let mut output = Payload { value: 0 };
    if !rwlock_read_guard_copy_into(&guard, &mut output) { return 80; }
    output.value
}

async def write_once(lock: &RwLock<Payload>) -> i64 {
    let acquired = await rwlock_write_guard(lock);
    if !acquired.is_ok { return acquired.error; }
    let mut guard = acquired.value;
    guard.set(Payload { value: 9 });
    let mut output = Payload { value: 0 };
    if !rwlock_write_guard_copy_into(&guard, &mut output) { return 81; }
    output.value
}

async def main() -> i64 {
    let lock = rwlock_new(Payload { value: 5 });
    await read_once(&lock) + await write_once(&lock)
}
"#;

    let ir = compile_with_async_stdlib(source);
    for symbol in [
        "sengoo_async_rwlock_read__start",
        "sengoo_async_rwlock_read__poll",
        "sengoo_async_rwlock_read__result",
        "sengoo_async_rwlock_write__start",
        "sengoo_async_rwlock_write__poll",
        "sengoo_async_rwlock_write__result",
    ] {
        assert!(ir.contains(symbol), "missing async RwLock symbol {symbol}");
    }
    assert!(ir.contains("RwLockReadGuard_Payload_Drop_drop"));
    assert!(ir.contains("RwLockWriteGuard_Payload_Drop_drop"));
}

#[test]
fn concurrent_mutex_guard_prevents_moving_its_lock() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
struct Payload { value: i64 }

def consume(lock: Mutex<Payload>) -> i64 { 0 }

async def main() -> i64 {
    let lock = mutex_new(Payload { value: 1 });
    let acquired = await mutex_lock_guard(&lock);
    if !acquired.is_ok { return acquired.error; }
    let guard = acquired.value;
    consume(lock)
}
"#
    );

    let error = compile_to_ir(&source).expect_err("a mutex must outlive its guard");
    assert!(
        error.to_string().contains("cannot move borrowed value"),
        "expected lock-outlives-guard diagnostic, got: {error}"
    );
}

#[test]
fn concurrent_mutex_guard_cannot_escape_a_borrowed_lock() {
    let source = format!(
        "{}\n\n{}",
        async_stdlib_prefix(),
        r#"
struct Payload { value: i64 }

async def leak_guard(lock: &Mutex<Payload>) -> Result<MutexGuard<Payload>, i64> {
    await mutex_lock_guard(lock)
}

async def main() -> i64 { 0 }
"#
    );

    let error = compile_to_ir(&source).expect_err("a mutex guard must not escape");
    let message = error.to_string();
    assert!(
        message.contains("guard") && (message.contains("escape") || message.contains("outlive")),
        "expected guard escape diagnostic, got: {message}"
    );
}

#[test]
fn concurrent_spawn_task_rejects_non_send_future_arguments() {
    let source = r#"
struct LocalHandle { raw: i64 }
impl !Send for LocalHandle {}

async def consume(value: LocalHandle) -> i64 { value.raw }

async def main() -> i64 {
    let local = LocalHandle { raw: 1 };
    spawn_task(consume(local))
}
"#;

    let error = compile_to_ir(source).expect_err("spawn_task must require a Send future");
    assert!(
        error.to_string().contains("not Send"),
        "expected stable Send diagnostic, got: {error}"
    );
}

#[test]
fn concurrent_spawn_rejects_non_send_future_arguments() {
    let source = r#"
struct LocalHandle { raw: i64 }
impl !Send for LocalHandle {}

async def consume(value: LocalHandle) -> i64 { value.raw }

async def main() -> i64 {
    let local = LocalHandle { raw: 1 };
    await spawn(consume(local))
}
"#;

    let error = compile_to_ir(source).expect_err("spawn must require a Send future");
    assert!(
        error.to_string().contains("not Send"),
        "expected stable Send diagnostic, got: {error}"
    );
}

#[test]
fn concurrent_spawn_task_rejects_capturing_future_factory() {
    let source = r#"
struct LocalHandle { raw: i64 }
impl !Send for LocalHandle {}

async def consume(value: LocalHandle) -> i64 { value.raw }

async def main() -> i64 {
    let local = LocalHandle { raw: 1 };
    let factory = | | consume(local);
    spawn_task(factory())
}
"#;

    let error = compile_to_ir(source).expect_err("spawn_task must reject callable captures");
    assert!(
        error.to_string().contains("directly called async function"),
        "expected direct-future diagnostic, got: {error}"
    );
}

#[test]
fn concurrent_spawn_rejects_capturing_future_factory() {
    let source = r#"
struct LocalHandle { raw: i64 }
impl !Send for LocalHandle {}

async def consume(value: LocalHandle) -> i64 { value.raw }

async def main() -> i64 {
    let local = LocalHandle { raw: 1 };
    let factory = | | consume(local);
    await spawn(factory())
}
"#;

    let error = compile_to_ir(source).expect_err("spawn must reject callable captures");
    assert!(
        error.to_string().contains("directly called async function"),
        "expected direct-future diagnostic, got: {error}"
    );
}

#[test]
fn concurrent_spawn_task_accepts_direct_multisegment_async_function_path() {
    let source = r#"
async def worker_child(value: i64) -> i64 { value }

async def main() -> i64 {
    spawn_task(worker::child(42))
}
"#;

    compile_to_ir(source).expect("direct multi-segment async calls should remain spawnable");
}

fn task_scope_test_prelude() -> &'static str {
    r#"
struct TaskScope { handle: i64 }
impl !Send for TaskScope {}
impl !Sync for TaskScope {}

impl Drop for TaskScope {
    def drop(&mut self) {}
}

async def scoped_child() -> i64 { 7 }
"#
}

#[test]
fn structured_task_scope_normal_fallthrough_joins_before_drop() {
    let source = format!(
        "{}\n{}",
        task_scope_test_prelude(),
        r#"
async def main() -> i64 {
    let scope = task_scope();
    scope_spawn(&scope, scoped_child());
    42
}
"#
    );

    let mir = compile_to_mir(&source).expect("normal task scope should lower");
    let body = mir
        .iter()
        .find(|function| function.name == "main__body")
        .expect("async main body should exist");
    let calls = body
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            Instruction::Call { func, .. } => Some(func.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let join = calls
        .iter()
        .position(|name| *name == "sengoo_async_task_scope_join")
        .expect("normal scope exit should join children");
    let drop = calls
        .iter()
        .position(|name| *name == "TaskScope_Drop_drop")
        .expect("scope guard should still run idempotent Drop");
    assert!(join < drop, "normal join must run before guard Drop");
}

#[test]
fn structured_task_scope_explicit_return_uses_cancel_drop_without_normal_join() {
    let source = format!(
        "{}\n{}",
        task_scope_test_prelude(),
        r#"
async def main() -> i64 {
    let scope = task_scope();
    scope_spawn(&scope, scoped_child());
    return 42;
}
"#
    );

    let mir = compile_to_mir(&source).expect("early task scope exit should lower");
    let body = mir
        .iter()
        .find(|function| function.name == "main__body")
        .expect("async main body should exist");
    assert!(!body.instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::Call { func, .. } if func == "sengoo_async_task_scope_join")
    }));
    assert!(body.instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::Call { func, .. } if func == "TaskScope_Drop_drop")
    }));
}

#[test]
fn structured_task_scope_cannot_escape_through_return() {
    let source = format!(
        "{}\n{}",
        task_scope_test_prelude(),
        r#"
def leak_scope() -> TaskScope { task_scope() }
def main() -> i64 { 0 }
"#
    );

    let error = compile_to_ir(&source).expect_err("TaskScope return must be rejected");
    assert!(error.to_string().contains("TaskScope cannot escape"));
}

#[test]
fn structured_task_scope_cannot_be_stored_in_aggregate_fields() {
    let source = format!(
        "{}\n{}",
        task_scope_test_prelude(),
        r#"
struct BadOwner { scope: TaskScope }
def main() -> i64 { 0 }
"#
    );

    let error = compile_to_ir(&source).expect_err("TaskScope aggregate field must be rejected");
    assert!(error.to_string().contains("TaskScope cannot escape"));
}

#[test]
fn structured_task_scope_cannot_be_stored_in_local_aggregates() {
    let source = format!(
        "{}\n{}",
        task_scope_test_prelude(),
        r#"
async def main() -> i64 {
    let pair = (task_scope(), 1);
    0
}
"#
    );

    let error = compile_to_ir(&source).expect_err("TaskScope local aggregate must be rejected");
    assert!(
        error.to_string().contains("TaskScope cannot escape"),
        "expected lexical-owner diagnostic, got: {error}"
    );
}

#[test]
fn structured_task_scope_question_mark_failure_skips_normal_join() {
    let source = format!(
        "{}\n{}",
        task_scope_test_prelude(),
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }

def fail() -> Result<i64, i64> {
    Result { is_ok: false, value: 0, error: 9 }
}

async def scoped_result() -> Result<i64, i64> {
    let scope = task_scope();
    scope_spawn(&scope, scoped_child());
    let value = fail()?;
    Result { is_ok: true, value: value, error: 0 }
}

def main() -> i64 { 0 }
"#
    );

    let mir = compile_to_mir(&source).expect("scoped Result propagation should lower");
    let body = mir
        .iter()
        .find(|function| function.name == "scoped_result")
        .expect("async Result body should exist");
    let joins = body
        .instructions
        .iter()
        .filter(|instruction| {
            matches!(instruction, Instruction::Call { func, .. } if func == "sengoo_async_task_scope_join")
        })
        .count();
    let drops = body
        .instructions
        .iter()
        .filter(|instruction| {
            matches!(instruction, Instruction::Call { func, .. } if func == "TaskScope_Drop_drop")
        })
        .count();
    assert_eq!(
        joins, 1,
        "only the success fallthrough should join normally"
    );
    assert!(
        drops >= 2,
        "success and ? failure exits should both drop scope"
    );
}

#[test]
fn structured_task_scope_break_uses_cancel_drop_without_normal_join() {
    let source = format!(
        "{}\n{}",
        task_scope_test_prelude(),
        r#"
async def main() -> i64 {
    loop {
        let scope = task_scope();
        scope_spawn(&scope, scoped_child());
        break;
    }
    42
}
"#
    );

    let mir = compile_to_mir(&source).expect("scoped break should lower");
    let body = mir
        .iter()
        .find(|function| function.name == "main__body")
        .expect("async main body should exist");
    assert!(!body.instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::Call { func, .. } if func == "sengoo_async_task_scope_join")
    }));
    assert!(body.instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::Call { func, .. } if func == "TaskScope_Drop_drop")
    }));
}

#[test]
fn structured_task_scope_handle_cannot_be_forged_with_struct_literal() {
    let source = format!(
        "{}\n{}",
        task_scope_test_prelude(),
        r#"
def main() -> i64 {
    let forged = TaskScope { handle: 1 };
    0
}
"#
    );

    let error = compile_to_ir(&source).expect_err("TaskScope construction must be opaque");
    assert!(
        error.to_string().contains("TaskScope is opaque"),
        "expected opaque scope diagnostic, got: {error}"
    );
}
