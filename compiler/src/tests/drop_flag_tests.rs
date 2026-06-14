use crate::mir::{Instruction, Local, MirConstant, MirFunction, Terminator};
use crate::{compile_to_ir, compile_to_mir};
use std::fs;
use std::path::Path;

fn load_stdlib(modules: &[&str]) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_root = manifest_dir
        .parent()
        .unwrap_or(manifest_dir)
        .join("tools")
        .join("stdlib");
    modules
        .iter()
        .map(|module| {
            fs::read_to_string(stdlib_root.join(module))
                .unwrap_or_else(|err| panic!("failed to read {module}: {err}"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn compile_with_owned_string(source: &str) -> Vec<MirFunction> {
    compile_to_mir(&format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        source
    ))
    .expect("source should compile to MIR")
}

fn function<'a>(mir_fns: &'a [MirFunction], name: &str) -> &'a MirFunction {
    mir_fns
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("expected function {name}"))
}

fn string_drop_calls(function: &MirFunction) -> Vec<Vec<Local>> {
    drop_calls(function, "String_drop")
}

fn drop_calls(function: &MirFunction, drop_func: &str) -> Vec<Vec<Local>> {
    function
        .instructions
        .iter()
        .filter_map(|inst| match inst {
            Instruction::Call { func, args, .. } if func == drop_func => Some(args.clone()),
            _ => None,
        })
        .collect()
}

fn bool_assigns(function: &MirFunction, value: bool) -> usize {
    function
        .instructions
        .iter()
        .filter(|inst| {
            matches!(
                inst,
                Instruction::Assign {
                    value: MirConstant::Bool(v),
                    ..
                } if *v == value
            )
        })
        .count()
}

fn has_guard_terminator(function: &MirFunction) -> bool {
    function
        .basic_blocks
        .iter()
        .any(|block| matches!(block.terminator, Some(Terminator::If { .. })))
}

#[test]
fn drop_glue_inserts_straight_line_string_drop_without_flags() {
    let mir = compile_with_owned_string(
        r#"
def main() -> i64 {
    let text: String = string_from_str("hello").value;
    0
}
"#,
    );
    let main_fn = function(&mir, "main");

    let calls = string_drop_calls(main_fn);
    assert_eq!(calls.len(), 1, "main should drop the owned String once");
    assert_eq!(
        bool_assigns(main_fn, false) + bool_assigns(main_fn, true),
        0,
        "single-exit drop glue should not allocate drop flags"
    );
}

#[test]
fn user_drop_impl_inserts_trait_drop_call_at_exit() {
    let mir =
        compile_to_mir(user_drop_impl_source()).expect("user Drop impl should compile to MIR");
    let main_fn = function(&mir, "main");

    let calls = drop_calls(main_fn, "Resource_Drop_drop");
    assert_eq!(
        calls.len(),
        1,
        "main should drop the Resource binding through the Drop impl once"
    );
}

#[test]
fn user_drop_impl_codegen_emits_void_drop_call() {
    let ir = compile_to_ir(user_drop_impl_source()).expect("user Drop impl should compile to IR");
    assert!(
        ir.contains("call void @Resource_Drop_drop("),
        "IR should call the user Drop impl as a void destructor, got:\n{ir}"
    );
}

fn user_drop_impl_source() -> &'static str {
    r#"
struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
    }
}

def main() -> i64 {
    let resource: Resource = Resource { handle: 7 };
    0
}
"#
}

#[test]
fn drop_flags_guard_question_mark_early_return_for_initialized_binding() {
    let mir = compile_with_owned_string(
        r#"
def may_fail() -> Result<i64, i64> {
    result_err_i64(9)
}

def checked() -> Result<i64, i64> {
    let text: String = string_from_str("before").value;
    let value = may_fail()?;
    result_ok_i64(value)
}
"#,
    );
    let checked = function(&mir, "checked");

    let calls = string_drop_calls(checked);
    assert_eq!(
        calls.len(),
        2,
        "both the ? early-return exit and success exit should run guarded drop glue"
    );
    assert_eq!(
        bool_assigns(checked, false),
        1,
        "one drop flag should be initialized false at function entry"
    );
    assert_eq!(
        bool_assigns(checked, true),
        1,
        "the drop flag should be set true after the owned binding initializes"
    );
    assert!(
        has_guard_terminator(checked),
        "multi-exit drop glue should guard drop calls with the drop flag"
    );
}

#[test]
fn drop_flags_do_not_drop_binding_initialized_after_question_mark_on_early_path() {
    let mir = compile_with_owned_string(
        r#"
def may_fail() -> Result<i64, i64> {
    result_err_i64(9)
}

def checked() -> Result<i64, i64> {
    let value = may_fail()?;
    let text: String = string_from_str("after").value;
    result_ok_i64(value)
}
"#,
    );
    let checked = function(&mir, "checked");

    assert_eq!(
        string_drop_calls(checked).len(),
        2,
        "the later binding appears in both exit chains but must be flag-guarded"
    );
    assert_eq!(bool_assigns(checked, false), 1);
    assert_eq!(bool_assigns(checked, true), 1);
}

