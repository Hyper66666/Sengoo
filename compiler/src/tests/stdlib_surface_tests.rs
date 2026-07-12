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
    compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "collections.sg",
        ],
        program,
    )
}

fn compile_with_stdlib_modules(modules: &[&str], program: &str) -> String {
    let source = format!("{}\n\n{}", load_stdlib_surface(modules), program);
    compile_to_ir(&source)
        .unwrap_or_else(|err| panic!("stdlib surface program should compile: {err}"))
}

fn llvm_function_section<'a>(ir: &'a str, function_header: &str) -> &'a str {
    let start = ir
        .find(function_header)
        .unwrap_or_else(|| panic!("missing LLVM function header `{function_header}`\n{ir}"));
    let rest = &ir[start..];
    let next = rest.find("\n; Function: ").unwrap_or(rest.len());
    &rest[..next]
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
fn string_module_hasher_protocol_satisfies_hash_bridge() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
struct Key {
    id: i64,
}

impl Hash for Key {
    def hash_into(&self, h: &mut Hasher) {
        h.write_i64(self.id);
        h.write_bool(true);
        h.write_str("key");
    }
}

def use_hash<T: Hash>(value: T) -> i64 {
    value.hash()
}

def main() -> i64 {
    use_hash(Key { id: 7 })
}
"#,
    );

    assert!(
        ir.contains("; Function: Key_Hash_hash")
            && ir.contains("call void @Key_Hash_hash_into")
            && ir.contains("call %Hasher @hasher_new"),
        "expected stdlib Hasher protocol bridge in IR\n{ir}"
    );
}

#[test]
fn string_module_imports_and_runs_str_len() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
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
fn string_module_imports_owned_string_helpers() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def main() -> i64 {
    let built = string_from_str("hi");
    if built.is_ok == false {
        return 0;
    }
    let owned = built.value;
    if owned.len() != 2 {
        return 0;
    }
    let moved = owned;
    moved.drop();
    1
}
"#,
    );

    assert!(ir.contains("sengoo_string_from_str_copy"));
    assert!(ir.contains("sengoo_string_len"));
    assert!(ir.contains("sengoo_string_free_status"));
}

#[test]
fn string_module_imports_owned_string_push_char() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def main() -> i64 {
    let built = string_from_str("hi").unwrap_or(String { handle: 0 });
    let pushed = built.push_char('!');
    if pushed.is_ok {
        built.len()
    } else {
        0
    }
}
"#,
    );

    assert!(
        ir.contains("declare i64 @sengoo_string_push_char_status(i64, i32)"),
        "push_char extern should preserve char as an i32 C ABI scalar\n{ir}"
    );
}

#[test]
fn string_module_imports_owned_string_comparison_helpers() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def main() -> i64 {
    let a = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let b = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let c = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let d = string_from_str("beta").unwrap_or(String { handle: 0 });
    if a.eq(b) && c.lt(d) {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_string_eq"));
    assert!(ir.contains("sengoo_string_compare"));
}

#[test]
fn string_module_lowers_owned_string_comparison_operators() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def main() -> i64 {
    let a = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let b = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let c = string_from_str("beta").unwrap_or(String { handle: 0 });
    if a == b && a < c && c >= b {
        1
    } else {
        0
    }
}
"#,
    );

    let main = llvm_function_section(&ir, "define i64 @main");
    assert!(ir.contains("sengoo_string_eq"));
    assert!(ir.contains("sengoo_string_compare"));
    assert!(
        !main.contains("icmp eq %String") && !main.contains("icmp slt %String"),
        "owned String operators must lower through runtime comparison helpers\n{main}"
    );
}

#[test]
fn string_and_str_satisfy_comparison_trait_bounds() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def accepts_comparison<T: PartialEq + Eq + PartialOrd + Ord>(value: T) -> i64 {
    1
}

def main() -> i64 {
    let owned = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let order_bonus = if "alpha" < "beta" {
        1
    } else {
        0
    };
    accepts_comparison(owned) + accepts_comparison("borrowed") + order_bonus
}
"#,
    );

    assert!(
        ir.contains("@accepts_comparison_String"),
        "owned String should satisfy comparison trait bounds through stdlib marker impls\n{ir}"
    );
    assert!(
        ir.contains("@accepts_comparison_ref_str"),
        "&str should satisfy comparison trait bounds through compiler-known impls\n{ir}"
    );
    assert!(
        ir.contains("@sengoo_str_compare"),
        "&str ordering operators should lower through the runtime compare helper\n{ir}"
    );
}

#[test]
fn string_module_rejects_owned_string_arithmetic_operators() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib_surface(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        r#"
def main() -> i64 {
    let a = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let b = string_from_str("beta").unwrap_or(String { handle: 0 });
    let c = a - b;
    c.len()
}
"#,
    );
    let err = compile_to_ir(&source).expect_err("String - String must not type-check");
    let err = format!("{err:?}");
    assert!(
        err.contains("TypeMismatch") || err.contains("类型不匹配") || err.contains("type"),
        "unexpected diagnostic: {err}"
    );
}

#[test]
fn string_module_imports_utf8_slice_helpers() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def main() -> i64 {
    let borrowed = str_get("hello", 1, 4).unwrap_or(String { handle: 0 });
    let owned = string_from_str("hello").unwrap_or(String { handle: 0 });
    let part = owned.get(1, 4).unwrap_or(String { handle: 0 });
    borrowed.len() + part.len()
}
"#,
    );

    assert!(ir.contains("sengoo_str_slice_copy"));
    assert!(ir.contains("sengoo_string_slice_status"));
}

#[test]
fn string_range_index_lowers_to_infallible_owned_slice() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def main() -> i64 {
    let text = string_from_str("hello").unwrap_or(String { handle: 0 });
    let part = text[1..4];
    part.len()
}
"#,
    );

    assert!(ir.contains("sengoo_string_slice_status"));
    assert!(ir.contains("sengoo_panic_result_unwrap_i64"));
}

#[test]
fn str_range_index_lowers_to_infallible_owned_slice() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def main() -> i64 {
    let part = "hello"[1..4];
    part.len()
}
"#,
    );

    assert!(ir.contains("sengoo_str_slice_copy"));
    assert!(ir.contains("sengoo_panic_result_unwrap_i64"));
}

#[test]
fn string_module_imports_string_bytes_and_chars_iterators() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def main() -> i64 {
    let bytes_text = string_from_str("hi").unwrap_or(String { handle: 0 });
    let chars_text = string_from_str("é").unwrap_or(String { handle: 0 });
    let mut bytes = bytes_text.bytes();
    let mut chars = chars_text.chars();
    let first_char = chars.next().unwrap_or('\0');
    let mut round_trip = string_new();
    round_trip.push_char(first_char);
    bytes.next().unwrap_or(0) + round_trip.len()
}
"#,
    );

    assert!(ir.contains("sengoo_string_bytes_iter_new"));
    assert!(ir.contains("sengoo_string_chars_iter_new"));
    assert!(ir.contains("sengoo_string_chars_iter_next_char"));
}

#[test]
fn string_chars_iterator_still_exposes_codepoint_next() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def main() -> i64 {
    let text = string_from_str("é").unwrap_or(String { handle: 0 });
    let mut chars = text.chars();
    chars.next_codepoint().unwrap_or(0)
}
"#,
    );

    assert!(ir.contains("sengoo_string_chars_iter_next_or_default"));
}

#[test]
fn string_module_imports_string_split_iterator() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def main() -> i64 {
    let text = string_from_str("a,b,").unwrap_or(String { handle: 0 });
    let mut parts = text.split(",");
    let first = parts.next().unwrap_or(String { handle: 0 });
    let second = parts.next().unwrap_or(String { handle: 0 });
    let third = parts.next().unwrap_or(String { handle: 0 });
    first.len() + second.len() + third.len()
}
"#,
    );

    assert!(ir.contains("sengoo_string_split_iter_new"));
    assert!(ir.contains("sengoo_string_split_iter_next"));
}

#[test]
fn string_iterators_satisfy_iterator_associated_item_bounds() {
    let ir = compile_with_stdlib(
        r#"
def select_item<I: Iterator>(iter: I, value: I::Item) -> I::Item {
    value
}

def main() -> i64 {
    let text = string_from_str("az").value;
    let bytes = text.bytes();
    let chars = text.chars();
    let parts = text.split(",");
    let b = select_item(bytes, 65);
    let c = select_item(chars, 'A');
    let s = select_item(parts, string_new());
    let mut round_trip = string_new();
    round_trip.push_char(c);
    b + round_trip.len() + s.len()
}
"#,
    );

    assert!(
        ir.contains("select_item_StringBytesIter"),
        "expected StringBytesIter to satisfy Iterator<Item = i64>, got:\n{ir}"
    );
    assert!(
        ir.contains("select_item_StringCharsIter"),
        "expected StringCharsIter to satisfy Iterator<Item = char>, got:\n{ir}"
    );
    assert!(
        ir.contains("select_item_StringSplitIter"),
        "expected StringSplitIter to satisfy Iterator<Item = String>, got:\n{ir}"
    );
}

#[test]
fn string_module_allows_owned_string_plus_str() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def main() -> i64 {
    let base = string_from_str("hi").unwrap_or(String { handle: 0 });
    let joined = base + "!";
    joined.len()
}
"#,
    );

    assert!(ir.contains("sengoo_string_concat_str_status"));
    assert!(ir.contains("sengoo_panic_result_unwrap_i64"));
}

#[test]
fn display_impl_can_be_printed_through_builtin_prints() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
struct Tag {
    id: i64,
}

impl Display for Tag {
    def to_string(&self) -> String {
        string_from_str("Tag").value
    }
}

def main() -> i64 {
    let out = Tag { id: 1 };
    let err = Tag { id: 2 };
    print(out);
    eprintln(err);
    0
}
"#,
    );

    assert!(ir.contains("Tag_Display_to_string"));
    assert!(ir.contains("sengoo_print_string"));
    assert!(ir.contains("sengoo_eprint_string"));
}

#[test]
fn stdlib_owned_handles_auto_drop_without_manual_release() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "status.sg",
            "string.sg",
            "collections.sg",
            "json.sg",
            "process.sg",
            "net.sg",
        ],
        r##"
def main() -> i64 {
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    let vec = vec_new_i64();
    let strings = vec_new_string();
    let doc = json_parse("{}").unwrap_or(JsonDoc { handle: 0 });
    let command = process_command("sengoo-missing-owned-drop").unwrap_or(ProcessCommand { handle: 0 });
    let output = ProcessOutput { handle: 0 };
    let process = ProcessHandle { handle: 0 };
    let stream = TcpStream { handle: 0 };
    let socket = UdpSocket { handle: 0 };
    let client = HttpClient { handle: 0 };
    let server = HttpServer { handle: 0 };
    let request = HttpServerRequest { handle: 0 };

    buffer.len() + vec.len() + strings.len() + doc.root().node_id + command.handle
        + output.handle + process.handle + stream.handle + socket.handle
        + client.handle + server.handle + request.handle
}
"##,
    );

    for symbol in [
        "Buffer_Drop_drop",
        "Vec_i64_Drop_drop",
        "Vec_String_Drop_drop",
        "JsonDoc_Drop_drop",
        "ProcessCommand_Drop_drop",
        "ProcessOutput_Drop_drop",
        "ProcessHandle_Drop_drop",
        "TcpStream_Drop_drop",
        "UdpSocket_Drop_drop",
        "HttpClient_Drop_drop",
        "HttpServer_Drop_drop",
        "HttpServerRequest_Drop_drop",
    ] {
        assert!(
            ir.contains(symbol),
            "expected stdlib owning handle auto-drop symbol {symbol}\n{ir}"
        );
    }
}

#[test]
fn stdlib_owned_result_unwrap_or_moves_value_without_dropping_it_first() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "status.sg"],
        r#"
def main() -> i64 {
    let buffer = ffi_buffer_new(8).unwrap_or(Buffer { handle: 0 });
    buffer.len()
}
"#,
    );
    let unwrap_or = llvm_function_section(&ir, "define %Buffer @Result_Buffer_i64_unwrap_or");

    assert!(
        !unwrap_or.contains("Buffer_Drop_drop"),
        "owned Result.unwrap_or must move its selected value out before drop glue runs\n{unwrap_or}"
    );
}

#[test]
fn stdlib_rc_shared_ownership_compiles_and_auto_drops() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "collections.sg",
        ],
        r#"
