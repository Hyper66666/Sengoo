use crate::compile_to_ir;
use std::fs;
use std::path::Path;

fn load_stdlib_surface(modules: &[&str]) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap_or(manifest_dir);
    let stdlib_root = workspace_root.join("tools").join("stdlib");
    modules
        .iter()
        .map(|module| {
            let stdlib_path = stdlib_root.join(module);
            fs::read_to_string(&stdlib_path).unwrap_or_else(|err| {
                panic!(
                    "failed to read stdlib surface {}: {err}",
                    stdlib_path.display()
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn compile_with_stdlib(program: &str) -> String {
    compile_with_stdlib_modules(&["option.sg", "result.sg", "collections.sg"], program)
}

fn compile_with_stdlib_modules(modules: &[&str], program: &str) -> String {
    let source = format!("{}\n\n{}", load_stdlib_surface(modules), program);
    compile_to_ir(&source)
        .unwrap_or_else(|err| panic!("stdlib surface program should compile: {err}"))
}

#[test]
fn option_module_imports_and_unwraps() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg"],
        r#"
def main() -> i64 {
    option_some_i64(41).map_add(1).unwrap_or(0)
}
"#,
    );

    assert!(ir.contains("option_some_i64"));
    assert!(ir.contains("Option_i64_map_add"));
}

#[test]
fn result_module_imports_and_chains() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg"],
        r#"
def main() -> i64 {
    result_ok_i64(20).map_add(1).and_then_mul(2).unwrap_or(0)
}
"#,
    );

    assert!(ir.contains("result_ok_i64"));
    assert!(ir.contains("Result_i64_i64_and_then_mul"));
}

#[test]
fn string_module_imports_and_runs_str_len() {
    let ir = compile_with_stdlib_modules(
        &["string.sg"],
        r#"
def main() -> i64 {
    let greeting = str_append("he", "llo");
    let repeated = str_repeat("ha", 2);

    if str_len(greeting) == 5
        && str_eq(greeting, "hello")
        && str_ne(greeting, repeated)
        && str_is_empty("")
        && str_is_not_empty(repeated) {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_str_len"));
    assert!(ir.contains("sengoo_str_concat"));
    assert!(ir.contains("sengoo_str_eq"));
    assert!(ir.contains("str_repeat"));
}

#[test]
fn math_module_imports_and_runs_abs_i64() {
    let ir = compile_with_stdlib_modules(
        &["math.sg"],
        r#"
def main() -> i64 {
    abs_i64(0 - 7)
        + min_i64(4, 9)
        + max_i64(4, 9)
        + pow_i64(2, 3)
        + sign_i64(0 - 9)
        + clamp_i64(12, 0, 10)
        + gcd_i64(54, 24)
        + lcm_i64(6, 8)
}
"#,
    );

    assert!(ir.contains("abs_i64"));
    assert!(ir.contains("pow_i64"));
    assert!(ir.contains("sign_i64"));
    assert!(ir.contains("clamp_i64"));
    assert!(ir.contains("gcd_i64"));
    assert!(ir.contains("lcm_i64"));
}

#[test]
fn error_module_imports_and_asserts_true() {
    let ir = compile_with_stdlib_modules(
        &["error.sg"],
        r#"
def main() -> i64 {
    if assert_eq_i64(4, 4) {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("assert_eq_i64"));
    assert!(ir.contains("sengoo_panic_option_unwrap_i64"));
}

#[test]
fn error_module_imports_common_assertion_helpers() {
    let ir = compile_with_stdlib_modules(
        &["error.sg"],
        r#"
def main() -> i64 {
    if assert_true(true)
        && assert_false(false)
        && assert_eq_i64(4, 4)
        && assert_ne_i64(4, 5)
        && assert_eq_bool(true, true)
        && assert_ne_bool(true, false)
        && assert_eq_str("ok", "ok")
        && assert_ne_str("ok", "no")
        && assert_eq_f64(1.5, 1.5)
        && assert_ne_f64(1.5, 2.5) {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("assert_false"));
    assert!(ir.contains("assert_ne_i64"));
    assert!(ir.contains("assert_eq_bool"));
    assert!(ir.contains("assert_ne_str"));
    assert!(ir.contains("assert_eq_f64"));
    assert!(ir.contains("sengoo_str_eq"));
}

#[test]
fn ffi_buffer_from_bytes_raw_returns_result() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg"],
        r#"
def main() -> i64 {
    let result = ffi_buffer_from_bytes_raw(0, 4);
    if result.is_err() {
        ffi_last_error_code()
    } else {
        let buffer = result.unwrap_or(Buffer { handle: 0 });
        if buffer.free() {
            0
        } else {
            1
        }
    }
}
"#,
    );

    assert!(ir.contains("sengoo_ffi_buffer_from_bytes"));
}

#[test]
fn stdlib_ffi_error_and_copy_wrappers_accept_managed_buffers() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg"],
        r#"
def main() -> i64 {
    let source = ffi_buffer_from_bytes("abc").unwrap_or(Buffer { handle: 0 });
    let out = ffi_buffer_new(16).unwrap_or(Buffer { handle: 0 });
    let error_len = ffi_last_error_copy(out).unwrap_or(0);
    let copied = source.copy_out(out).unwrap_or(0);
    source.free();
    out.free();
    error_len + copied
}
"#,
    );

    assert!(ir.contains("sengoo_ffi_last_error_copy"));
    assert!(ir.contains("sengoo_ffi_buffer_copy_out"));
}

#[test]
fn stdlib_ffi_and_lua54_value_calls_avoid_pointer_slots() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "lua54.sg"],
        r#"
def main() -> i64 {
    let lib = CLib { handle: 0 };
    let ffi_value = lib.call_i64_2("add", 20, 22).unwrap_or(0);
    let object = lib.object_create_1("counter_new", 5, "counter_drop").unwrap_or(CppObject { handle: 0 });
    let object_value = object.call_i64_1("counter_add", 7).unwrap_or(0);
    let lua = Lua54 { handle: 0 };
    let lua_value = lua.call_i64_2("add", 2, 5).unwrap_or(0);
    ffi_value + object_value + lua_value
}
"#,
    );

    assert!(ir.contains("sengoo_ffi_c_call_i64_value"));
    assert!(ir.contains("sengoo_ffi_object_create_value"));
    assert!(ir.contains("sengoo_ffi_object_call_i64_value"));
    assert!(ir.contains("sengoo_lua54_call_i64_value"));
}

#[test]
fn stdlib_reflection_wrappers_accept_strings_without_raw_pointers() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "db.sg",
            "lua54.sg",
            "net.sg",
        ],
        r#"