#[test]
fn drop_glue_drops_multiple_bindings_in_reverse_order() {
    let mir = compile_with_owned_string(
        r#"
def main() -> i64 {
    let first: String = string_from_str("first").value;
    let second: String = string_from_str("second").value;
    0
}
"#,
    );
    let main_fn = function(&mir, "main");

    let calls = string_drop_calls(main_fn);
    assert_eq!(calls.len(), 2);
    assert_ne!(
        calls[0][0], calls[1][0],
        "drops should target distinct locals in reverse declaration order"
    );

    let return_block = main_fn
        .basic_blocks
        .iter()
        .find(|block| matches!(block.terminator, Some(Terminator::Return(_))))
        .expect("main should have a return block");
    let call_order = return_block
        .instructions
        .iter()
        .filter_map(|id| match main_fn.instruction(*id) {
            Instruction::Call { func, args, .. } if func == "String_drop" => Some(args[0]),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        call_order,
        vec![calls[0][0], calls[1][0]],
        "straight-line drop calls should be inserted immediately before return"
    );
}

#[test]
fn moved_owned_binding_is_excluded_from_drop_glue() {
    let mir = compile_with_owned_string(
        r#"
def main() -> i64 {
    let first: String = string_from_str("first").value;
    let second: String = first;
    0
}
"#,
    );
    let main_fn = function(&mir, "main");

    assert_eq!(
        string_drop_calls(main_fn).len(),
        1,
        "the moved-from binding should not be dropped a second time"
    );
}

#[test]
fn assignment_moved_owned_binding_is_excluded_from_drop_glue() {
    let mir = compile_with_owned_string(
        r#"
def main() -> i64 {
    let first: String = string_from_str("first").value;
    let mut second: String = string_from_str("second").value;
    second = first;
    0
}
"#,
    );
    let main_fn = function(&mir, "main");

    let calls = string_drop_calls(main_fn);
    assert_eq!(
        calls.len(),
        2,
        "assignment should drop the overwritten target once and the reinitialized target at exit"
    );
    assert_eq!(
        calls[0][0], calls[1][0],
        "assignment drop glue should target the reassigned local, not the moved-from source"
    );
}

#[test]
fn conditional_init_uses_flags_even_with_single_function_return() {
    let mir = compile_with_owned_string(
        r#"
def choose(flag: bool) -> i64 {
    if flag {
        let text: String = string_from_str("branch").value;
        text.len()
    } else {
        0
    }
}
"#,
    );
    let choose = function(&mir, "choose");

    assert_eq!(
        string_drop_calls(choose).len(),
        1,
        "the branch-local String should still be dropped on the initialized path"
    );
    assert_eq!(
        bool_assigns(choose, false),
        1,
        "conditional initialization needs a false entry flag"
    );
    assert_eq!(
        bool_assigns(choose, true),
        1,
        "the flag should be set only after the branch-local String initializes"
    );
    assert!(
        has_guard_terminator(choose),
        "single-return conditional init must use guarded drop glue"
    );
}

#[test]
fn returned_owned_binding_is_not_dropped_before_return() {
    let mir = compile_with_owned_string(
        r#"
def make_text() -> String {
    let text: String = string_from_str("return").value;
    text
}
"#,
    );
    let make_text = function(&mir, "make_text");

    assert_eq!(
        string_drop_calls(make_text).len(),
        0,
        "returning an owned binding moves it out of the function"
    );
}

#[test]
fn named_call_owned_argument_is_excluded_from_caller_drop_glue() {
    let mir = compile_with_owned_string(
        r#"
def consume(value: String) -> i64 {
    value.len()
}

def main() -> i64 {
    let text: String = string_from_str("call").value;
    consume(text)
}
"#,
    );
    let main_fn = function(&mir, "main");

    assert_eq!(
        string_drop_calls(main_fn).len(),
        0,
        "passing an owned binding to a named call moves it out of the caller"
    );
}

#[test]
fn method_owned_argument_is_excluded_from_caller_drop_glue() {
    let mir = compile_with_owned_string(
        r#"
def main() -> i64 {
    let left: String = string_from_str("left").value;
    let right: String = string_from_str("right").value;
    if left.eq(right) {
        1
    } else {
        0
    }
}
"#,
    );
    let main_fn = function(&mir, "main");

    assert_eq!(
        string_drop_calls(main_fn).len(),
        1,
        "method call arguments are move-checked by typeck and should not be dropped again by the caller"
    );
}

#[test]
fn explicit_drop_method_consumes_receiver_for_drop_glue() {
    let mir = compile_with_owned_string(
        r#"
def main() -> i64 {
    let text: String = string_from_str("drop").value;
    text.drop();
    0
}
"#,
    );
    let main_fn = function(&mir, "main");

    assert_eq!(
        string_drop_calls(main_fn).len(),
        1,
        "the explicit String.drop() call should be the only drop for the receiver"
    );
}

#[test]
fn explicit_return_creates_distinct_function_exit() {
    let mir = compile_with_owned_string(
        r#"
def choose(flag: bool) -> i64 {
    if flag {
        return 7;
    }
    9
}
"#,
    );
    let choose = function(&mir, "choose");

    let return_count = choose
        .basic_blocks
        .iter()
        .filter(|block| matches!(block.terminator, Some(Terminator::Return(_))))
        .count();
    assert_eq!(
        return_count, 2,
        "explicit return should lower to its own MIR return exit"
    );
}

#[test]
fn explicit_return_runs_drop_glue_for_live_owned_binding() {
    let mir = compile_with_owned_string(
        r#"
def choose(flag: bool) -> i64 {
    let text: String = string_from_str("return").value;
    if flag {
        return text.len();
    }
    0
}
"#,
    );
    let choose = function(&mir, "choose");

    assert_eq!(
        string_drop_calls(choose).len(),
        2,
        "both the explicit return and the fallthrough return should drop the live String"
    );
    assert!(
        has_guard_terminator(choose),
        "multi-exit drop glue should guard explicit-return exits"
    );
}