def main() -> i64 {
    let first = rc_new_i64(21);
    let second = first.clone();
    first.strong_count() + second.get()
}
"#,
    );

    assert!(
        ir.contains("sengoo_rc_clone"),
        "Rc clone should lower to runtime refcount increment\n{ir}"
    );
    assert!(
        ir.contains("Rc_i64_Drop_drop"),
        "Rc<i64> locals should auto-drop through Drop glue\n{ir}"
    );
    assert!(
        ir.contains("sengoo_rc_drop"),
        "Rc Drop impl should call the runtime decrement/free helper\n{ir}"
    );

    let clone_section = llvm_function_section(&ir, "; Function: Rc_i64_clone");
    assert!(
        !clone_section.contains("@Rc_i64_Drop_drop"),
        "Rc::clone has a borrowed receiver and must not auto-drop its receiver parameter\n{clone_section}"
    );
    let count_section = llvm_function_section(&ir, "; Function: Rc_i64_strong_count");
    assert!(
        !count_section.contains("@Rc_i64_Drop_drop"),
        "Rc::strong_count has a borrowed receiver and must not auto-drop its receiver parameter\n{count_section}"
    );
}

#[test]
fn compiler_generated_payload_callback_drops_owned_struct_fields() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "collections.sg",
        ],
        r#"
struct Payload {
    text: String,
}

def main() -> i64 {
    let text = string_from_str("callback").unwrap_or(String { handle: 0 });
    let shared = rc_new(Payload { text: text });
    shared.strong_count()
}
"#,
    );

    assert!(
        ir.contains("define void @__sengoo_rc_drop_Rc_Payload(i8*"),
        "compiler should synthesize an erased payload drop callback\n{ir}"
    );
    let thunk = llvm_function_section(&ir, "define void @__sengoo_rc_drop_Rc_Payload");
    assert!(
        thunk.contains("@Payload_Drop_drop") || thunk.contains("@String_Drop_drop"),
        "payload callback must execute owned-field drop glue\n{thunk}"
    );
    assert!(
        ir.contains("@__sengoo_rc_drop_Rc_Payload") && ir.contains("call i64 @sengoo_rc_new_copy"),
        "the generated callback must cross the C runtime ABI\n{ir}"
    );
}

#[test]
fn stdlib_rc_string_shared_ownership_clones_and_auto_drops() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "collections.sg",
        ],
        r#"
def main() -> i64 {
    let text = string_from_str("hello").unwrap_or(String { handle: 0 });
    let first = rc_new_string(text);
    let second = first.clone();
    let copy = second.get();
    first.strong_count() + copy.len()
}

"#,
    );

    assert!(
        ir.contains("sengoo_rc_new_string"),
        "Rc<String> constructor should transfer an owned String handle into the Rc runtime\n{ir}"
    );
    assert!(
        ir.contains("Rc_String_Drop_drop"),
        "Rc<String> locals should auto-drop through Drop glue\n{ir}"
    );
    assert!(
        ir.contains("sengoo_rc_drop"),
        "Rc<String> Drop impl should call the runtime decrement/free helper\n{ir}"
    );
}

#[test]
fn stdlib_vec_methods_borrow_receiver_without_early_drop() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "collections.sg",
        ],
        r#"
def main() -> i64 {
    let values = vec_new_i64();
    values.push(1);
    let deque = vecdeque_new_i64();
    deque.push_back(2);
    let map = hashmap_new_i64_i64();
    map.insert(1, 3);
    let ids = hashset_new_i64();
    ids.insert(4);
    let names = hashset_new_string();
    names.insert("alpha");
    values.len() + deque.len() + map.len() + ids.len() + names.len()
}
"#,
    );
    for method in ["Vec_i64_push", "Vec_i64_len"] {
        let section = llvm_function_section(&ir, &format!("; Function: {method}"));
        assert!(
            !section.contains("@Vec_i64_Drop_drop"),
            "{method} borrows its receiver and must not free the vector handle:\n{section}"
        );
    }
    let deque_push = llvm_function_section(&ir, "; Function: VecDeque_i64_push_back");
    assert!(
        !deque_push.contains("@VecDeque_i64_Drop_drop"),
        "VecDeque::push_back must not drop its borrowed receiver:\n{deque_push}"
    );
    assert!(
        ir.contains("VecDeque_i64_Drop_drop"),
        "caller-owned VecDeque<i64> should receive automatic Drop glue:\n{ir}"
    );
    for method in ["HashMap_i64_i64_insert", "HashMap_i64_i64_len"] {
        let section = llvm_function_section(&ir, &format!("; Function: {method}"));
        assert!(
            !section.contains("@HashMap_i64_i64_Drop_drop"),
            "{method} borrows its receiver and must not free the map handle:\n{section}"
        );
    }
    for (method, drop_name) in [
        ("HashSet_i64_insert", "HashSet_i64_Drop_drop"),
        ("HashSet_String_insert", "HashSet_String_Drop_drop"),
    ] {
        let section = llvm_function_section(&ir, &format!("; Function: {method}"));
        assert!(
            !section.contains(drop_name),
            "{method} borrows its receiver and must not free the set handle:\n{section}"
        );
        assert!(
            ir.contains(drop_name),
            "caller-owned set should receive automatic Drop glue `{drop_name}`:\n{ir}"
        );
    }
}

#[test]
fn stdlib_rc_value_trait_allows_generic_shared_ownership_construction() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "collections.sg",
        ],
        r#"
def share<T: RcValue>(value: T) -> Rc<T> {
    value.rc()
}

def main() -> i64 {
    let first = share(21);
    let second = first.clone();
    let flag = share(true);
    if flag.get() {
        second.get()
    } else {
        0
    }
}
"#,
    );

    assert!(
        ir.contains("; Function: share_i64"),
        "expected generic share<i64> specialization\n{ir}"
    );
    assert!(
        ir.contains("; Function: share_bool"),
        "expected generic share<bool> specialization\n{ir}"
    );
    assert!(
        ir.contains("sengoo_rc_new_i64") && ir.contains("sengoo_rc_new_bool"),
        "expected RcValue impls to dispatch through concrete runtime constructors\n{ir}"
    );
}

#[test]
fn stdlib_rc_clone_uses_one_generic_handle_representation() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "collections.sg",
        ],
        r#"
def clone_shared<T>(value: &Rc<T>) -> Rc<T> {
    value.clone()
}

def main() -> i64 {
    let first = rc_new_i64(21);
    let second = clone_shared(&first);
    first.strong_count() + second.get()
}
"#,
    );

    assert!(
        ir.contains("; Function: clone_shared_i64"),
        "expected generic Rc<T> clone helper specialization\n{ir}"
    );
    assert!(
        ir.contains("sengoo_rc_clone"),
        "generic Rc<T>::clone should increment the shared runtime control block\n{ir}"
    );
}

#[test]
fn string_module_imports_search_helpers() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def main() -> i64 {
    let text = "sengoo";
    let score = if str_contains(text, "goo")
        && str_starts_with(text, "sen")
        && str_ends_with(text, "goo")
        && str_index_of(text, "go") == 3 {
        1
    } else {
        0
    };
    score
}
"#,
    );

    assert!(ir.contains("sengoo_str_contains"));
    assert!(ir.contains("sengoo_str_starts_with"));
    assert!(ir.contains("sengoo_str_ends_with"));
    assert!(ir.contains("sengoo_str_index_of"));
}

#[test]
fn string_module_imports_trim_and_ascii_case_helpers() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "string.sg"],
        r#"
def main() -> i64 {
    let trimmed = str_trim("  sengoo\n").unwrap_or(String { handle: 0 });
    let upper = str_to_ascii_upper("senGoo").unwrap_or(String { handle: 0 });
    let lower = str_to_ascii_lower("SenGOO").unwrap_or(String { handle: 0 });
    trimmed.len() + upper.len() + lower.len()
}
"#,
    );

    assert!(ir.contains("sengoo_str_trim"));
    assert!(ir.contains("sengoo_str_to_ascii_upper"));
    assert!(ir.contains("sengoo_str_to_ascii_lower"));
    assert!(ir.contains("String_Drop_drop"));
}

#[test]
fn strconv_module_imports_i64_parse_and_format_helpers() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "status.sg",
            "strconv.sg",
        ],
        r#"
def main() -> i64 {
    let buffer = ffi_buffer_new(32).unwrap_or(Buffer { handle: 0 });
    let parsed = strconv_parse_i64(" -42\n").unwrap_or(0);
    let formatted = strconv_format_i64(parsed + 50, buffer).unwrap_or(0);
    let parsed_f64 = strconv_parse_f64(" 3.25\n").unwrap_or(0.0);
    let formatted_f64 = strconv_format_f64(parsed_f64, 2, buffer).unwrap_or(0);
    let parsed_f32 = strconv_parse_f32(" 2.5\n").unwrap_or(0.0f32);
    let formatted_f32 = strconv_format_f32(parsed_f32, 1, buffer).unwrap_or(0);
    let source = ffi_buffer_from_bytes("123").unwrap_or(Buffer { handle: 0 });
    let parsed_buffer = strconv_parse_i64_buffer(source, 3).unwrap_or(0);
    let parsed_f64_buffer = strconv_parse_f64_buffer(source, 3).unwrap_or(0.0);
    let parsed_f32_buffer = strconv_parse_f32_buffer(source, 3).unwrap_or(0.0f32);
    let invalid = strconv_parse_i64("12x").unwrap_or(7);
    source.free();
    buffer.free();
    formatted + formatted_f64 + formatted_f32 + parsed_buffer
        + (parsed_f64_buffer as i64) + (parsed_f32_buffer as i64) + invalid
}
"#,
    );

    assert!(ir.contains("sengoo_strconv_last_error_code"));
    assert!(ir.contains("sengoo_strconv_parse_i64"));
    assert!(ir.contains("sengoo_strconv_format_i64"));
    assert!(ir.contains("sengoo_strconv_parse_f64"));
    assert!(ir.contains("sengoo_strconv_format_f64"));
    assert!(ir.contains("fptrunc double"));
    assert!(ir.contains("fpext float"));
}

#[test]
fn file_module_imports_copy_and_move_helpers() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "status.sg", "file.sg"],
        r#"
def main() -> i64 {
    let source = "target/sengoo-stdlib-file-surface-source.txt";
    let copy = "target/sengoo-stdlib-file-surface-copy.txt";
    let moved = "target/sengoo-stdlib-file-surface-moved.txt";
    let wrote = file_write_str(source, "abc").unwrap_or(0);
    let copied = file_copy(source, copy, false).unwrap_or(0);
    let moved_ok = file_move(copy, moved, false).unwrap_or(false);
    file_remove(source);
    file_remove(moved);

    if wrote == 3 && copied == 3 && moved_ok {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_file_copy"));
    assert!(ir.contains("sengoo_file_move"));
}

#[test]
fn env_module_imports_process_and_variable_helpers() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "status.sg", "env.sg"],
        r#"
def main() -> i64 {
    let buffer = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
    let copied = env_var_copy("SENGOO_TEST_ENV", buffer).unwrap_or(0);
    let present = env_has_var("SENGOO_TEST_ENV");
    let windows = env_is_windows();
    let unix = env_is_unix();
    let code = env_exit_code(false, 7);
    buffer.free();

    if copied >= 0 && (windows || unix) && code == 7 {
        if present { 1 } else { 0 }
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_env_var_len"));
    assert!(ir.contains("sengoo_env_var_copy"));
    assert!(ir.contains("sengoo_env_is_windows"));
    assert!(ir.contains("sengoo_env_is_unix"));
    assert!(ir.contains("env_exit_code"));
}

#[test]
fn time_module_imports_clock_and_sleep_helpers() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "status.sg", "time.sg"],
        r#"