def main() -> i64 {
    let _lib = ffi_open("missing.dll");
    let _db = db_open("sqlite::memory:");
    let _lua = lua54_open("");
    let _tcp = net_tcp_connect("127.0.0.1", 65535, 1);
    let _udp = udp_bind("127.0.0.1", 0);
    let _http = http_get("http://127.0.0.1/", 1);
    let _ws = ws_connect("ws://127.0.0.1/", 1);
    0
}
"#,
    );

    assert!(ir.contains("; Function: ffi_open"));
    assert!(ir.contains("; Function: db_open"));
    assert!(ir.contains("; Function: lua54_open"));
    assert!(ir.contains("; Function: net_tcp_connect"));
    assert!(ir.contains("; Function: http_get"));
    assert!(ir.contains("declare i64 @sengoo_stdlib_str_ptr(i8*)"));
}

#[test]
fn stdlib_proto_wrapper_accepts_event_name_string_and_buffer() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "proto.sg"],
        r#"
def main() -> i64 {
    let event = proto_user_event(7, "alice", 42);
    let buffer = ffi_buffer_new(128).unwrap_or(Buffer { handle: 0 });
    let encoded = proto_user_event_encode(event, buffer);
    buffer.free();
    encoded.unwrap_or(0)
}
"#,
    );

    assert!(ir.contains("; Function: proto_user_event"));
    assert!(ir.contains("declare i64 @sengoo_stdlib_str_ptr(i8*)"));
    assert!(ir.contains("sengoo_ffi_buffer_new"));
    assert!(ir.contains("sengoo_proto_user_event_encode"));
}

