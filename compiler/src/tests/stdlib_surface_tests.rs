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

fn compile_with_stdlib_error(program: &str) -> String {
    let source = format!(
        "{}\n\n{}",
        load_stdlib_surface(&["option.sg", "result.sg", "collections.sg"]),
        program
    );
    compile_to_ir(&source)
        .expect_err("stdlib surface program should fail")
        .to_string()
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
    str_len("hello")
}
"#,
    );

    assert!(ir.contains("sengoo_str_len"));
}

#[test]
fn math_module_imports_and_runs_abs_i64() {
    let ir = compile_with_stdlib_modules(
        &["math.sg"],
        r#"
def main() -> i64 {
    abs_i64(0 - 7) + min_i64(4, 9) + max_i64(4, 9) + pow_i64(2, 3)
}
"#,
    );

    assert!(ir.contains("abs_i64"));
    assert!(ir.contains("pow_i64"));
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
fn stdlib_surface_vec_runtime_mutators_remain_i64_only() {
    let i64_ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let vec: Vec<i64> = Vec { handle: 0, marker: 0 };
    if vec.push(1) { 1 } else { 0 }
}
"#,
    );
    assert!(i64_ir.contains("sengoo_vec_push_i64"));

    let err = compile_with_stdlib_error(
        r#"
def main() -> i64 {
    let vec: Vec<bool> = Vec { handle: 0, marker: false };
    vec.push(true);
    0
}
"#,
    );

    assert!(
        err.contains("Vec<bool>") && err.contains("push"),
        "expected Vec<bool>.push to be absent while Vec<i64>.push stays runtime-backed, got: {}",
        err
    );
}

#[test]
fn stdlib_surface_hashmap_runtime_mutators_remain_i64_only() {
    let i64_ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let map: HashMap<i64, i64> = HashMap { handle: 0, key_marker: 0, value_marker: 0 };
    if map.insert(1, 2) { 1 } else { 0 }
}
"#,
    );
    assert!(i64_ir.contains("sengoo_hashmap_insert_i64"));

    let err = compile_with_stdlib_error(
        r#"
def main() -> i64 {
    let map: HashMap<bool, bool> = HashMap { handle: 0, key_marker: false, value_marker: false };
    map.insert(true, false);
    0
}
"#,
    );

    assert!(
        err.contains("HashMap<bool,bool>") && err.contains("insert"),
        "expected HashMap<bool, bool>.insert to be absent while HashMap<i64, i64>.insert stays runtime-backed, got: {}",
        err
    );
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