def main() -> i64 {
    let before = time_unix_ms();
    time_sleep_ms(0);
    let after = time_unix_ms();

    if after >= before && time_unix_seconds() >= 0 {
        time_elapsed_ms(before, after)
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_time_unix_ms"));
    assert!(ir.contains("sengoo_time_unix_seconds"));
    assert!(ir.contains("sengoo_time_sleep_ms"));
    assert!(ir.contains("time_elapsed_ms"));
}

#[test]
fn random_module_imports_seeded_random_helpers() {
    let ir = compile_with_stdlib_modules(
        &["random.sg"],
        r#"
def main() -> i64 {
    random_seed(123);
    let value = random_i64();
    let bounded = random_range_i64(10, 20);
    let coin = random_bool();

    if value >= 0 && bounded >= 10 && bounded < 20 && (coin || !coin) {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_random_seed"));
    assert!(ir.contains("sengoo_random_i64"));
    assert!(ir.contains("sengoo_random_range_i64"));
    assert!(ir.contains("sengoo_random_bool"));
}

#[test]
fn path_module_imports_cross_platform_helpers() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "status.sg",
            "string.sg",
            "path.sg",
        ],
        r#"
def main() -> i64 {
    let joined = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
    let parent = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
    let file_name = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
    let stem = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
    let extension = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
    let normalized = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });

    let separator = path_separator();
    let joined_len = path_join("alpha", "beta.sg", joined).unwrap_or(0);
    let parent_len = path_parent("alpha/beta.sg", parent).unwrap_or(0);
    let file_name_len = path_file_name("alpha/beta.sg", file_name).unwrap_or(0);
    let stem_len = path_stem("alpha/beta.sg", stem).unwrap_or(0);
    let extension_len = path_extension("alpha/beta.sg", extension).unwrap_or(0);
    let normalized_len = path_normalize("alpha//./beta/../gamma.sg", normalized).unwrap_or(0);
    let absolute = path_is_absolute("/alpha") || path_is_absolute("C:/alpha") || path_is_absolute("\\\\server\\share");

    joined.free();
    parent.free();
    file_name.free();
    stem.free();
    extension.free();
    normalized.free();

    if separator > 0
        && joined_len > 0
        && parent_len > 0
        && file_name_len > 0
        && stem_len > 0
        && extension_len > 0
        && normalized_len > 0
        && absolute {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_path_separator"));
    assert!(ir.contains("sengoo_path_is_absolute"));
    assert!(ir.contains("sengoo_path_join"));
    assert!(ir.contains("sengoo_path_parent"));
    assert!(ir.contains("sengoo_path_file_name"));
    assert!(ir.contains("sengoo_path_stem"));
    assert!(ir.contains("sengoo_path_extension"));
    assert!(ir.contains("sengoo_path_normalize"));
}

#[test]
fn process_module_imports_metadata_helpers() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "status.sg",
            "process.sg",
        ],
        r#"
def main() -> i64 {
    let len = process_current_dir_len().unwrap_or(0);
    let buffer = ffi_buffer_new(len + 1).unwrap_or(Buffer { handle: 0 });
    let copied = process_current_dir_copy(buffer).unwrap_or(0);
    let pid = process_id();
    let ok_code = process_exit_code(true, 9);
    let err_code = process_exit_code(false, 9);
    let missing_0 = process_run("sengoo-missing-process").is_err();
    let missing_1 = process_run_1("sengoo-missing-process", "a").is_err();
    let missing_2 = process_run_2("sengoo-missing-process", "a", "b").is_err();
    let missing_3 = process_run_3("sengoo-missing-process", "a", "b", "c").is_err();
    let stale_handle = ProcessHandle { handle: 0 };
    let wait_cancellable_rejected = stale_handle.wait_cancellable(1).is_err();
    buffer.free();

    if len > 0 && copied == len && pid > 0 && ok_code == 0 && err_code == 9 && missing_0 && missing_1 && missing_2 && missing_3 && wait_cancellable_rejected {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_process_id"));
    assert!(ir.contains("sengoo_process_current_dir_len"));
    assert!(ir.contains("sengoo_process_current_dir_copy"));
    assert!(ir.contains("sengoo_process_run"));
    assert!(ir.contains("sengoo_process_handle_wait_cancellable"));
    assert!(ir.contains("process_exit_code"));
}

#[test]
fn dir_module_imports_directory_helpers() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "status.sg",
            "string.sg",
            "dir.sg",
        ],
        r#"
def main() -> i64 {
    let root = "target/sengoo-stdlib-dir-surface";
    let nested = "target/sengoo-stdlib-dir-surface/nested";
    let buffer = ffi_buffer_new(16).unwrap_or(Buffer { handle: 0 });
    let created = dir_create_all(nested).unwrap_or(false);
    let nested_exists = dir_exists(nested);
    let count = dir_entry_count(root).unwrap_or(0);
    let first = dir_entry_name(root, 0, buffer).unwrap_or(0);
    let removed_nested = dir_remove(nested).unwrap_or(false);
    let removed_root = dir_remove(root).unwrap_or(false);
    buffer.free();

    if created && nested_exists && count >= 1 && first >= 0 && removed_nested && removed_root {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_dir_exists"));
    assert!(ir.contains("sengoo_dir_create"));
    assert!(ir.contains("sengoo_dir_create_all"));
    assert!(ir.contains("sengoo_dir_remove"));
    assert!(ir.contains("sengoo_dir_entry_count"));
    assert!(ir.contains("sengoo_dir_entry_name"));
}

#[test]
fn file_and_dir_modules_import_metadata_and_recursive_walk_helpers() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "status.sg",
            "string.sg",
            "file.sg",
            "dir.sg",
        ],
        r#"
def main() -> i64 {
    let root = "target/sengoo-stdlib-metadata-surface";
    let child = "target/sengoo-stdlib-metadata-surface/child.txt";
    let nested = "target/sengoo-stdlib-metadata-surface/nested";
    let buffer = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });

    let made = dir_create_all(nested).unwrap_or(false);
    let wrote = file_write_str(child, "abc").unwrap_or(0);
    let kind = file_kind(child).unwrap_or(0);
    let dir_kind = file_kind(root).unwrap_or(0);
    let size = file_size(child).unwrap_or(0);
    let modified = file_modified_unix_ms(child).unwrap_or(0);
    let walk = dir_walk(root, 1).unwrap_or(DirWalk { handle: 0 });
    let first = walk.next(buffer).unwrap_or(0);
    let closed = walk.close();

    file_remove(child);
    dir_remove(nested);
    let removed = dir_remove(root).unwrap_or(false);
    buffer.free();

    if made
        && wrote == 3
        && kind == PATH_KIND_FILE()
        && dir_kind == PATH_KIND_DIR()
        && size == 3
        && modified >= 0
        && first >= 0
        && closed
        && removed {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("PATH_KIND_FILE"));
    assert!(ir.contains("PATH_KIND_DIR"));
    assert!(ir.contains("sengoo_file_kind"));
    assert!(ir.contains("sengoo_file_size"));
    assert!(ir.contains("sengoo_file_modified_unix_ms"));
    assert!(ir.contains("sengoo_dir_walk_new"));
    assert!(ir.contains("sengoo_dir_walk_next"));
    assert!(ir.contains("sengoo_dir_walk_close"));
}

#[test]
fn io_module_imports_standard_stream_helpers() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "status.sg", "io.sg"],
        r#"
def main() -> i64 {
    let buffer = ffi_buffer_new(16).unwrap_or(Buffer { handle: 0 });
    let read = io_stdin_read(buffer).unwrap_or(0);
    let line = io_stdin_read_line(buffer).unwrap_or(0);
    let out = io_stdout_write("ok").unwrap_or(0);
    let err = io_stderr_write("warn").unwrap_or(0);
    let flushed = io_stdout_flush().unwrap_or(false) && io_stderr_flush().unwrap_or(false);
    buffer.free();

    if read >= 0 && line >= 0 && out == 2 && err == 4 && flushed {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_io_stdin_read"));
    assert!(ir.contains("sengoo_io_stdin_read_line"));
    assert!(ir.contains("sengoo_io_stdout_write"));
    assert!(ir.contains("sengoo_io_stderr_write"));
    assert!(ir.contains("sengoo_io_stdout_flush"));
    assert!(ir.contains("sengoo_io_stderr_flush"));
}

#[test]
fn args_module_imports_argument_helpers_and_emits_opt_in_entry_wrapper() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "status.sg", "args.sg"],
        r#"
def main() -> i64 {
    let first_len = arg_len(0).unwrap_or(0);
    let buffer = ffi_buffer_new(first_len + 1).unwrap_or(Buffer { handle: 0 });
    let copied = arg_copy(0, buffer).unwrap_or(0);
    let exists = arg_exists(0);
    buffer.free();

    if args_len() >= 0 && exists && copied == first_len {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_args_len"));
    assert!(ir.contains("sengoo_arg_len"));
    assert!(ir.contains("sengoo_arg_copy"));
    assert!(ir.contains("declare void @sengoo_args_init(i64, i64)"));
    assert!(ir.contains("declare i64 @sengoo_args_len()"));
    assert!(ir.contains("declare i64 @sengoo_arg_len(i64)"));
    assert!(ir.contains("declare i64 @sengoo_arg_copy(i64, i64, i64)"));
    assert!(ir.contains("define i64 @sengoo_user_main()"));
    assert!(ir.contains("define i32 @main(i32 %argc, i8** %argv)"));
    assert!(ir.contains("call void @sengoo_args_init"));
    assert!(ir.contains("call i64 @sengoo_user_main()"));
}

#[test]
fn codegen_preserves_zero_argument_main_without_args_runtime() {
    let ir = compile_to_ir("def main() -> i64 {\n    0\n}\n").expect("plain main should compile");

    assert!(ir.contains("define i64 @main()"));
    assert!(!ir.contains("sengoo_args_init"));
    assert!(!ir.contains("sengoo_user_main"));
}

#[test]
fn math_module_imports_and_runs_abs_i64() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "status.sg", "math.sg"],
        r#"
