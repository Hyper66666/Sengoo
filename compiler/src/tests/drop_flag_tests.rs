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
    function
        .instructions
        .iter()
        .filter_map(|inst| match inst {
            Instruction::Call { func, args, .. } if func == "String_Drop_drop" => {
                Some(args.clone())
            }
            _ => None,
        })
        .collect()
}

fn named_drop_calls(function: &MirFunction, name: &str) -> Vec<Vec<Local>> {
    function
        .instructions
        .iter()
        .filter_map(|inst| match inst {
            Instruction::Call { func, args, .. } if func == name => Some(args.clone()),
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

fn extracted_field_index(function: &MirFunction, local: Local) -> Option<u32> {
    function.instructions.iter().find_map(|inst| match inst {
        Instruction::Extract {
            destination, index, ..
        } if *destination == local => Some(*index),
        _ => None,
    })
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
            Instruction::Call { func, args, .. } if func == "String_Drop_drop" => Some(args[0]),
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
fn composite_owning_fields_are_dropped_in_reverse_declaration_order() {
    let mir = compile_with_owned_string(
        r#"
struct Pair {
    left: String,
    right: String,
}

def main() -> i64 {
    let pair = Pair {
        left: string_from_str("left").value,
        right: string_from_str("right").value,
    };
    0
}
"#,
    );
    let main_fn = function(&mir, "main");
    let field_order = string_drop_calls(main_fn)
        .iter()
        .filter_map(|args| extracted_field_index(main_fn, args[0]))
        .collect::<Vec<_>>();

    assert_eq!(
        field_order,
        vec![1, 0],
        "composite fields should be dropped in reverse declaration order"
    );
}

#[test]
fn partial_move_drops_only_the_remaining_composite_field() {
    let mir = compile_with_owned_string(
        r#"
struct Pair {
    left: String,
    right: String,
}

def main() -> i64 {
    let pair = Pair {
        left: string_from_str("left").value,
        right: string_from_str("right").value,
    };
    let moved = pair.left;
    0
}
"#,
    );
    let main_fn = function(&mir, "main");
    let field_order = string_drop_calls(main_fn)
        .iter()
        .filter_map(|args| extracted_field_index(main_fn, args[0]))
        .collect::<Vec<_>>();

    assert_eq!(
        field_order,
        vec![1],
        "the moved-out field must be skipped while the sibling field is dropped"
    );
    assert_eq!(
        string_drop_calls(main_fn).len(),
        2,
        "the moved binding and the one remaining field should each be dropped once"
    );
}

#[test]
fn field_moved_into_call_is_not_dropped_by_the_caller() {
    let mir = compile_with_owned_string(
        r#"
struct Pair {
    left: String,
    right: String,
}

def consume(value: String) -> i64 {
    value.len()
}

def main() -> i64 {
    let pair = Pair {
        left: string_from_str("left").value,
        right: string_from_str("right").value,
    };
    consume(pair.left)
}
"#,
    );
    let main_fn = function(&mir, "main");
    let field_order = string_drop_calls(main_fn)
        .iter()
        .filter_map(|args| extracted_field_index(main_fn, args[0]))
        .collect::<Vec<_>>();

    assert_eq!(
        field_order,
        vec![1],
        "only the sibling field should remain owned by the caller"
    );
    assert_eq!(string_drop_calls(main_fn).len(), 1);
}

#[test]
fn returned_field_is_not_dropped_before_leaving_the_function() {
    let mir = compile_with_owned_string(
        r#"
struct Pair {
    left: String,
    right: String,
}

def take_left() -> String {
    let pair = Pair {
        left: string_from_str("left").value,
        right: string_from_str("right").value,
    };
    pair.left
}
"#,
    );
    let take_left = function(&mir, "take_left");
    let field_order = string_drop_calls(take_left)
        .iter()
        .filter_map(|args| extracted_field_index(take_left, args[0]))
        .collect::<Vec<_>>();

    assert_eq!(
        field_order,
        vec![1],
        "the returned field must move out while its sibling is dropped"
    );
    assert_eq!(string_drop_calls(take_left).len(), 1);
}

#[test]
fn reinitialized_moved_field_is_dropped_again_at_scope_exit() {
    let mir = compile_with_owned_string(
        r#"
struct Pair {
    left: String,
    right: String,
}

def main() -> i64 {
    let mut pair = Pair {
        left: string_from_str("left").value,
        right: string_from_str("right").value,
    };
    let moved = pair.left;
    pair.left = string_from_str("replacement").value;
    0
}
"#,
    );
    let main_fn = function(&mir, "main");
    let field_order = string_drop_calls(main_fn)
        .iter()
        .filter_map(|args| extracted_field_index(main_fn, args[0]))
        .collect::<Vec<_>>();

    assert_eq!(
        field_order,
        vec![1, 0],
        "both fields should be live again and dropped in reverse order"
    );
    assert_eq!(
        string_drop_calls(main_fn).len(),
        3,
        "the moved binding plus both reconstituted fields should be dropped"
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
fn conditional_init_drops_at_branch_exit_without_function_flag() {
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
        bool_assigns(choose, false) + bool_assigns(choose, true),
        0,
        "lexical branch cleanup should not allocate a function-lifetime drop flag"
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
        named_drop_calls(main_fn, "String_drop").len(),
        1,
        "the explicit String.drop() call should release the receiver through the compatibility method"
    );
    assert_eq!(
        string_drop_calls(main_fn).len(),
        0,
        "explicit String.drop() marks the receiver moved so auto-drop is suppressed"
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

#[test]
fn user_drop_impl_is_called_for_live_owning_local() {
    let mir = compile_to_mir(
        r#"
struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
    }
}

def main() -> i64 {
    let resource = Resource { handle: 1 };
    0
}
"#,
    )
    .expect("user Drop type should compile to MIR");
    let main_fn = function(&mir, "main");

    assert_eq!(
        named_drop_calls(main_fn, "Resource_Drop_drop").len(),
        1,
        "a live user Drop value must call its concrete trait impl at function exit"
    );
}

#[test]
fn user_drop_impl_auto_drop_codegen_uses_void_drop_call() {
    let ir = compile_to_ir(
        r#"
struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
    }
}

def main() -> i64 {
    let resource = Resource { handle: 1 };
    0
}
"#,
    )
    .expect("user Drop auto-drop should lower through LLVM codegen");

    assert!(
        ir.contains("call void @Resource_Drop_drop("),
        "main should call the concrete Drop impl with its lowered ABI:\n{ir}"
    );
}

#[test]
fn user_drop_impl_is_called_for_by_value_parameter() {
    let mir = compile_to_mir(
        r#"
struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
    }
}

def consume(value: Resource) -> i64 {
    value.handle
}
"#,
    )
    .expect("user Drop parameter should compile to MIR");
    let consume = function(&mir, "consume");

    assert_eq!(
        named_drop_calls(consume, "Resource_Drop_drop").len(),
        1,
        "by-value owning parameters should be dropped by the callee"
    );
}

#[test]
fn drop_impl_does_not_recursively_drop_its_receiver() {
    let mir = compile_to_mir(
        r#"
struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
    }
}
"#,
    )
    .expect("user Drop impl should compile to MIR");
    let drop_fn = function(&mir, "Resource_Drop_drop");

    assert_eq!(
        named_drop_calls(drop_fn, "Resource_Drop_drop").len(),
        0,
        "Drop::drop must not recursively auto-drop its own receiver"
    );
}