#[test]
fn stdlib_proto_decode_wrapper_owns_decoded_event_fields() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "proto.sg"],
        r#"
def main() -> i64 {
    let encoded = ffi_buffer_new(128).unwrap_or(Buffer { handle: 0 });
    let name = ffi_buffer_new(32).unwrap_or(Buffer { handle: 0 });
    let decoded = proto_user_event_decode(encoded, 16).unwrap_or(ProtoDecodedUserEvent { handle: 0 });
    let id = decoded.id();
    let ts = decoded.ts();
    let copied = decoded.name_copy(name).unwrap_or(0);
    decoded.close();
    encoded.free();
    name.free();
    id + ts + copied
}
"#,
    );

    assert!(ir.contains("sengoo_proto_user_event_decode_open"));
    assert!(ir.contains("sengoo_proto_user_event_decoded_id"));
    assert!(ir.contains("sengoo_proto_user_event_decoded_name_copy"));
    assert!(ir.contains("sengoo_proto_user_event_decoded_close"));
}

#[test]
fn stdlib_net_wrappers_accept_managed_buffers() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "net.sg"],
        r#"
def main() -> i64 {
    let buffer = ffi_buffer_new(256).unwrap_or(Buffer { handle: 0 });
    let tcp = TcpStream { handle: 0 };
    let udp = UdpSocket { handle: 0 };
    let http = HttpClient { handle: 0 };
    let ws = WsClient { handle: 0 };

    let a = net_error_name_copy(0, buffer).unwrap_or(0);
    let b = tcp.recv(buffer, 1).unwrap_or(0);
    let c = udp.recv(buffer, 1).unwrap_or(0);
    let d = http.body_copy(buffer).unwrap_or(0);
    let e = ws.recv_text(buffer, 1).unwrap_or(0);
    let f = net_bench_last_error_copy(buffer).unwrap_or(0);
    let g = net_bench_run(1, 1, 1, 8, buffer).unwrap_or(0);
    buffer.free();
    a + b + c + d + e + f + g
}
"#,
    );

    assert!(ir.contains("sengoo_net_error_name_copy"));
    assert!(ir.contains("sengoo_tcp_recv"));
    assert!(ir.contains("sengoo_udp_recv"));
    assert!(ir.contains("sengoo_http_body_copy"));
    assert!(ir.contains("sengoo_ws_recv_text"));
    assert!(ir.contains("sengoo_net_bench_last_error_copy"));
    assert!(ir.contains("sengoo_net_bench_run"));
}

#[test]
fn stdlib_net_http_server_wrappers_accept_strings() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "net.sg"],
        r#"
def main() -> i64 {
    let server = http_server_bind("127.0.0.1", 0).unwrap_or(HttpServer { handle: 0 });
    let port = server.local_port().unwrap_or(0);
    let limited = server.set_limits(4096, 8192).unwrap_or(false);
    let routed = server.add_route("GET", "/hello/:name", 200, "hello {name}").unwrap_or(false);
    let guarded = server.require_header("x-token", "secret", 401, "denied").unwrap_or(false);
    let websocket = server.add_ws_echo_route("/ws").unwrap_or(false);
    let served = server.serve_once(1).unwrap_or(false);
    server.close();
    if port >= 0 && limited && routed && guarded && websocket && !served { 1 } else { 0 }
}
"#,
    );

    assert!(ir.contains("sengoo_http_server_bind"));
    assert!(ir.contains("sengoo_http_server_add_route"));
    assert!(ir.contains("sengoo_http_server_add_middleware_require_header"));
    assert!(ir.contains("sengoo_http_server_add_ws_echo_route"));
    assert!(ir.contains("sengoo_http_server_serve_once"));
}