def main() -> i64 {
    let widened: i64 = (7i32).into();
    let widened_i64_from_isize: i64 = (7isize).into();
    let widened_u64_from_usize: u64 = (7usize).into();
    let f = max_f64(abs_f64(-1.5), min_f64(3.0, 2.0));
    let root = sqrt_f64(9.0);
    let power = pow_f64(2.0, 3.0);
    let log_roundtrip = ln_f64(exp_f64(1.0));
    let rounded = floor_f64(2.8) + ceil_f64(2.2) + round_f64(2.6);
    let trig = sin_f64(0.0) + cos_f64(0.0) + tan_f64(0.0);
    let f32_value = max_f32(abs_f32(-1.5f32), min_f32(3.0f32, 2.0f32));
    let f32_root = sqrt_f32(9.0f32);
    let f32_power = pow_f32(2.0f32, 3.0f32);
    let f32_roundtrip = ln_f32(exp_f32(1.0f32));
    let f32_rounded = floor_f32(2.8f32) + ceil_f32(2.2f32) + round_f32(2.6f32);
    let f32_trig = sin_f32(0.0f32) + cos_f32(0.0f32) + tan_f32(0.0f32);
    let ok = is_finite_f64(f)
        && !is_nan_f64(f)
        && !is_infinite_f64(f)
        && root == 3.0
        && power == 8.0
        && log_roundtrip > 0.999
        && log_roundtrip < 1.001
        && rounded == 8.0
        && trig == 1.0
        && is_finite_f32(f32_value)
        && !is_nan_f32(f32_value)
        && !is_infinite_f32(f32_value)
        && f32_root == 3.0f32
        && f32_power == 8.0f32
        && f32_roundtrip > 0.999f32
        && f32_roundtrip < 1.001f32
        && f32_rounded == 8.0f32
        && f32_trig == 1.0f32;
    let i32_ok = checked_i64_to_i32(2147483647);
    let i32_overflow = checked_i64_to_i32(2147483648);
    let i16_ok = checked_i64_to_i16(32767);
    let i16_overflow = checked_i64_to_i16(32768);
    let i8_ok = checked_i64_to_i8(127);
    let i8_overflow = checked_i64_to_i8(128);
    let u64_ok = checked_i64_to_u64(9223372036854775807);
    let u64_negative = checked_i64_to_u64(0 - 1);
    let u32_ok = checked_i64_to_u32(4294967295);
    let u32_negative = checked_i64_to_u32(0 - 1);
    let u16_ok = checked_i64_to_u16(65535);
    let u16_overflow = checked_i64_to_u16(65536);
    let u8_ok = checked_i64_to_u8(255);
    let u8_overflow = checked_i64_to_u8(256);
    let i32_to_i16_ok = checked_i32_to_i16(32767i32);
    let i32_to_i16_overflow = checked_i32_to_i16(32768i32);
    let i32_to_i8_ok = checked_i32_to_i8(127i32);
    let i32_to_i8_overflow = checked_i32_to_i8(128i32);
    let i32_to_u64_ok = checked_i32_to_u64(2147483647i32);
    let i32_to_u64_negative = checked_i32_to_u64(-1i32);
    let i32_to_u32_ok = checked_i32_to_u32(2147483647i32);
    let i32_to_u32_negative = checked_i32_to_u32(-1i32);
    let i32_to_u16_ok = checked_i32_to_u16(65535i32);
    let i32_to_u16_overflow = checked_i32_to_u16(65536i32);
    let i32_to_u8_ok = checked_i32_to_u8(255i32);
    let i32_to_u8_overflow = checked_i32_to_u8(256i32);
    let i16_to_i8_ok = checked_i16_to_i8(127i16);
    let i16_to_i8_overflow = checked_i16_to_i8(128i16);
    let i16_to_u64_ok = checked_i16_to_u64(32767i16);
    let i16_to_u64_negative = checked_i16_to_u64(-1i16);
    let i16_to_u32_ok = checked_i16_to_u32(32767i16);
    let i16_to_u16_ok = checked_i16_to_u16(32767i16);
    let i16_to_u16_negative = checked_i16_to_u16(-1i16);
    let i16_to_u8_ok = checked_i16_to_u8(255i16);
    let i16_to_u8_overflow = checked_i16_to_u8(256i16);
    let i8_to_u64_ok = checked_i8_to_u64(127i8);
    let i8_to_u64_negative = checked_i8_to_u64(-1i8);
    let i8_to_u32_ok = checked_i8_to_u32(127i8);
    let i8_to_u16_ok = checked_i8_to_u16(127i8);
    let i8_to_u8_ok = checked_i8_to_u8(127i8);
    let i8_to_u8_negative = checked_i8_to_u8(-1i8);
    let u32_to_i64_ok = checked_u32_to_i64(4294967295u32);
    let u32_to_i32_ok = checked_u32_to_i32(2147483647u32);
    let u32_to_i32_overflow = checked_u32_to_i32(2147483648u32);
    let u32_to_i16_ok = checked_u32_to_i16(32767u32);
    let u32_to_i16_overflow = checked_u32_to_i16(32768u32);
    let u32_to_i8_ok = checked_u32_to_i8(127u32);
    let u32_to_i8_overflow = checked_u32_to_i8(128u32);
    let u32_to_u16_ok = checked_u32_to_u16(65535u32);
    let u32_to_u16_overflow = checked_u32_to_u16(65536u32);
    let u32_to_u8_ok = checked_u32_to_u8(255u32);
    let u32_to_u8_overflow = checked_u32_to_u8(256u32);
    let u16_to_i64_ok = checked_u16_to_i64(65535u16);
    let u16_to_i32_ok = checked_u16_to_i32(65535u16);
    let u16_to_i16_ok = checked_u16_to_i16(32767u16);
    let u16_to_i16_overflow = checked_u16_to_i16(32768u16);
    let u16_to_i8_ok = checked_u16_to_i8(127u16);
    let u16_to_i8_overflow = checked_u16_to_i8(128u16);
    let u16_to_u32_ok = checked_u16_to_u32(65535u16);
    let u16_to_u64_ok = checked_u16_to_u64(65535u16);
    let u16_to_u8_ok = checked_u16_to_u8(255u16);
    let u16_to_u8_overflow = checked_u16_to_u8(256u16);
    let u8_to_i64_ok = checked_u8_to_i64(255u8);
    let u8_to_i32_ok = checked_u8_to_i32(255u8);
    let u8_to_i16_ok = checked_u8_to_i16(255u8);
    let u8_to_i8_ok = checked_u8_to_i8(127u8);
    let u8_to_i8_overflow = checked_u8_to_i8(128u8);
    let u8_to_u16_ok = checked_u8_to_u16(255u8);
    let u8_to_u32_ok = checked_u8_to_u32(255u8);
    let u8_to_u64_ok = checked_u8_to_u64(255u8);
    let isize_to_i64_ok = checked_isize_to_i64(9223372036854775807isize);
    let isize_to_i32_ok = checked_isize_to_i32(2147483647isize);
    let isize_to_i32_overflow = checked_isize_to_i32(2147483648isize);
    let isize_to_i16_ok = checked_isize_to_i16(32767isize);
    let isize_to_i16_overflow = checked_isize_to_i16(32768isize);
    let isize_to_i8_ok = checked_isize_to_i8(127isize);
    let isize_to_i8_overflow = checked_isize_to_i8(128isize);
    let isize_to_u64_ok = checked_isize_to_u64(9223372036854775807isize);
    let isize_to_u64_negative = checked_isize_to_u64(-1isize);
    let isize_to_u32_ok = checked_isize_to_u32(4294967295isize);
    let isize_to_u16_ok = checked_isize_to_u16(65535isize);
    let isize_to_u16_overflow = checked_isize_to_u16(65536isize);
    let isize_to_u8_ok = checked_isize_to_u8(255isize);
    let isize_to_u8_overflow = checked_isize_to_u8(256isize);
    let usize_to_i64_ok = checked_usize_to_i64(9223372036854775807usize);
    let usize_to_i32_ok = checked_usize_to_i32(2147483647usize);
    let usize_to_i32_overflow = checked_usize_to_i32(2147483648usize);
    let usize_to_i16_ok = checked_usize_to_i16(32767usize);
    let usize_to_i16_overflow = checked_usize_to_i16(32768usize);
    let usize_to_i8_ok = checked_usize_to_i8(127usize);
    let usize_to_i8_overflow = checked_usize_to_i8(128usize);
    let usize_to_u64_ok = checked_usize_to_u64(4294967295usize);
    let usize_to_u32_ok = checked_usize_to_u32(4294967295usize);
    let usize_to_u32_overflow = checked_usize_to_u32(4294967296usize);
    let usize_to_u16_ok = checked_usize_to_u16(65535usize);
    let usize_to_u16_overflow = checked_usize_to_u16(65536usize);
    let usize_to_u8_ok = checked_usize_to_u8(255usize);
    let usize_to_u8_overflow = checked_usize_to_u8(256usize);
    let max = 9223372036854775807;
    let checked_add = 40.checked_add(2);
    let checked_add_overflow = max.checked_add(1);
    let checked_sub = 40.checked_sub(2);
    let checked_mul = 6.checked_mul(7);
    abs_i64(0 - 7)
        + widened
        + if widened_i64_from_isize == 7 { 0 } else { 1000 }
        + if widened_u64_from_usize == 7u64 { 0 } else { 1000 }
        + min_i64(4, 9)
        + max_i64(4, 9)
        + pow_i64(2, 3)
        + sign_i64(0 - 9)
        + clamp_i64(12, 0, 10)
        + gcd_i64(54, 24)
        + lcm_i64(6, 8)
        + if ok { 0 } else { 1000 }
        + if i32_ok.is_ok { 0 } else { 1000 }
        + if i32_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if i16_ok.is_ok { 0 } else { 1000 }
        + if i16_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if i8_ok.is_ok { 0 } else { 1000 }
        + if i8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if u64_ok.is_ok { 0 } else { 1000 }
        + if u64_negative.error == STATUS_INVALID_ARGUMENT() { 0 } else { 1000 }
        + if u32_ok.is_ok { 0 } else { 1000 }
        + if u32_negative.error == STATUS_INVALID_ARGUMENT() { 0 } else { 1000 }
        + if u16_ok.is_ok { 0 } else { 1000 }
        + if u16_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if u8_ok.is_ok { 0 } else { 1000 }
        + if u8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if i32_to_i16_ok.is_ok { 0 } else { 1000 }
        + if i32_to_i16_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if i32_to_i8_ok.is_ok { 0 } else { 1000 }
        + if i32_to_i8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if i32_to_u64_ok.is_ok { 0 } else { 1000 }
        + if i32_to_u64_negative.error == STATUS_INVALID_ARGUMENT() { 0 } else { 1000 }
        + if i32_to_u32_ok.is_ok { 0 } else { 1000 }
        + if i32_to_u32_negative.error == STATUS_INVALID_ARGUMENT() { 0 } else { 1000 }
        + if i32_to_u16_ok.is_ok { 0 } else { 1000 }
        + if i32_to_u16_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if i32_to_u8_ok.is_ok { 0 } else { 1000 }
        + if i32_to_u8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if i16_to_i8_ok.is_ok { 0 } else { 1000 }
        + if i16_to_i8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if i16_to_u64_ok.is_ok { 0 } else { 1000 }
        + if i16_to_u64_negative.error == STATUS_INVALID_ARGUMENT() { 0 } else { 1000 }
        + if i16_to_u32_ok.is_ok { 0 } else { 1000 }
        + if i16_to_u16_ok.is_ok { 0 } else { 1000 }
        + if i16_to_u16_negative.error == STATUS_INVALID_ARGUMENT() { 0 } else { 1000 }
        + if i16_to_u8_ok.is_ok { 0 } else { 1000 }
        + if i16_to_u8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if i8_to_u64_ok.is_ok { 0 } else { 1000 }
        + if i8_to_u64_negative.error == STATUS_INVALID_ARGUMENT() { 0 } else { 1000 }
        + if i8_to_u32_ok.is_ok { 0 } else { 1000 }
        + if i8_to_u16_ok.is_ok { 0 } else { 1000 }
        + if i8_to_u8_ok.is_ok { 0 } else { 1000 }
        + if i8_to_u8_negative.error == STATUS_INVALID_ARGUMENT() { 0 } else { 1000 }
        + if u32_to_i64_ok.is_ok { 0 } else { 1000 }
        + if u32_to_i32_ok.is_ok { 0 } else { 1000 }
        + if u32_to_i32_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if u32_to_i16_ok.is_ok { 0 } else { 1000 }
        + if u32_to_i16_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if u32_to_i8_ok.is_ok { 0 } else { 1000 }
        + if u32_to_i8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if u32_to_u16_ok.is_ok { 0 } else { 1000 }
        + if u32_to_u16_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if u32_to_u8_ok.is_ok { 0 } else { 1000 }
        + if u32_to_u8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if u16_to_i64_ok.is_ok { 0 } else { 1000 }
        + if u16_to_i32_ok.is_ok { 0 } else { 1000 }
        + if u16_to_i16_ok.is_ok { 0 } else { 1000 }
        + if u16_to_i16_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if u16_to_i8_ok.is_ok { 0 } else { 1000 }
        + if u16_to_i8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if u16_to_u32_ok.is_ok { 0 } else { 1000 }
        + if u16_to_u64_ok.is_ok { 0 } else { 1000 }
        + if u16_to_u8_ok.is_ok { 0 } else { 1000 }
        + if u16_to_u8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if u8_to_i64_ok.is_ok { 0 } else { 1000 }
        + if u8_to_i32_ok.is_ok { 0 } else { 1000 }
        + if u8_to_i16_ok.is_ok { 0 } else { 1000 }
        + if u8_to_i8_ok.is_ok { 0 } else { 1000 }
        + if u8_to_i8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if u8_to_u16_ok.is_ok { 0 } else { 1000 }
        + if u8_to_u32_ok.is_ok { 0 } else { 1000 }
        + if u8_to_u64_ok.is_ok { 0 } else { 1000 }
        + if isize_to_i64_ok.is_ok { 0 } else { 1000 }
        + if isize_to_i32_ok.is_ok { 0 } else { 1000 }
        + if isize_to_i32_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if isize_to_i16_ok.is_ok { 0 } else { 1000 }
        + if isize_to_i16_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if isize_to_i8_ok.is_ok { 0 } else { 1000 }
        + if isize_to_i8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if isize_to_u64_ok.is_ok { 0 } else { 1000 }
        + if isize_to_u64_negative.error == STATUS_INVALID_ARGUMENT() { 0 } else { 1000 }
        + if isize_to_u32_ok.is_ok { 0 } else { 1000 }
        + if isize_to_u16_ok.is_ok { 0 } else { 1000 }
        + if isize_to_u16_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if isize_to_u8_ok.is_ok { 0 } else { 1000 }
        + if isize_to_u8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if usize_to_i64_ok.is_ok { 0 } else { 1000 }
        + if usize_to_i32_ok.is_ok { 0 } else { 1000 }
        + if usize_to_i32_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if usize_to_i16_ok.is_ok { 0 } else { 1000 }
        + if usize_to_i16_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if usize_to_i8_ok.is_ok { 0 } else { 1000 }
        + if usize_to_i8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if usize_to_u64_ok.is_ok { 0 } else { 1000 }
        + if usize_to_u32_ok.is_ok { 0 } else { 1000 }
        + if usize_to_u32_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if usize_to_u16_ok.is_ok { 0 } else { 1000 }
        + if usize_to_u16_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if usize_to_u8_ok.is_ok { 0 } else { 1000 }
        + if usize_to_u8_overflow.error == STATUS_OVERFLOW() { 0 } else { 1000 }
        + if checked_add.unwrap_or(0) == 42 { 0 } else { 1000 }
        + if checked_add_overflow.is_none() { 0 } else { 1000 }
        + if checked_sub.unwrap_or(0) == 38 { 0 } else { 1000 }
        + if checked_mul.unwrap_or(0) == 42 { 0 } else { 1000 }
        + if max.saturating_add(1) == max { 0 } else { 1000 }
        + if max.wrapping_add(1) < 0 { 0 } else { 1000 }
}
"#,
    );

    assert!(ir.contains("abs_i64"));
    assert!(ir.contains("pow_i64"));
    assert!(ir.contains("sign_i64"));
    assert!(ir.contains("clamp_i64"));
    assert!(ir.contains("gcd_i64"));
    assert!(ir.contains("lcm_i64"));
    assert!(ir.contains("abs_f64"));
    assert!(ir.contains("min_f64"));
    assert!(ir.contains("max_f64"));
    assert!(ir.contains("sengoo_f64_is_nan"));
    assert!(ir.contains("sengoo_f64_is_finite"));
    assert!(ir.contains("sengoo_f64_is_infinite"));
    assert!(ir.contains("sengoo_f64_sqrt"));
    assert!(ir.contains("sengoo_f64_pow"));
    assert!(ir.contains("sengoo_f64_exp"));
    assert!(ir.contains("sengoo_f64_ln"));
    assert!(ir.contains("sengoo_f64_floor"));
    assert!(ir.contains("sengoo_f64_ceil"));
    assert!(ir.contains("sengoo_f64_round"));
    assert!(ir.contains("sengoo_f64_sin"));
    assert!(ir.contains("sengoo_f64_cos"));
    assert!(ir.contains("sengoo_f64_tan"));
    assert!(ir.contains("sengoo_f32_is_nan"));
    assert!(ir.contains("sengoo_f32_is_finite"));
    assert!(ir.contains("sengoo_f32_is_infinite"));
    assert!(ir.contains("sengoo_f32_sqrt"));
    assert!(ir.contains("sengoo_f32_pow"));
    assert!(ir.contains("sengoo_f32_exp"));
    assert!(ir.contains("sengoo_f32_ln"));
    assert!(ir.contains("sengoo_f32_floor"));
    assert!(ir.contains("sengoo_f32_ceil"));
    assert!(ir.contains("sengoo_f32_round"));
    assert!(ir.contains("sengoo_f32_sin"));
    assert!(ir.contains("sengoo_f32_cos"));
    assert!(ir.contains("sengoo_f32_tan"));
    assert!(ir.contains("checked_i64_to_i32"));
    assert!(ir.contains("checked_i64_to_i16"));
    assert!(ir.contains("checked_i64_to_i8"));
    assert!(ir.contains("checked_i64_to_u64"));
    assert!(ir.contains("checked_i64_to_u32"));
    assert!(ir.contains("checked_i64_to_u16"));
    assert!(ir.contains("checked_i64_to_u8"));
    assert!(ir.contains("checked_i32_to_i16"));
    assert!(ir.contains("checked_i32_to_i8"));
    assert!(ir.contains("checked_i32_to_u64"));
    assert!(ir.contains("checked_i32_to_u32"));
    assert!(ir.contains("checked_i32_to_u16"));
    assert!(ir.contains("checked_i32_to_u8"));
    assert!(ir.contains("checked_i16_to_i8"));
    assert!(ir.contains("checked_i16_to_u64"));
    assert!(ir.contains("checked_i16_to_u32"));
    assert!(ir.contains("checked_i16_to_u16"));
    assert!(ir.contains("checked_i16_to_u8"));
    assert!(ir.contains("checked_i8_to_u64"));
    assert!(ir.contains("checked_i8_to_u32"));
    assert!(ir.contains("checked_i8_to_u16"));
    assert!(ir.contains("checked_i8_to_u8"));
    assert!(ir.contains("checked_u32_to_i64"));
    assert!(ir.contains("checked_u32_to_i32"));
    assert!(ir.contains("checked_u32_to_i16"));
    assert!(ir.contains("checked_u32_to_i8"));
    assert!(ir.contains("checked_u32_to_u16"));
    assert!(ir.contains("checked_u32_to_u8"));
    assert!(ir.contains("checked_u16_to_i64"));
    assert!(ir.contains("checked_u16_to_i32"));
    assert!(ir.contains("checked_u16_to_i16"));
    assert!(ir.contains("checked_u16_to_i8"));
    assert!(ir.contains("checked_u16_to_u32"));
    assert!(ir.contains("checked_u16_to_u64"));
    assert!(ir.contains("checked_u16_to_u8"));
    assert!(ir.contains("checked_u8_to_i64"));
    assert!(ir.contains("checked_u8_to_i32"));
    assert!(ir.contains("checked_u8_to_i16"));
    assert!(ir.contains("checked_u8_to_i8"));
    assert!(ir.contains("checked_u8_to_u16"));
    assert!(ir.contains("checked_u8_to_u32"));
    assert!(ir.contains("checked_u8_to_u64"));
    assert!(ir.contains("checked_isize_to_i64"));
    assert!(ir.contains("checked_isize_to_i32"));
    assert!(ir.contains("checked_isize_to_i16"));
    assert!(ir.contains("checked_isize_to_i8"));
    assert!(ir.contains("checked_isize_to_u64"));
    assert!(ir.contains("checked_isize_to_u32"));
    assert!(ir.contains("checked_isize_to_u16"));
    assert!(ir.contains("checked_isize_to_u8"));
    assert!(ir.contains("checked_usize_to_i64"));
    assert!(ir.contains("checked_usize_to_i32"));
    assert!(ir.contains("checked_usize_to_i16"));
    assert!(ir.contains("checked_usize_to_i8"));
    assert!(ir.contains("checked_usize_to_u64"));
    assert!(ir.contains("checked_usize_to_u32"));
    assert!(ir.contains("checked_usize_to_u16"));
    assert!(ir.contains("checked_usize_to_u8"));
    assert!(ir.contains("sengoo_i64_wrapping_add"));
    assert!(ir.contains("sengoo_i64_saturating_add"));
    assert!(ir.contains("sengoo_i64_checked_add_ok"));
    assert!(ir.contains("i32_Into_i64_into"));
    assert!(ir.contains("u8_Into_u64_into"));
    assert!(ir.contains("u16_Into_u32_into"));
    assert!(ir.contains("u32_Into_u64_into"));
    assert!(ir.contains("isize_Into_i64_into"));
    assert!(ir.contains("usize_Into_u64_into"));
}