#[test]
fn nested_if_scope_drops_before_joining_outer_control_flow() {
    let mir = compile_to_mir(
        r#"
struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
    }
}

def after_scope() -> i64 {
    7
}

def main(flag: bool) -> i64 {
    if flag {
        let resource = Resource { handle: 1 };
        resource.handle
    } else {
        0
    };
    after_scope()
}
"#,
    )
    .expect("nested owning scope should compile to MIR");
    let main_fn = function(&mir, "main");
    let then_block = main_fn
        .basic_blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(Terminator::If { then_block, .. }) => Some(then_block),
            _ => None,
        })
        .expect("main should contain the source if branch");

    assert!(
        main_fn.basic_blocks[then_block]
            .instructions
            .iter()
            .any(|id| matches!(
                main_fn.instruction(*id),
                Instruction::Call { func, .. } if func == "Resource_Drop_drop"
            )),
        "the branch-local Resource must be dropped before the then branch joins outer control flow"
    );
}

#[test]
fn nested_if_scope_drops_before_explicit_return() {
    let mir = compile_to_mir(
        r#"
struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
    }
}

def main(flag: bool) -> i64 {
    if flag {
        let resource = Resource { handle: 1 };
        return resource.handle;
    }
    0
}
"#,
    )
    .expect("nested return cleanup should compile to MIR");
    let main_fn = function(&mir, "main");
    let explicit_return_block = main_fn
        .basic_blocks
        .iter()
        .find(|block| {
            matches!(block.terminator, Some(Terminator::Return(Some(_))))
                && block.instructions.iter().any(|id| {
                    matches!(
                        main_fn.instruction(*id),
                        Instruction::Store { destination, .. }
                            if matches!(
                                main_fn.locals.get(destination.index()),
                                Some((_, crate::mir::MIRType::Struct { name, .. })) if name == "Resource"
                            )
                    )
                })
        })
        .expect("then branch should contain the explicit return");

    assert!(
        explicit_return_block.instructions.iter().any(|id| matches!(
            main_fn.instruction(*id),
            Instruction::Call { func, .. } if func == "Resource_Drop_drop"
        )),
        "branch-local Resource must be dropped before its explicit return"
    );
}