#[test]
fn stdlib_db_and_lua54_wrappers_accept_managed_buffers() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "db.sg", "lua54.sg"],
        r#"
def main() -> i64 {
    let buffer = ffi_buffer_new(256).unwrap_or(Buffer { handle: 0 });
    let result = DbResult { handle: 0 };

    let a = db_last_error_len();
    let b = db_last_error_copy(buffer).unwrap_or(0);
    let c = result.col_name_len(0);
    let d = result.col_name_copy(0, buffer).unwrap_or(0);
    let e = result.cell_len(0, 0);
    let f = result.cell_copy(0, 0, buffer).unwrap_or(0);
    let g = lua54_last_error_len();
    let h = lua54_last_error_copy(buffer).unwrap_or(0);
    let cleared = lua54_last_error_clear();
    buffer.free();
    if cleared {
        a + b + c + d + e + f + g + h
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_db_last_error_copy"));
    assert!(ir.contains("sengoo_db_result_col_name_copy"));
    assert!(ir.contains("sengoo_db_result_cell_copy"));
    assert!(ir.contains("sengoo_lua54_last_error_copy"));
}

#[test]
fn stdlib_surface_vec_and_hashmap_compile_and_emit_runtime_calls() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    vec.push(4);
    vec.push(8);
    let got = vec.get(1).unwrap_or(0);

    let map = hashmap_new_i64_i64();
    map.insert(7, got);
    map.get(7).unwrap_or(0)
}
"#,
    );

    assert!(ir.contains("sengoo_vec_new_i64"));
    assert!(ir.contains("sengoo_vec_push_i64"));
    assert!(ir.contains("sengoo_hashmap_insert_i64"));
    assert!(ir.contains("sengoo_hashmap_get_or_default_i64"));
}

#[test]
fn stdlib_surface_iterator_adapters_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    vec.push(1);
    vec.push(2);
    vec.push(3);
    vec.push(4);

    let iter = vec.iter();
    let mapped = iter.map_add(5);
    let evens = iter.filter_even();
    mapped.unwrap_or(0) + evens.unwrap_or(0)
}
"#,
    );

    assert!(ir.contains("sengoo_vec_iter_new_i64"));
    assert!(ir.contains("sengoo_vec_iter_next_or_default_i64"));
}

#[test]
fn stdlib_surface_option_and_result_ergonomics_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let option_value = option_some_i64(2).map_add(3).and_then_mul(4);
    let result_value = result_ok_i64(option_value.unwrap_or(0)).map_add(1).and_then_mul(2);
    let err_value = result_err_i64(5).map_err_add(7);
    result_value.unwrap_or(0) + err_value.unwrap_or(0)
}
"#,
    );

    assert!(ir.contains("option_some_i64"));
    assert!(ir.contains("result_ok_i64"));
    assert!(ir.contains("result_err_i64"));
}

#[test]
fn stdlib_surface_vec_remove_and_contains_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    vec.push(3);
    vec.push(5);
    vec.push(7);

    let had_five = vec.contains(5);
    let removed = vec.remove(1).unwrap_or(0);
    let still_has_five = vec.contains(5);
    let tail = vec.get(1).unwrap_or(0);

    if had_five && !still_has_five {
        removed + tail
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_vec_contains_i64"));
    assert!(ir.contains("sengoo_vec_remove_or_default_i64"));
}

#[test]
fn stdlib_surface_iterator_higher_order_adapters_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    vec.push(1);
    vec.push(2);
    vec.push(3);
    vec.push(4);

    let add1 = |x| x + 1;

    let iter = vec.iter();
    let mapped = iter.map_with(add1).unwrap_or(0);
    iter.reset();
    let filtered = iter.filter_with(add1).unwrap_or(0);
    mapped + filtered
}
"#,
    );

    assert!(!ir.contains("call i64 @f("));
}

#[test]
fn stdlib_surface_hashmap_iter_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let map = hashmap_new_i64_i64();
    map.insert(1, 10);
    map.insert(2, 20);
    let iter = map.iter();
    let first = iter.next().unwrap_or(0);
    first
}
"#,
    );

    assert!(ir.contains("sengoo_hashmap_iter_new_i64"));
    assert!(ir.contains("sengoo_hashmap_iter_next_or_default_i64"));
}