#[test]
fn math_module_generic_order_helpers_specialize_across_numeric_families() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "status.sg", "math.sg"],
        r#"
def main() -> i64 {
    let signed = numeric_abs(0 - 7);
    let signed_i32 = numeric_abs(0i32 - 6i32);
    let signed_i16 = numeric_abs(0i16 - 5i16);
    let signed_i8 = numeric_abs(0i8 - 4i8);
    let signed_isize = numeric_abs(0isize - 3isize);
    let unsigned = numeric_min(9u64, 4u64);
    let unsigned_u32 = numeric_min(9u32, 4u32);
    let unsigned_u16 = numeric_min(9u16, 4u16);
    let unsigned_u8 = numeric_min(9u8, 4u8);
    let unsigned_usize = numeric_min(9usize, 4usize);
    let float32 = numeric_max(1.5f32, 2.5f32);
    let float64 = numeric_clamp(12.0, 0.0, 10.0);
    signed
        + unsigned as i64
        + signed_i32 as i64
        + signed_i16 as i64
        + signed_i8 as i64
        + signed_isize as i64
        + unsigned_u32 as i64
        + unsigned_u16 as i64
        + unsigned_u8 as i64
        + unsigned_usize as i64
        + if float32 == 2.5f32 { 1 } else { 0 }
        + if float64 == 10.0 { 1 } else { 0 }
}
"#,
    );

    assert!(ir.contains("numeric_abs_i64"));
    assert!(ir.contains("numeric_abs_i32"));
    assert!(ir.contains("numeric_abs_i16"));
    assert!(ir.contains("numeric_abs_i8"));
    assert!(ir.contains("numeric_min_u64"));
    assert!(ir.contains("numeric_min_u32"));
    assert!(ir.contains("numeric_min_u16"));
    assert!(ir.contains("numeric_min_u8"));
    assert!(ir.contains("numeric_max_f32"));
    assert!(ir.contains("numeric_clamp_f64"));
    assert!(ir.contains("i64_NumericOrder_i64_numeric_abs_value"));
    assert!(ir.contains("u64_NumericOrder_u64_numeric_min_value"));
    assert!(ir.contains("f32_NumericOrder_f32_numeric_max_value"));
    assert!(ir.contains("f64_NumericOrder_f64_numeric_clamp_value"));
}

#[test]
fn math_module_i32_overflow_helpers_compile() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "status.sg", "math.sg"],
        r#"
def main() -> i64 {
    let max = 2147483647i32;
    let min = -2147483648i32;
    let checked_ok = (40i32).checked_add(2i32);
    let checked_add_overflow = max.checked_add(1i32);
    let checked_sub_overflow = min.checked_sub(1i32);
    let checked_mul_overflow = max.checked_mul(2i32);
    let saturated_high = max.saturating_add(1i32);
    let saturated_low = min.saturating_sub(1i32);
    let wrapped = max.wrapping_add(1i32);

    if checked_ok.unwrap_or(0i32) == 42i32
        && checked_add_overflow.is_none()
        && checked_sub_overflow.is_none()
        && checked_mul_overflow.is_none()
        && saturated_high == max
        && saturated_low == min
        && wrapped < 0i32 {
        0
    } else {
        1
    }
}
"#,
    );

    assert!(ir.contains("i32_checked_add"));
    assert!(ir.contains("i32_checked_sub"));
    assert!(ir.contains("i32_checked_mul"));
    assert!(ir.contains("i32_saturating_add"));
    assert!(ir.contains("i32_wrapping_add"));
    assert_eq!(
        ir.matches("define %Option_i32 @option_none_with_i32")
            .count(),
        1,
        "generic option_none_with<i32> should be emitted once:\n{ir}"
    );
}

#[test]
fn math_module_narrow_signed_overflow_helpers_compile() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "status.sg", "math.sg"],
        r#"
def main() -> i64 {
    let i16_max = 32767i16;
    let i16_min = -32768i16;
    let i8_max = 127i8;
    let i8_min = -128i8;

    if (40i16).checked_add(2i16).unwrap_or(0i16) == 42i16
        && i16_max.checked_add(1i16).is_none()
        && i16_min.checked_sub(1i16).is_none()
        && i16_max.checked_mul(2i16).is_none()
        && i16_max.saturating_add(1i16) == i16_max
        && i16_min.saturating_sub(1i16) == i16_min
        && i16_max.wrapping_add(1i16) < 0i16
        && (40i8).checked_add(2i8).unwrap_or(0i8) == 42i8
        && i8_max.checked_add(1i8).is_none()
        && i8_min.checked_sub(1i8).is_none()
        && i8_max.checked_mul(2i8).is_none()
        && i8_max.saturating_add(1i8) == i8_max
        && i8_min.saturating_sub(1i8) == i8_min
        && i8_max.wrapping_add(1i8) < 0i8 {
        0
    } else {
        1
    }
}
"#,
    );

    assert!(ir.contains("i16_checked_add"));
    assert!(ir.contains("i16_saturating_add"));
    assert!(ir.contains("i16_wrapping_add"));
    assert!(ir.contains("i8_checked_add"));
    assert!(ir.contains("i8_saturating_add"));
    assert!(ir.contains("i8_wrapping_add"));
}

#[test]
fn math_module_unsigned_overflow_helpers_compile() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "status.sg", "math.sg"],
        r#"
def main() -> i64 {
    let u32_max = 4294967295u32;
    let u64_max = 18446744073709551615u64;
    let usize_max = 18446744073709551615usize;
    let isize_max = 9223372036854775807isize;
    let isize_min = 0isize - 9223372036854775807isize - 1isize;
    let u16_max = 65535u16;
    let u8_max = 255u8;

    if (40u32).checked_add(2u32).unwrap_or(0u32) == 42u32
        && u32_max.checked_add(1u32).is_none()
        && (0u32).checked_sub(1u32).is_none()
        && u32_max.checked_mul(2u32).is_none()
        && u32_max.saturating_add(1u32) == u32_max
        && (0u32).saturating_sub(1u32) == 0u32
        && u32_max.wrapping_add(1u32) == 0u32
        && (40u64).checked_add(2u64).unwrap_or(0u64) == 42u64
        && u64_max.checked_add(1u64).is_none()
        && (0u64).checked_sub(1u64).is_none()
        && u64_max.checked_mul(2u64).is_none()
        && u64_max.saturating_add(1u64) == u64_max
        && (0u64).saturating_sub(1u64) == 0u64
        && u64_max.wrapping_add(1u64) == 0u64
        && (40usize).checked_add(2usize).unwrap_or(0usize) == 42usize
        && usize_max.checked_add(1usize).is_none()
        && (0usize).checked_sub(1usize).is_none()
        && usize_max.checked_mul(2usize).is_none()
        && usize_max.saturating_add(1usize) == usize_max
        && (0usize).saturating_sub(1usize) == 0usize
        && usize_max.wrapping_add(1usize) == 0usize
        && (40isize).checked_add(2isize).unwrap_or(0isize) == 42isize
        && isize_max.checked_add(1isize).is_none()
        && isize_min.checked_sub(1isize).is_none()
        && isize_max.checked_mul(2isize).is_none()
        && isize_max.saturating_add(1isize) == isize_max
        && isize_min.saturating_sub(1isize) == isize_min
        && isize_max.wrapping_add(1isize) < 0isize
        && (40u16).checked_add(2u16).unwrap_or(0u16) == 42u16
        && u16_max.checked_add(1u16).is_none()
        && (0u16).checked_sub(1u16).is_none()
        && u16_max.checked_mul(2u16).is_none()
        && u16_max.saturating_add(1u16) == u16_max
        && (0u16).saturating_sub(1u16) == 0u16
        && u16_max.wrapping_add(1u16) == 0u16
        && (40u8).checked_add(2u8).unwrap_or(0u8) == 42u8
        && u8_max.checked_add(1u8).is_none()
        && (0u8).checked_sub(1u8).is_none()
        && u8_max.checked_mul(2u8).is_none()
        && u8_max.saturating_add(1u8) == u8_max
        && (0u8).saturating_sub(1u8) == 0u8
        && u8_max.wrapping_add(1u8) == 0u8 {
        0
    } else {
        1
    }
}
"#,
    );

    assert!(ir.contains("u32_checked_add"));
    assert!(ir.contains("u32_saturating_add"));
    assert!(ir.contains("u32_wrapping_add"));
    assert!(ir.contains("u64_checked_add"));
    assert!(ir.contains("u64_saturating_add"));
    assert!(ir.contains("u64_wrapping_add"));
    assert!(ir.contains("usize_checked_add"));
    assert!(ir.contains("usize_saturating_add"));
    assert!(ir.contains("usize_wrapping_add"));
    assert!(ir.contains("isize_checked_add"));
    assert!(ir.contains("isize_saturating_add"));
    assert!(ir.contains("isize_wrapping_add"));
    assert!(ir.contains("u16_checked_add"));
    assert!(ir.contains("u16_saturating_add"));
    assert!(ir.contains("u16_wrapping_add"));
    assert!(ir.contains("u8_checked_add"));
    assert!(ir.contains("u8_saturating_add"));
    assert!(ir.contains("u8_wrapping_add"));
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
    assert!(ir.contains("sengoo_assert_failure_v1"));
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
fn status_module_imports_categories_and_messages() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "status.sg"],
        r#"