#[test]
fn loop_scope_drops_before_break() {
    let mir = compile_to_mir(
        r#"
struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
    }
}

def main() -> i64 {
    loop {
        let resource = Resource { handle: 1 };
        break;
    }
    0
}
"#,
    )
    .expect("loop break cleanup should compile to MIR");
    let main_fn = function(&mir, "main");
    let break_block = main_fn
        .basic_blocks
        .iter()
        .find(|block| matches!(block.terminator, Some(Terminator::Break { .. })))
        .expect("loop body should contain a break terminator");

    assert!(
        break_block.instructions.iter().any(|id| matches!(
            main_fn.instruction(*id),
            Instruction::Call { func, .. } if func == "Resource_Drop_drop"
        )),
        "loop-local Resource must be dropped before break"
    );
}

#[test]
fn while_scope_drops_before_continue() {
    let mir = compile_to_mir(
        r#"
struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
    }
}

def main(flag: bool) -> i64 {
    while flag {
        let resource = Resource { handle: 1 };
        continue;
    }
    0
}
"#,
    )
    .expect("while continue cleanup should compile to MIR");
    let main_fn = function(&mir, "main");
    let continue_block = main_fn
        .basic_blocks
        .iter()
        .find(|block| matches!(block.terminator, Some(Terminator::Continue { .. })))
        .expect("while body should contain a continue terminator");

    assert!(
        continue_block.instructions.iter().any(|id| matches!(
            main_fn.instruction(*id),
            Instruction::Call { func, .. } if func == "Resource_Drop_drop"
        )),
        "while-local Resource must be dropped before continue"
    );
}

#[test]
fn nested_scope_drops_before_question_mark_return() {
    let mir = compile_to_mir(
        r#"
struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
    }
}

struct Result<T, E> {
    is_ok: bool,
    value: T,
    error: E,
}

def may_fail() -> Result<i64, i64> {
    Result { is_ok: false, value: 0, error: 9 }
}

def checked(flag: bool) -> Result<i64, i64> {
    if flag {
        let resource = Resource { handle: 1 };
        let value = may_fail()?;
        value + resource.handle
    } else {
        0
    };
    Result { is_ok: true, value: 0, error: 0 }
}
"#,
    )
    .expect("nested question-mark cleanup should compile to MIR");
    let checked = function(&mir, "checked");
    let fail_return = checked.basic_blocks.iter().find(|block| {
        matches!(block.terminator, Some(Terminator::Return(Some(_))))
            && block.instructions.iter().any(|id| {
                matches!(
                    checked.instruction(*id),
                    Instruction::Call { func, .. } if func == "Resource_Drop_drop"
                )
            })
    });

    assert!(
        fail_return.is_some(),
        "the `?` failure return must drop the branch-local Resource first"
    );
}

#[test]
fn explicit_block_scope_drops_before_following_statement() {
    let mir = compile_to_mir(
        r#"
struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
    }
}

def after_scope() -> i64 {
    7
}

def main() -> i64 {
    {
        let resource = Resource { handle: 1 };
        resource.handle
    };
    after_scope()
}
"#,
    )
    .expect("explicit block cleanup should compile to MIR");
    let main_fn = function(&mir, "main");
    let block_with_resource = main_fn
        .basic_blocks
        .iter()
        .find(|block| {
            block.instructions.iter().any(|id| {
                matches!(
                    main_fn.instruction(*id),
                    Instruction::Store { destination, .. }
                        if matches!(
                            main_fn.locals.get(destination.index()),
                            Some((_, crate::mir::MIRType::Struct { name, .. })) if name == "Resource"
                        )
                )
            })
        })
        .expect("explicit block should contain the Resource binding");

    assert!(
        block_with_resource.instructions.iter().any(|id| matches!(
            main_fn.instruction(*id),
            Instruction::Call { func, .. } if func == "Resource_Drop_drop"
        )),
        "explicit block-local Resource must be dropped before later statements"
    );
}

#[test]
fn try_block_scope_drops_on_success_and_question_mark_failure() {
    let mir = compile_to_mir(
        r#"
struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
    }
}

struct Result<T, E> {
    is_ok: bool,
    value: T,
    error: E,
}

def may_fail() -> Result<i64, i64> {
    Result { is_ok: false, value: 0, error: 9 }
}

def main() -> i64 {
    let outcome = try {
        let resource = Resource { handle: 1 };
        let value = may_fail()?;
        value + resource.handle
    };
    if outcome.is_ok {
        outcome.value
    } else {
        0
    }
}
"#,
    )
    .expect("try-block cleanup should compile to MIR");
    let main_fn = function(&mir, "main");

    assert_eq!(
        named_drop_calls(main_fn, "Resource_Drop_drop").len(),
        2,
        "try-block Resource must be dropped on both success and propagated failure paths"
    );
}