#[test]
fn stdlib_surface_clear_and_is_empty_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    vec.push(1);
    vec.clear();

    let map = hashmap_new_i64_i64();
    map.insert(1, 2);
    map.clear();

    if vec.is_empty() && map.is_empty() {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_vec_clear_i64_status"));
    assert!(ir.contains("sengoo_hashmap_clear_i64_status"));
}

#[test]
fn function_value_parameter_uses_indirect_call_ir() {
    let source = r#"
def apply_twice(x: i64, f: fn(i64) -> i64) -> i64 {
    f(f(x))
}

def main() -> i64 {
    let add1 = |y| y + 1;
    apply_twice(40, add1)
}
"#;

    let ir = compile_to_ir(source)
        .unwrap_or_else(|err| panic!("function-value call should compile: {err}"));
    assert!(
        !ir.contains("call i64 @f("),
        "indirect call should not lower to literal @f
{}",
        ir
    );
}

#[test]
fn function_returning_struct_preserves_receiver_type_for_method_calls() {
    let source = r#"
struct Point {
    x: i64,
    y: i64,
}

def make_point() -> Point {
    Point { x: 1, y: 2 }
}

impl Point {
    def sum(self) -> i64 {
        self.x + self.y
    }
}

def main() -> i64 {
    let point = make_point();
    point.sum()
}
"#;

    let ir = compile_to_ir(source).unwrap_or_else(|err| {
        panic!("struct-returning function should preserve receiver type: {err}")
    });
    assert!(ir.contains("Point_sum"));
}

#[test]
fn stdlib_surface_generic_i64_instantiations_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec: Vec<i64> = vec_new_i64();
    vec.push(5);
    let popped: Option<i64> = vec.pop();

    let map: HashMap<i64, i64> = hashmap_new_i64_i64();
    map.insert(1, popped.unwrap_or(0));
    let got: Option<i64> = map.get(1);

    let result: Result<i64, i64> = result_ok_i64(got.unwrap_or(0));
    result.unwrap_or(0)
}
"#,
    );

    assert!(ir.contains("sengoo_vec_push_i64"));
    assert!(ir.contains("sengoo_hashmap_insert_i64"));
}