def main() -> i64 {
    let buffer = ffi_buffer_new(64).unwrap_or(Buffer { handle: 0 });
    let unknown = STATUS_UNKNOWN();
    let invalid = STATUS_INVALID_ARGUMENT();
    let buffer_error = STATUS_BUFFER_TOO_SMALL();
    let canceled = STATUS_CANCELED();
    let mapped = status_from_raw_ffi(-2001);
    let name_len = status_name_copy(buffer_error, buffer).unwrap_or(0);
    let message_len = status_message_copy(invalid, buffer).unwrap_or(0);
    buffer.free();
    unknown + canceled + mapped + name_len + message_len
}
"#,
    );

    assert!(ir.contains("STATUS_UNKNOWN"));
    assert!(ir.contains("STATUS_INVALID_ARGUMENT"));
    assert!(ir.contains("STATUS_CANCELED"));
    assert!(ir.contains("sengoo_status_name_copy"));
    assert!(ir.contains("sengoo_status_message_copy"));
    assert!(ir.contains("status_from_raw_ffi"));
}

#[test]
fn ffi_buffer_imports_composable_text_helpers() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg"],
        r#"
def main() -> i64 {
    let source = ffi_buffer_from_bytes("abcdef").unwrap_or(Buffer { handle: 0 });
    let out = ffi_buffer_new(16).unwrap_or(Buffer { handle: 0 });
    let capacity = out.capacity();
    out.clear();
    let copied = out.copy_from_str("ab").unwrap_or(0);
    let appended = out.append_str("cd").unwrap_or(0);
    let range = source.copy_range(1, 3, out).unwrap_or(0);
    let used = out.used_len();
    let utf8 = out.is_utf8();
    source.free();
    out.free();
    if capacity >= 16 && copied == 2 && appended == 2 && range == 3 && used == 3 && utf8 {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("sengoo_ffi_buffer_capacity"));
    assert!(ir.contains("sengoo_ffi_buffer_used_len"));
    assert!(ir.contains("sengoo_ffi_buffer_clear"));
    assert!(ir.contains("sengoo_ffi_buffer_copy_range"));
    assert!(ir.contains("sengoo_ffi_buffer_copy_in"));
    assert!(ir.contains("sengoo_ffi_buffer_append"));
    assert!(ir.contains("sengoo_ffi_buffer_is_utf8"));
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
            "string.sg",
            "status.sg",
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
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "status.sg",
            "net.sg",
        ],
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
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "status.sg",
            "net.sg",
        ],
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
    iter.reset();
    let folded = iter.fold_with(10, |acc, value| acc + value);
    iter.reset();
    let skipped_sum = iter.skip(2).sum();

    let map = hashmap_new_i64_i64();
    map.insert(1, 5);
    map.insert(2, 6);
    let map_iter = map.iter();
    let map_folded = map_iter.fold_with(0, |acc, value| acc + value);
    map_iter.reset();
    let map_skipped = map_iter.skip(1).count();

    mapped + filtered + folded + skipped_sum + map_folded + map_skipped
}
"#,
    );

    assert!(!ir.contains("call i64 @f("));
    assert!(ir.contains("VecIter_i64_Iterator_fold_with"));
    assert!(ir.contains("VecIter_i64_Iterator_skip"));
    assert!(ir.contains("HashMapIter_i64_Iterator_fold_with"));
    assert!(ir.contains("HashMapIter_i64_Iterator_skip"));
}

#[test]
fn stdlib_surface_bool_iterator_fold_adapters_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec = vec_new_bool();
    vec.push(true);
    vec.push(false);
    let vec_iter = vec.iter();
    let any_false = vec_iter.skip(1).fold_with(true, |acc, value| acc && value);

    let map = hashmap_new_bool_bool();
    map.insert(true, true);
    map.insert(false, false);
    let map_iter = map.iter();
    let all_true = map_iter.skip(1).fold_with(true, |acc, value| acc && value);

    if any_false || all_true { 1 } else { 0 }
}
"#,
    );

    assert!(ir.contains("VecIter_bool_fold_with"));
    assert!(ir.contains("VecIter_bool_skip"));
    assert!(ir.contains("HashMapIter_bool_fold_with"));
    assert!(ir.contains("HashMapIter_bool_skip"));
}

#[test]
fn stdlib_surface_vec_iterator_enumerate_adapters_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    vec.push(10);
    vec.push(20);
    let mut enumerated = vec.iter().enumerate();
    let first = enumerated.next().unwrap_or(EnumeratedI64 { index: 0, value: 0 });
    let second = enumerated.next().unwrap_or(EnumeratedI64 { index: 0, value: 0 });

    let bool_vec = vec_new_bool();
    bool_vec.push(false);
    bool_vec.push(true);
    let mut bool_enumerated = bool_vec.iter().enumerate();
    let _first_bool = bool_enumerated.next().unwrap_or(EnumeratedBool { index: 0, value: false });
    let second_bool = bool_enumerated.next().unwrap_or(EnumeratedBool { index: 0, value: false });

    let map = hashmap_new_i64_i64();
    map.insert(7, 30);
    let mut map_enumerated = map.iter().enumerate();
    let first_map = map_enumerated.next().unwrap_or(EnumeratedI64 { index: 0, value: 0 });

    let bool_map = hashmap_new_bool_bool();
    bool_map.insert(true, true);
    let mut bool_map_enumerated = bool_map.iter().enumerate();
    let first_bool_map = bool_map_enumerated.next().unwrap_or(EnumeratedBool { index: 0, value: false });

    let set = hashset_new_i64();
    set.insert(42);
    let mut set_enumerated = set.iter().enumerate();
    let first_set = set_enumerated.next().unwrap_or(EnumeratedI64 { index: 0, value: 0 });

    let bool_set = hashset_new_bool();
    bool_set.insert(true);
    let mut bool_set_enumerated = bool_set.iter().enumerate();
    let first_bool_set = bool_set_enumerated.next().unwrap_or(EnumeratedBool { index: 0, value: false });

    let word_set = hashset_new_string();
    word_set.insert("ready");
    let mut word_set_enumerated = word_set.iter().enumerate();
    let first_word_set = word_set_enumerated.next().unwrap_or(EnumeratedString { index: 0, value: string_new() });

    first.index + first.value + second.index + second.value
        + second_bool.index + if second_bool.value { 1 } else { 0 }
        + first_map.index + first_map.value
        + first_bool_map.index + if first_bool_map.value { 1 } else { 0 }
        + first_set.index + first_set.value
        + first_bool_set.index + if first_bool_set.value { 1 } else { 0 }
        + first_word_set.index + first_word_set.value.len()
}
"#,
    );

    assert!(ir.contains("VecIter_i64_Iterator_enumerate"));
    assert!(ir.contains("EnumerateVecIterI64_next"));
    assert!(ir.contains("VecIter_bool_enumerate"));
    assert!(ir.contains("EnumerateVecIterBool_next"));
    assert!(ir.contains("HashMapIter_i64_Iterator_enumerate"));
    assert!(ir.contains("EnumerateHashMapIterI64_next"));
    assert!(ir.contains("HashMapIter_bool_enumerate"));
    assert!(ir.contains("EnumerateHashMapIterBool_next"));
    assert!(ir.contains("HashSet_i64_iter"));
    assert!(ir.contains("HashSetIter_i64_Iterator_enumerate"));
    assert!(ir.contains("EnumerateHashSetIterI64_next"));
    assert!(ir.contains("HashSet_bool_iter"));
    assert!(ir.contains("HashSetIter_bool_Iterator_enumerate"));
    assert!(ir.contains("EnumerateHashSetIterBool_next"));
    assert!(ir.contains("HashSet_String_iter"));
    assert!(ir.contains("StringMapKeyIter_enumerate"));
    assert!(ir.contains("EnumerateStringMapKeyIter_next"));
    assert!(ir.contains("sengoo_hashmap_key_iter_next_or_default_i64"));
    assert!(ir.contains("sengoo_string_map_key_iter_next_string"));
}

#[test]
fn stdlib_surface_iterator_take_helpers_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    vec.push(1);
    vec.push(2);
    vec.push(3);
    let taken = vec.iter().take(2);
    let taken_sum = taken.iter().sum();

    let bool_vec = vec_new_bool();
    bool_vec.push(true);
    bool_vec.push(false);
    let bool_taken = bool_vec.iter().take(1);
    let bool_value = if bool_taken.get(0).unwrap_or(false) { 1 } else { 0 };

    let map = hashmap_new_i64_i64();
    map.insert(1, 10);
    map.insert(2, 20);
    let map_taken = map.iter().take(1);

    let bool_map = hashmap_new_bool_bool();
    bool_map.insert(true, true);
    bool_map.insert(false, false);
    let bool_map_taken = bool_map.iter().take(2);
    let bool_map_value = if bool_map_taken.get(1).unwrap_or(false) { 1 } else { 0 };

    let set = hashset_new_i64();
    set.insert(7);
    set.insert(9);
    let set_taken = set.iter().take(1);

    let bool_set = hashset_new_bool();
    bool_set.insert(true);
    bool_set.insert(false);
    let bool_set_taken = bool_set.iter().take(2);

    let strings = vec_new_string();
    strings.push(string_from_str("alpha").unwrap_or(string_new()));
    strings.push(string_from_str("beta").unwrap_or(string_new()));
    strings.push(string_from_str("gamma").unwrap_or(string_new()));
    let string_taken = strings.iter().skip(1).take(2);

    taken_sum + bool_value + map_taken.len() + bool_map_taken.len() + bool_map_value
        + set_taken.len() + bool_set_taken.len() + string_taken.len()
}

"#,
    );

    assert!(ir.contains("VecIter_i64_Iterator_take"));
    assert!(ir.contains("VecIter_bool_take"));
    assert!(ir.contains("HashMapIter_i64_Iterator_take"));
    assert!(ir.contains("HashMapIter_bool_take"));
    assert!(ir.contains("HashSetIter_i64_Iterator_take"));
    assert!(ir.contains("HashSetIter_bool_Iterator_take"));
    assert!(ir.contains("VecStringIter_skip"));
    assert!(ir.contains("VecStringIter_take"));
}

#[test]
fn stdlib_surface_generic_into_iterator_take_is_lazy_state_machine() {
    let ir = compile_with_stdlib(
        r#"
struct Payload {
    value: i64,
}

def main() -> i64 {
    let values: Vec<Payload> = vec_new();
    values.push(Payload { value: 10 });
    values.push(Payload { value: 20 });
    values.push(Payload { value: 30 });
    let iter = values.into_iter().skip(1).take(1);
    let first: Option<Payload> = iter.next();
    let exhausted: Option<Payload> = iter.next();
    if first.is_some && first.value.value == 20 && exhausted.is_none() { 0 } else { 1 }
}
"#,
    );

    assert!(
        ir.contains("TakeIter_") && ir.contains("_next"),
        "expected a concrete lazy TakeIter specialization:\n{ir}"
    );
}

#[test]
fn stdlib_surface_iterator_collect_helpers_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    vec.push(1);
    vec.push(2);
    let collected = vec.iter().collect();
    let collected_sum = collected.iter().sum();

    let bool_vec = vec_new_bool();
    bool_vec.push(true);
    bool_vec.push(false);
    let bool_collected = bool_vec.iter().collect();
    let bool_value = if bool_collected.get(0).unwrap_or(false) { 1 } else { 0 };

    let map = hashmap_new_i64_i64();
    map.insert(1, 10);
    map.insert(2, 20);
    let map_collected = map.iter().collect();

    let bool_map = hashmap_new_bool_bool();
    bool_map.insert(true, true);
    bool_map.insert(false, false);
    let bool_map_collected = bool_map.iter().collect();

    let strings = vec_new_string();
    strings.push(string_from_str("alpha").unwrap_or(string_new()));
    strings.push(string_from_str("beta").unwrap_or(string_new()));
    let string_count = strings.iter().count();
    let string_collected = strings.iter().collect();

    let string_key_map: HashMap<String, i64> = hashmap_new_string_i64();
    string_key_map.insert("alpha", 1);
    string_key_map.insert("beta", 2);
    let string_key_collected = string_key_map.iter_keys().collect();

    let string_text_map: HashMap<String, String> = hashmap_new_string_string();
    string_text_map.insert("title", string_from_str("gamma").unwrap_or(string_new()));
    let string_text_keys = string_text_map.iter_keys().collect();

    let string_set = hashset_new_string();
    string_set.insert("ready");
    let string_set_keys = string_set.iter().collect();

    collected_sum + bool_value + map_collected.len() + bool_map_collected.len()
        + string_count + string_collected.len() + string_key_collected.len()
        + string_text_keys.len() + string_set_keys.len()
}
"#,
    );

    assert!(ir.contains("VecIter_i64_Iterator_collect"));
    assert!(ir.contains("VecIter_bool_collect"));
    assert!(ir.contains("HashMapIter_i64_Iterator_collect"));
    assert!(ir.contains("HashMapIter_bool_collect"));
    assert!(ir.contains("VecStringIter_collect"));
    assert!(ir.contains("VecStringIter_count"));
    assert!(ir.contains("StringMapKeyIter_collect"));
    assert!(ir.contains("StringMapStringKeyIter_collect"));
}