#[test]
fn stdlib_surface_generic_handle_and_sum_methods_compile() {
    let ir = compile_with_stdlib(
        r#"
def option_flag(opt: Option<bool>) -> bool {
    opt.is_some() && !opt.is_none() && opt.unwrap_or(false)
}

def result_flag(res: Result<bool, i64>) -> bool {
    res.is_ok() && !res.is_err() && res.unwrap_or(false)
}

def main() -> i64 {
    let vec: Vec<bool> = Vec { handle: 0, marker: false };
    let map: HashMap<bool, bool> = HashMap { handle: 0, key_marker: false, value_marker: false };
    let option_true: Option<bool> = Option { is_some: true, value: true };
    let result_true: Result<bool, i64> = Result { is_ok: true, value: true, error: 0 };

    if vec.is_empty() && map.is_empty() && option_flag(option_true) && result_flag(result_true) {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_vec_len_i64"));
    assert!(ir.contains("sengoo_hashmap_len_i64"));
}

#[test]
fn stdlib_surface_vec_runtime_mutators_support_i64_and_bool() {
    let i64_ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec: Vec<i64> = Vec { handle: 0, marker: 0 };
    if vec.push(1) { 1 } else { 0 }
}
"#,
    );
    assert!(i64_ir.contains("sengoo_vec_push_i64"));

    let bool_ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec: Vec<bool> = vec_new_bool();
    vec.push(true);
    vec.push(false);

    let first = vec.get(0).unwrap_or(false);
    let second = vec.pop().unwrap_or(true);
    let had_true = vec.contains(true);
    vec.set(0, false);
    let removed = vec.remove(0).unwrap_or(true);

    if first && !second && had_true && !removed && vec.is_empty() {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(bool_ir.contains("vec_new_bool"));
    assert!(bool_ir.contains("Vec_bool_push"));
    assert!(bool_ir.contains("Vec_bool_get"));
    assert!(bool_ir.contains("Vec_bool_remove"));
}

#[test]
fn stdlib_surface_hashmap_runtime_mutators_support_i64_and_bool() {
    let i64_ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let map: HashMap<i64, i64> = HashMap { handle: 0, key_marker: 0, value_marker: 0 };
    if map.insert(1, 2) { 1 } else { 0 }
}
"#,
    );
    assert!(i64_ir.contains("sengoo_hashmap_insert_i64"));

    let bool_ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let map: HashMap<bool, bool> = hashmap_new_bool_bool();
    map.insert(true, false);
    map.insert(false, true);

    let true_value = map.get(true).unwrap_or(true);
    let false_value = map.get(false).unwrap_or(false);
    let had_true = map.contains(true);
    let removed_true = map.remove(true);
    let missing_true = map.get(true).unwrap_or(true);

    let mixed_key = hashmap_new_bool_i64();
    mixed_key.insert(true, 6);

    let mixed_value = hashmap_new_i64_bool();
    mixed_value.insert(3, true);

    if !true_value
        && false_value
        && had_true
        && removed_true
        && missing_true
        && mixed_key.get(true).unwrap_or(0) == 6
        && mixed_value.get(3).unwrap_or(false) {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(bool_ir.contains("hashmap_new_bool_bool"));
    assert!(bool_ir.contains("HashMap_bool_bool_insert"));
    assert!(bool_ir.contains("HashMap_bool_i64_get"));
    assert!(bool_ir.contains("HashMap_i64_bool_get"));
}

#[test]
fn stdlib_surface_generic_bool_methods_emit_bool_returns() {
    let ir = compile_with_stdlib(
        r#"
def option_flag(opt: Option<bool>) -> bool {
    opt.is_none() || opt.unwrap_or(false)
}

def result_flag(res: Result<bool, i64>) -> bool {
    res.is_err() || res.unwrap_or(false)
}

def main() -> i64 {
    0
}
"#,
    );

    let option_section = ir
        .split("; Function: Option_bool_is_none")
        .nth(1)
        .expect("Option_bool_is_none should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        option_section.contains("ret i1"),
        "Option_bool_is_none should return i1
{}",
        option_section
    );
    assert!(
        !option_section.contains("ret i64"),
        "Option_bool_is_none should not return i64
{}",
        option_section
    );

    let option_unwrap_section = ir
        .split("; Function: Option_bool_unwrap_or")
        .nth(1)
        .expect("Option_bool_unwrap_or should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        option_unwrap_section.contains("define i1 @Option_bool_unwrap_or"),
        "Option_bool_unwrap_or should return i1
{}",
        option_unwrap_section
    );
    assert!(
        !option_unwrap_section.contains("define i64 @Option_bool_unwrap_or"),
        "Option_bool_unwrap_or should not return i64
{}",
        option_unwrap_section
    );

    let result_section = ir
        .split("; Function: Result_bool_i64_is_err")
        .nth(1)
        .expect("Result_bool_i64_is_err should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        result_section.contains("ret i1"),
        "Result_bool_i64_is_err should return i1
{}",
        result_section
    );
    assert!(
        !result_section.contains("ret i64"),
        "Result_bool_i64_is_err should not return i64
{}",
        result_section
    );

    let result_unwrap_section = ir
        .split("; Function: Result_bool_i64_unwrap_or")
        .nth(1)
        .expect("Result_bool_i64_unwrap_or should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        result_unwrap_section.contains("define i1 @Result_bool_i64_unwrap_or"),
        "Result_bool_i64_unwrap_or should return i1
{}",
        result_unwrap_section
    );
    assert!(
        !result_unwrap_section.contains("define i64 @Result_bool_i64_unwrap_or"),
        "Result_bool_i64_unwrap_or should not return i64
{}",
        result_unwrap_section
    );
}
#[test]
fn stdlib_surface_generic_result_projection_methods_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let ok_result: Result<bool, i64> = Result { is_ok: true, value: true, error: 6 };
    let err_result: Result<i64, bool> = Result { is_ok: false, value: 0, error: true };
    let ok_option: Option<bool> = ok_result.ok();
    let err_option: Option<bool> = err_result.err();

    if ok_option.unwrap_or(false) && err_option.unwrap_or(false) {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(
        ir.contains("; Function: Result_bool_i64_ok"),
        "expected Result<bool, i64>.ok specialization
{}",
        ir
    );
    assert!(
        ir.contains("; Function: Result_i64_bool_err"),
        "expected Result<i64, bool>.err specialization
{}",
        ir
    );
    assert!(
        ir.contains("; Function: Option_bool_unwrap_or"),
        "expected Option<bool>.unwrap_or specialization
{}",
        ir
    );
}

#[test]
fn stdlib_surface_method_generic_ok_or_emits_specialized_bool_error_variants() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let some_flag: Option<bool> = Option { is_some: true, value: true };
    let none_flag: Option<bool> = Option { is_some: false, value: false };

    let ok_result = some_flag.ok_or(false);
    let err_result = none_flag.ok_or(true);

    if ok_result.ok().unwrap_or(false) && err_result.err().unwrap_or(false) {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(
        ir.contains("; Function: Option_bool_ok_or_bool"),
        "expected method-generic ok_or specialization for bool error
{}",
        ir
    );
    assert!(
        ir.contains("; Function: Result_bool_bool_ok"),
        "expected Result<bool, bool>.ok specialization
{}",
        ir
    );
    assert!(
        ir.contains("; Function: Result_bool_bool_err"),
        "expected Result<bool, bool>.err specialization
{}",
        ir
    );
    assert!(
        !ir.contains("define %Result_bool_i64 @Option_bool_ok_or("),
        "unexpected unresolved ok_or lowering leaked into IR
{}",
        ir
    );
}

#[test]
fn stdlib_surface_generic_option_result_constructors_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let some_flag: Option<bool> = option_some(true);
    let none_flag: Option<bool> = option_none_with(false);
    let ok_flag: Result<bool, bool> = result_ok_with(true, false);
    let err_flag: Result<bool, bool> = result_err_with(false, true);

    if some_flag.unwrap_or(false)
        && none_flag.is_none()
        && ok_flag.ok().unwrap_or(false)
        && err_flag.err().unwrap_or(false) {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(
        ir.contains("; Function: option_some_bool"),
        "expected generic option_some<bool> specialization\n{}",
        ir
    );
    assert!(
        ir.contains("; Function: option_none_with_bool"),
        "expected generic option_none_with<bool> specialization\n{}",
        ir
    );
    assert!(
        ir.contains("; Function: result_ok_with_bool_bool"),
        "expected generic result_ok_with<bool, bool> specialization\n{}",
        ir
    );
    assert!(
        ir.contains("; Function: result_err_with_bool_bool"),
        "expected generic result_err_with<bool, bool> specialization\n{}",
        ir
    );
}

#[test]
fn stdlib_surface_bool_option_result_convenience_constructors_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let some_flag: Option<bool> = option_some_bool(true);
    let none_flag: Option<bool> = option_none_bool();
    let ok_flag: Result<bool, i64> = result_ok_bool(true);
    let err_flag: Result<bool, i64> = result_err_bool(7);

    if some_flag.unwrap_or(false)
        && none_flag.is_none()
        && ok_flag.ok().unwrap_or(false)
        && err_flag.err().unwrap_or(0) == 7 {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(
        ir.contains("; Function: option_some_bool"),
        "expected concrete Option<bool> constructor\n{}",
        ir
    );
    assert!(
        ir.contains("; Function: option_none_bool"),
        "expected concrete Option<bool> none constructor\n{}",
        ir
    );
    assert!(
        ir.contains("; Function: result_ok_bool"),
        "expected concrete Result<bool, i64> ok constructor\n{}",
        ir
    );
    assert!(
        ir.contains("; Function: result_err_bool"),
        "expected concrete Result<bool, i64> err constructor\n{}",
        ir
    );
}

#[test]
fn stdlib_surface_bool_option_result_unwrap_and_expect_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let option_value = option_some_bool(true).unwrap();
    let expected_option = option_some_bool(true).expect("option bool ok");
    let result_value = result_ok_bool(false).unwrap();
    let expected_result = result_ok_bool(true).expect("result bool ok");

    if option_value && expected_option && !result_value && expected_result {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(
        ir.contains("; Function: Option_bool_unwrap"),
        "expected Option<bool>.unwrap specialization\n{}",
        ir
    );
    assert!(
        ir.contains("; Function: Option_bool_expect"),
        "expected Option<bool>.expect specialization\n{}",
        ir
    );
    assert!(
        ir.contains("; Function: Result_bool_i64_unwrap"),
        "expected Result<bool, i64>.unwrap specialization\n{}",
        ir
    );
    assert!(
        ir.contains("; Function: Result_bool_i64_expect"),
        "expected Result<bool, i64>.expect specialization\n{}",
        ir
    );
}

#[test]
fn stdlib_surface_mixed_option_instantiations_emit_distinct_struct_types() {
    let ir = compile_with_stdlib(
        r#"
def option_flag(opt: Option<bool>) -> bool {
    opt.unwrap_or(false)
}

def option_sum(opt: Option<i64>) -> i64 {
    opt.unwrap_or(0)
}

def main() -> i64 {
    let bool_opt: Option<bool> = Option { is_some: true, value: true };
    let int_opt: Option<i64> = Option { is_some: true, value: 7 };

    if option_flag(bool_opt) {
        option_sum(int_opt)
    } else {
        0
    }
}
"#,
    );

    assert!(
        ir.contains("%Option_bool = type { i1, i1 }"),
        "expected distinct LLVM struct for Option<bool>\n{}",
        ir
    );
    assert!(
        ir.contains("%Option_i64 = type { i1, i64 }"),
        "expected distinct LLVM struct for Option<i64>\n{}",
        ir
    );
}

#[test]
fn stdlib_surface_option_and_result_remain_tagged_struct_layouts() {
    let ir = compile_with_stdlib(
        r#"
def option_flag(opt: Option<bool>) -> bool {
    opt.unwrap_or(false)
}

def option_sum(opt: Option<i64>) -> i64 {
    opt.unwrap_or(0)
}

def result_flag(res: Result<bool, i64>) -> bool {
    res.unwrap_or(false)
}

def result_sum(res: Result<i64, bool>) -> i64 {
    res.unwrap_or(0)
}

def main() -> i64 {
    let bool_opt: Option<bool> = Option { is_some: true, value: true };
    let int_opt: Option<i64> = Option { is_some: true, value: 7 };
    let ok_result: Result<bool, i64> = Result { is_ok: true, value: true, error: 6 };
    let err_result: Result<i64, bool> = Result { is_ok: false, value: 9, error: true };

    if option_flag(bool_opt) && result_flag(ok_result) {
        option_sum(int_opt) + result_sum(err_result)
    } else {
        0
    }
}
"#,
    );

    assert!(
        ir.contains("%Option_bool = type { i1, i1 }"),
        "expected Option<bool> tagged-struct layout\n{}",
        ir
    );
    assert!(
        ir.contains("%Option_i64 = type { i1, i64 }"),
        "expected Option<i64> tagged-struct layout\n{}",
        ir
    );
    assert!(
        ir.contains("%Result_bool_i64 = type { i1, i1, i64 }"),
        "expected Result<bool, i64> tagged-struct layout\n{}",
        ir
    );
    assert!(
        ir.contains("%Result_i64_bool = type { i1, i64, i1 }"),
        "expected Result<i64, bool> tagged-struct layout\n{}",
        ir
    );
}