#[test]
fn stdlib_surface_ordered_string_collections_compile() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let counts: BTreeMap<String, i64> = btreemap_new_string_i64();
    counts.insert("zeta", 6);
    counts.insert("alpha", 1);
    let count_keys = counts.iter_keys().collect();

    let flags: BTreeMap<String, bool> = btreemap_new_string_bool();
    flags.insert("ready", true);

    let labels: BTreeMap<String, String> = btreemap_new_string_string();
    labels.insert("name", string_from_str("sengoo").unwrap_or(string_new()));

    let names: BTreeSet<String> = btreeset_new_string();
    names.insert("zeta");
    names.insert("alpha");
    let name_keys = names.iter().collect();

    counts.get("alpha").unwrap_or(0)
        + if flags.get("ready").unwrap_or(false) { 1 } else { 0 }
        + labels.get("name").unwrap_or(string_new()).len()
        + count_keys.len()
        + name_keys.len()
}
"#,
    );

    assert!(ir.contains("btreemap_new_string_i64"));
    assert!(ir.contains("btreemap_new_string_bool"));
    assert!(ir.contains("btreemap_new_string_string"));
    assert!(ir.contains("btreeset_new_string"));
    assert!(ir.contains("sengoo_string_map_key_iter_new"));
    assert!(ir.contains("sengoo_string_map_string_get_clone"));
}

#[test]
fn stdlib_surface_ordered_i64_collections_compile_over_dedicated_runtime() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let counts: BTreeMap<i64, i64> = btreemap_new_i64_i64();
    counts.insert(30, 3);
    counts.insert(-4, 4);
    counts.insert(10, 1);
    counts.insert(10, 9);
    let keys = counts.iter_keys().collect();

    let flags: BTreeMap<i64, bool> = btreemap_new_i64_bool();
    flags.insert(2, true);
    flags.insert(2, false);

    let ids: BTreeSet<i64> = btreeset_new_i64();
    ids.insert(8);
    ids.insert(-8);
    let first_id = ids.iter().next().unwrap_or(0);

    counts.get(10).unwrap_or(0)
        + if flags.get(2).unwrap_or(true) { 1 } else { 0 }
        + keys.get(0).unwrap_or(0)
        + first_id
}
"#,
    );

    for symbol in [
        "btreemap_new_i64_i64",
        "btreemap_new_i64_bool",
        "btreeset_new_i64",
        "sengoo_btreemap_insert_i64",
        "sengoo_btreemap_get_or_default_i64",
        "sengoo_btreemap_remove_i64",
        "sengoo_btreemap_key_iter_new_i64",
        "sengoo_btreemap_key_iter_next_or_default_i64",
        "sengoo_btreemap_free_i64_status",
    ] {
        assert!(
            ir.contains(symbol),
            "missing ordered i64 symbol {symbol}\n{ir}"
        );
    }
    assert!(ir.contains("BTreeMap_i64_i64_Drop_drop"));
    assert!(ir.contains("BTreeMap_i64_bool_Drop_drop"));
    assert!(ir.contains("BTreeSet_i64_Drop_drop"));

    let ordered_iter = llvm_function_section(&ir, "; Function: BTreeMap_i64_i64_iter_keys");
    assert!(ordered_iter.contains("@sengoo_btreemap_key_iter_new_i64"));
    assert!(!ordered_iter.contains("@sengoo_hashmap_key_iter_new_i64"));
    for method in [
        "BTreeMap_i64_i64_insert",
        "BTreeMap_i64_i64_get",
        "BTreeMap_i64_i64_remove",
        "BTreeMap_i64_i64_clear",
    ] {
        let section = llvm_function_section(&ir, &format!("; Function: {method}"));
        assert!(
            !section.contains("@BTreeMap_i64_i64_Drop_drop"),
            "{method} borrows its receiver and must not release the map handle:\n{section}"
        );
    }
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
    let vec: Vec<bool> = Vec { handle: 0, marker: 0 };
    let map: HashMap<bool, bool> = HashMap { handle: 0, key_marker: 0, value_marker: 0 };
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
    vec.insert(1, true);

    let first = vec.get(0).unwrap_or(false);
    let second = vec.pop().unwrap_or(true);
    let had_true = vec.contains(true);
    vec.set(0, false);
    let removed = vec.remove(0).unwrap_or(true);
    let removed_tail = vec.remove(0).unwrap_or(false);

    if first && !second && had_true && !removed && removed_tail && vec.is_empty() {
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
    assert!(bool_ir.contains("Vec_bool_insert"));
    assert!(bool_ir.contains("Vec_bool_remove"));
}

#[test]
fn stdlib_surface_vecdeque_i64_and_bool_compile_over_vec_runtime() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec = vec_new_i64();
    let inserted = vec.insert(0, 3);

    let deque = vecdeque_new_i64();
    let pushed_back = deque.push_back(2);
    let pushed_front = deque.push_front(1);
    let front = deque.front().unwrap_or(0);
    let back = deque.back().unwrap_or(0);
    let popped_front = deque.pop_front().unwrap_or(0);
    let popped_back = deque.pop_back().unwrap_or(0);

    let flags = vecdeque_new_bool();
    let flag_back = flags.push_back(false);
    let flag_front = flags.push_front(true);
    let front_flag = flags.front().unwrap_or(false);
    let back_flag = flags.back().unwrap_or(true);
    let popped_front_flag = flags.pop_front().unwrap_or(false);
    let popped_back_flag = flags.pop_back().unwrap_or(true);

    if inserted && pushed_back && pushed_front && front == 1 && back == 2
        && popped_front == 1 && popped_back == 2 && deque.is_empty()
        && flag_back && flag_front && front_flag && !back_flag
        && popped_front_flag && !popped_back_flag && flags.is_empty() {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("vecdeque_new_i64"));
    assert!(ir.contains("Vec_i64_insert"));
    assert!(ir.contains("VecDeque_i64_push_front"));
    assert!(ir.contains("VecDeque_i64_pop_front"));
    assert!(ir.contains("vecdeque_new_bool"));
    assert!(ir.contains("VecDeque_bool_push_front"));
    assert!(ir.contains("VecDeque_bool_pop_front"));
    assert!(ir.contains("sengoo_vec_insert_i64"));
    assert!(ir.contains("sengoo_vec_remove_or_default_i64"));
}

#[test]
fn stdlib_surface_generic_vecdeque_struct_uses_raw_vec_runtime() {
    let ir = compile_with_stdlib(
        r#"
struct Payload { value: i64 }

def main() -> i64 {
    let deque: VecDeque<Payload> = vecdeque_new();
    deque.push_back(Payload { value: 2 });
    deque.push_front(Payload { value: 1 });
    let borrowed_ok = {
        let front = deque.front();
        let back = deque.back();
        (*front).value == 1 && (*back).value == 2
    };
    let popped_front = deque.pop_front();
    let popped_back = deque.pop_back();
    if borrowed_ok && popped_front.is_some() && popped_back.is_some() { 1 } else { 0 }
}
"#,
    );
    assert!(ir.contains("call i64 @sengoo_raw_vec_new_parts"));
    assert!(ir.contains("call i64 @sengoo_raw_vec_push"));
    assert!(ir.contains("call i64 @sengoo_raw_vec_insert"));
    assert!(ir.contains("call i64 @sengoo_raw_vec_remove"));
    assert!(ir.contains("; Function: VecDeque_Payload_Drop_drop"));
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

    let string_i64_ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let map: HashMap<String, i64> = hashmap_new_string_i64();
    let initially_empty = map.is_empty();
    map.insert("alpha", 2);
    map.insert("beta", 7);
    let length = map.len();
    let beta_value = map.get("beta").unwrap_or(0);
    let removed_alpha = map.remove("alpha");
    map.clear();
    if initially_empty && length == 2 && beta_value == 7 && removed_alpha
        && !map.contains("alpha") && map.is_empty() {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(string_i64_ir.contains("hashmap_new_string_i64"));
    assert!(string_i64_ir.contains("HashMap_String_i64_insert"));
    assert!(string_i64_ir.contains("HashMap_String_i64_get"));
    assert!(string_i64_ir.contains("HashMap_String_i64_remove"));
    assert!(string_i64_ir.contains("HashMap_String_i64_len"));
    assert!(string_i64_ir.contains("HashMap_String_i64_clear"));
    let insert_section = string_i64_ir
        .split("; Function: HashMap_String_i64_insert")
        .nth(1)
        .expect("HashMap<String, i64>.insert should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        insert_section.contains("sengoo_string_map_insert_i64"),
        "HashMap<String, i64>.insert should call the string-keyed runtime:\n{}",
        insert_section
    );
    let get_section = string_i64_ir
        .split("; Function: HashMap_String_i64_get")
        .nth(1)
        .expect("HashMap<String, i64>.get should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        get_section.contains("sengoo_string_map_get_or_default_i64"),
        "HashMap<String, i64>.get should call the string-keyed runtime:\n{}",
        get_section
    );
    let len_section = string_i64_ir
        .split("; Function: HashMap_String_i64_len")
        .nth(1)
        .expect("HashMap<String, i64>.len should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        len_section.contains("sengoo_string_map_len")
            && !len_section.contains("sengoo_hashmap_len_i64"),
        "HashMap<String, i64>.len should use the string-keyed runtime:\n{}",
        len_section
    );
    let clear_section = string_i64_ir
        .split("; Function: HashMap_String_i64_clear")
        .nth(1)
        .expect("HashMap<String, i64>.clear should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        clear_section.contains("sengoo_string_map_clear_status"),
        "HashMap<String, i64>.clear should use the string-keyed runtime:\n{}",
        clear_section
    );

    let string_bool_ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let map: HashMap<String, bool> = hashmap_new_string_bool();
    let initially_empty = map.is_empty();
    map.insert("enabled", true);
    map.insert("enabled", false);
    let length = map.len();
    let value = map.get("enabled").unwrap_or(true);
    let removed = map.remove("enabled");
    map.clear();
    if initially_empty && length == 1 && !value && removed
        && !map.contains("enabled") && map.is_empty() {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(string_bool_ir.contains("hashmap_new_string_bool"));
    assert!(string_bool_ir.contains("HashMap_String_bool_insert"));
    assert!(string_bool_ir.contains("HashMap_String_bool_get"));
    assert!(string_bool_ir.contains("HashMap_String_bool_remove"));
    let string_bool_insert_section = string_bool_ir
        .split("; Function: HashMap_String_bool_insert")
        .nth(1)
        .expect("HashMap<String, bool>.insert should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        string_bool_insert_section.contains("sengoo_string_map_insert_bool"),
        "HashMap<String, bool>.insert should call the string-bool runtime:\n{}",
        string_bool_insert_section
    );

    let string_string_ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let map: HashMap<String, String> = hashmap_new_string_string();
    let initially_empty = map.is_empty();
    map.insert("title", string_from_str("alpha").unwrap_or(String { handle: 0 }));
    map.insert("title", string_from_str("gamma").unwrap_or(String { handle: 0 }));
    let length = map.len();
    let key_count = map.iter_keys().count();
    let taken_keys = map.iter_keys().take(1);
    let skipped_key_count = map.iter_keys().skip(1).count();
    let value = map.get("title").unwrap_or(String { handle: 0 });
    let removed = map.remove("title").unwrap_or(String { handle: 0 });
    map.clear();
    if initially_empty && length == 1 && key_count == 1 && taken_keys.len() == 1
        && skipped_key_count == 0 && value.len() == 5 && removed.len() == 5
        && !map.contains("title") && map.is_empty() {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(string_string_ir.contains("hashmap_new_string_string"));
    assert!(string_string_ir.contains("HashMap_String_String_insert"));
    assert!(string_string_ir.contains("HashMap_String_String_get"));
    assert!(string_string_ir.contains("HashMap_String_String_remove"));
    assert!(string_string_ir.contains("StringMapStringKeyIter_count"));
    assert!(string_string_ir.contains("StringMapStringKeyIter_take"));
    let string_insert_section = string_string_ir
        .split("; Function: HashMap_String_String_insert")
        .nth(1)
        .expect("HashMap<String, String>.insert should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        string_insert_section.contains("sengoo_string_map_string_insert"),
        "HashMap<String, String>.insert should call the string-string runtime:\n{}",
        string_insert_section
    );
    let string_get_section = string_string_ir
        .split("; Function: HashMap_String_String_get")
        .nth(1)
        .expect("HashMap<String, String>.get should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        string_get_section.contains("sengoo_string_map_string_get_clone"),
        "HashMap<String, String>.get should call the string-string runtime:\n{}",
        string_get_section
    );
}

#[test]
fn stdlib_surface_generic_vec_constructor_emits_descriptor_callbacks() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "collections.sg",
        ],
        r#"
def main() -> i64 {
    let values: Vec<i64> = vec_new();
    values.len()
}
"#,
    );
    assert!(
        ir.contains("call i64 @sengoo_raw_vec_new_parts"),
        "generic Vec constructor should use the descriptor runtime\n{ir}"
    );
    assert!(
        ir.contains("@__sengoo_vec_move_i64") && ir.contains("@__sengoo_rc_drop_VecElement_i64"),
        "generic Vec constructor should materialize move/drop callbacks\n{ir}"
    );
}

#[test]
fn stdlib_surface_generic_vec_struct_push_uses_raw_runtime() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "collections.sg",
        ],
        r#"
struct Payload {
    x: i64,
    y: i64,
}

def main() -> i64 {
    let values: Vec<Payload> = vec_new();
    let pushed = values.push(Payload { x: 20, y: 22 });
    let replaced = values.set(0, Payload { x: 40, y: 2 });
    let inserted = values.insert(0, Payload { x: 1, y: 2 });
    let borrowed_ok = {
        let borrowed = values.get(0);
        (*borrowed).x == 1
    };
    let iter_ok = {
        let iter = values.iter();
        iter.next().is_some()
    };
    let removed = values.remove(1);
    let popped = values.pop();
    values.push(Payload { x: 7, y: 8 });
    let into = values.into_iter();
    let into_ok = into.next().is_some();
    if pushed && replaced && inserted && borrowed_ok && iter_ok && removed.is_some() && popped.is_some() && into_ok {
        1
    } else {
        0
    }
}
"#,
    );
    assert!(
        ir.contains("; Function: Vec_Payload_push") && ir.contains("call i64 @sengoo_raw_vec_push"),
        "generic Vec<struct>.push should use the RawVec runtime\n{ir}"
    );
    assert!(
        ir.contains("; Function: Vec_Payload_iter")
            && ir.contains("call i64 @sengoo_raw_vec_iter_new")
            && ir.contains("; Function: RawVecIter_Payload_next")
            && ir.contains("call i8* @sengoo_raw_vec_iter_next")
            && ir.contains("; Function: Vec_Payload_into_iter")
            && ir.contains("; Function: RawVecIntoIter_Payload_next"),
        "generic Vec<struct> iterators should use borrowed and owning RawVec paths\n{ir}"
    );
    assert!(
        ir.contains("; Function: Vec_Payload_get")
            && ir.contains("call i8* @sengoo_raw_vec_get")
            && ir.contains("; Function: Vec_Payload_remove")
            && ir.contains("call i64 @sengoo_raw_vec_remove")
            && ir.contains("; Function: Vec_Payload_pop")
            && ir.contains("call i64 @sengoo_raw_vec_pop"),
        "generic Vec<struct> get/pop/remove should use the RawVec runtime\n{ir}"
    );
    assert!(
        ir.contains("; Function: Vec_Payload_set")
            && ir.contains("call i64 @sengoo_raw_vec_set")
            && ir.contains("; Function: Vec_Payload_insert")
            && ir.contains("call i64 @sengoo_raw_vec_insert"),
        "generic Vec<struct> set/insert should use the RawVec runtime\n{ir}"
    );
    assert!(
        ir.contains("; Function: Vec_Payload_Drop_drop")
            && ir.contains("call i64 @sengoo_raw_vec_free"),
        "generic Vec<struct> should auto-drop through RawVec\n{ir}"
    );
}

#[test]
fn stdlib_surface_generic_vec_rejects_mutation_while_element_borrow_is_live() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib_surface(&[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "collections.sg",
        ]),
        r#"
struct Payload { value: i64 }

def main() -> i64 {
    let values: Vec<Payload> = vec_new();
    values.push(Payload { value: 1 });
    let borrowed = values.get(0);
    values.push(Payload { value: 2 });
    (*borrowed).value
}

"#
    );
    let err = compile_to_ir(&source).expect_err("a live Vec element borrow must block growth");
    assert!(
        err.to_string()
            .contains("cannot move borrowed value `values`"),
        "unexpected Vec borrow invalidation diagnostic: {err}"
    );
}

#[test]
fn stdlib_surface_generic_vec_rejects_mutation_while_borrowing_iterator_is_live() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib_surface(&[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "collections.sg",
        ]),
        r#"
struct Payload { value: i64 }

def main() -> i64 {
    let values: Vec<Payload> = vec_new();
    values.push(Payload { value: 1 });
    let iter = values.iter();
    values.clear();
    if iter.next().is_some() { 1 } else { 0 }
}
"#
    );
    let err = compile_to_ir(&source).expect_err("a live Vec iterator must block clear");
    assert!(
        err.to_string()
            .contains("cannot move borrowed value `values`"),
        "unexpected Vec iterator invalidation diagnostic: {err}"
    );
}

#[test]
fn stdlib_surface_generic_hashmap_struct_callbacks_use_raw_runtime() {
    let ir = compile_with_stdlib(
        r#"
#[derive(Hash, PartialEq, Eq)]
struct Key { id: i64 }

struct Payload { value: i64 }

def main() -> i64 {
    let map: HashMap<Key, Payload> = hashmap_new();
    let inserted = map.insert(Key { id: 7 }, Payload { value: 42 });
    let lookup = Key { id: 7 };
    let borrowed_ok = {
        let value = map.get(&lookup);
        (*value).value == 42
    };
    let contains = map.contains(&lookup);
    let removed = map.remove(&lookup);
    if inserted && borrowed_ok && contains && removed.is_some() && map.is_empty() { 1 } else { 0 }
}
"#,
    );
    assert!(ir.contains("call i64 @sengoo_raw_hashmap_new_parts"));
    assert!(ir.contains("; Function: __sengoo_hash_Key"));
    assert!(ir.contains("call i64 @Key_Hash_hash"));
    assert!(ir.contains("; Function: __sengoo_eq_Key"));
    assert!(ir.contains("call i1 @Key_eq"));
    assert!(ir.contains("call i64 @sengoo_raw_hashmap_insert"));
    assert!(ir.contains("call i8* @sengoo_raw_hashmap_get"));
    assert!(ir.contains("call i64 @sengoo_raw_hashmap_remove"));
    assert!(ir.contains("; Function: HashMap_Key_Payload_Drop_drop"));
}

#[test]
fn stdlib_surface_generic_hashset_struct_uses_key_descriptor() {
    let ir = compile_with_stdlib(
        r#"
#[derive(Hash, PartialEq, Eq)]
struct Key { id: i64 }

def main() -> i64 {
    let set: HashSet<Key> = hashset_new();
    let inserted = set.insert(Key { id: 7 });
    let duplicate = set.insert(Key { id: 7 });
    let lookup = Key { id: 7 };
    let contains = set.contains(&lookup);
    let removed = set.remove(&lookup);
    if inserted && duplicate && contains && removed && set.is_empty() { 1 } else { 0 }
}
"#,
    );
    assert!(ir.contains("call i64 @sengoo_raw_hashmap_new_parts"));
    assert!(ir.contains("; Function: __sengoo_hash_Key"));
    assert!(ir.contains("; Function: __sengoo_eq_Key"));
    assert!(ir.contains("call i64 @sengoo_raw_hashmap_insert"));
    assert!(ir.contains("call i64 @sengoo_raw_hashmap_contains"));
    assert!(ir.contains("call i64 @sengoo_raw_hashmap_remove"));
    assert!(ir.contains("; Function: HashSet_Key_Drop_drop"));
}

#[test]
fn stdlib_surface_generic_btree_struct_uses_ord_descriptor_and_key_cursor() {
    let ir = compile_with_stdlib(
        r#"
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Key { id: i64 }

struct Payload { value: i64 }

def main() -> i64 {
    let map: BTreeMap<Key, Payload> = btreemap_new();
    map.insert(Key { id: 2 }, Payload { value: 20 });
    map.insert(Key { id: 1 }, Payload { value: 10 });
    let fallback = Key { id: 0 };
    let ordered = {
        let keys = map.iter_keys();
        let first = keys.next().unwrap_or(&fallback);
        let second = keys.next().unwrap_or(&fallback);
        (*first).id == 1 && (*second).id == 2
    };
    let set: BTreeSet<Key> = btreeset_new();
    set.insert(Key { id: 4 });
    set.insert(Key { id: 3 });
    if ordered && map.len() == 2 && set.len() == 2 { 1 } else { 0 }
}
"#,
    );
    assert!(ir.contains("call i64 @sengoo_raw_btreemap_new_parts"));
    assert!(ir.contains("; Function: __sengoo_compare_Key"));
    assert!(ir.contains("call i64 @Key_compare"));
    assert!(ir.contains("call i64 @sengoo_raw_map_key_iter_new"));
    assert!(ir.contains("call i8* @sengoo_raw_map_key_iter_next"));
    assert!(ir.contains("; Function: BTreeMap_Key_Payload_Drop_drop"));
    assert!(ir.contains("; Function: BTreeSet_Key_Drop_drop"));
}

#[test]
fn stdlib_surface_vec_string_methods_use_string_runtime() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec: Vec<String> = vec_new_string();
    let first = string_from_str("alpha").unwrap_or(String { handle: 0 });
    let second = string_from_str("beta").unwrap_or(String { handle: 0 });
    let middle = string_from_str("mid").unwrap_or(String { handle: 0 });
    let replacement = string_from_str("gamma").unwrap_or(String { handle: 0 });
    let initially_empty = vec.is_empty();
    vec.push(first);
    vec.push(second);
    vec.insert(1, middle);
    vec.set(2, replacement);
    let inserted = vec.get(1).unwrap_or(String { handle: 0 });
    let replaced = vec.get(2).unwrap_or(String { handle: 0 });
    let length = vec.len();
    vec.clear();
    if initially_empty && length == 3 && inserted.len() == 3 && replaced.len() == 5 && vec.is_empty() {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("vec_new_string"));
    assert!(ir.contains("Vec_String_len"));
    assert!(ir.contains("Vec_String_clear"));
    assert!(ir.contains("Vec_String_set"));
    assert!(ir.contains("Vec_String_insert"));
    let len_section = ir
        .split("; Function: Vec_String_len")
        .nth(1)
        .expect("Vec<String>.len should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        len_section.contains("sengoo_vec_string_len")
            && !len_section.contains("sengoo_vec_len_i64"),
        "Vec<String>.len should use the string vector runtime:\n{}",
        len_section
    );
    let clear_section = ir
        .split("; Function: Vec_String_clear")
        .nth(1)
        .expect("Vec<String>.clear should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        clear_section.contains("sengoo_vec_string_clear_status"),
        "Vec<String>.clear should use the string vector runtime:\n{}",
        clear_section
    );
    let set_section = ir
        .split("; Function: Vec_String_set")
        .nth(1)
        .expect("Vec<String>.set should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        set_section.contains("sengoo_vec_string_set"),
        "Vec<String>.set should use the string vector runtime:\n{}",
        set_section
    );
    let insert_section = ir
        .split("; Function: Vec_String_insert")
        .nth(1)
        .expect("Vec<String>.insert should be emitted")
        .split("; Function: ")
        .next()
        .unwrap();
    assert!(
        insert_section.contains("sengoo_vec_string_insert"),
        "Vec<String>.insert should use the string vector runtime:\n{}",
        insert_section
    );
}

#[test]
fn stdlib_surface_hashset_runtime_mutators_support_i64_bool_and_string() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let ids = hashset_new_i64();
    ids.insert(42);
    let id_ok = ids.contains(42);
    let id_removed = ids.remove(42);

    let flags = hashset_new_bool();
    flags.insert(true);
    let flag_ok = flags.contains(true);

    let words = hashset_new_string();
    words.insert("ready");
    let word_ok = words.contains("ready");
    let word_removed = words.remove("ready");

    if id_ok && id_removed && flag_ok && word_ok && word_removed {
        1
    } else {
        0
    }
}
"#,
    );

    assert!(ir.contains("hashset_new_i64"));
    assert!(ir.contains("hashset_new_bool"));
    assert!(ir.contains("hashset_new_string"));
    assert!(ir.contains("HashSet_i64_insert"));
    assert!(ir.contains("HashSet_bool_contains"));
    assert!(ir.contains("HashSet_String_remove"));
    assert!(ir.contains("sengoo_hashmap_insert_i64"));
    assert!(ir.contains("sengoo_string_map_insert_bool"));
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
