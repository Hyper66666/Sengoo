use crate::ast::DeclKind;
use crate::mir::{Instruction, LocalKind};
use crate::{compile_to_ir, compile_to_mir, Parser};

#[test]
fn parser_marks_async_def_as_async_function() {
    let source = r#"
async def compute() -> i64 {
    1
}
"#;

    let program = Parser::parse(source).expect("parser should accept async def");
    let first_decl = program.decls.first().expect("program should have one decl");
    let DeclKind::Function(function) = &first_decl.kind else {
        panic!("expected function declaration");
    };
    assert!(function.is_async, "function should be marked as async");
}

#[test]
fn async_keyword_on_non_function_decl_is_rejected() {
    let source = r#"
async const FLAG: i64 = 1;
"#;

    let err = Parser::parse(source).expect_err("async should only be valid on function decls");
    let msg = err.to_string();
    assert!(
        msg.contains("`async` is only supported on function declarations"),
        "error should explain async placement rule, got: {}",
        msg
    );
}

#[test]
fn await_outside_async_context_is_rejected() {
    let source = r#"
async def helper() -> i64 { 42 }
def main() -> i64 {
    let value = await helper();
    value
}
"#;

    let err = compile_to_ir(source).expect_err("await outside async should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("await is only allowed in async contexts"),
        "error should mention async context restriction, got: {}",
        msg
    );
}

#[test]
fn await_on_non_future_is_rejected() {
    let source = r#"
async def main() -> i64 {
    let value = await 1;
    value
}
"#;

    let err = compile_to_ir(source).expect_err("await on non-future should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("await requires a Future value"),
        "error should mention Future requirement, got: {}",
        msg
    );
}

#[test]
fn async_function_with_await_compiles() {
    let source = r#"
async def add_one(x: i64) -> i64 {
    x + 1
}
async def main() -> i64 {
    let result = await add_one(41);
    result
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "async/await function should compile, got: {:?}",
        result.err()
    );
}

#[test]
fn async_block_direct_await_compiles() {
    let source = r#"
async def main() -> i64 {
    let value = await async { 42 };
    value
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "direct await on async block should compile, got: {:?}",
        result.err()
    );
}

#[test]
fn async_block_captures_outer_local_and_can_be_awaited_via_binding() {
    let source = r#"
async def main() -> i64 {
    let base = 41;
    let fut = async { base + 1 };
    let value = await fut;
    value
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "async block with captured local should compile, got: {:?}",
        result.err()
    );
}

#[test]
fn sleep_builtin_requires_async_context() {
    let source = r#"
def main() -> i64 {
    sleep(1);
    0
}
"#;

    let err = compile_to_ir(source).expect_err("sleep outside async should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("sleep is only allowed in async contexts"),
        "error should mention async context restriction, got: {}",
        msg
    );
}

#[test]
fn sleep_builtin_can_be_awaited() {
    let source = r#"
async def main() -> i64 {
    await sleep(1);
    42
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "sleep builtin should compile in async contexts, got: {:?}",
        result.err()
    );
}

#[test]
fn sleep_future_binding_can_be_awaited() {
    let source = r#"
async def main() -> i64 {
    let fut = sleep(1);
    await fut;
    42
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "sleep future bindings should remain awaitable, got: {:?}",
        result.err()
    );
}

#[test]
fn sleep_lowering_emits_runtime_sleep_start_call() {
    let source = r#"
async def main() -> i64 {
    await sleep(1);
    0
}
"#;

    let mir_fns = compile_to_mir(source).expect("sleep source should lower to MIR");
    let has_sleep_start = mir_fns.iter().any(|mir_fn| {
        mir_fn.instructions.iter().any(|inst| match inst {
            Instruction::Call { func, .. } => func == "sengoo_async_sleep__start",
            _ => false,
        })
    });

    assert!(
        has_sleep_start,
        "sleep lowering should emit a runtime sleep start call"
    );
}

#[test]
fn timeout_builtin_requires_async_context() {
    let source = r#"
async def helper() -> i64 { 1 }
def main() -> i64 {
    timeout(helper(), 1);
    0
}
"#;

    let err = compile_to_ir(source).expect_err("timeout outside async should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("timeout is only allowed in async contexts"),
        "error should mention async context restriction, got: {}",
        msg
    );
}

#[test]
fn timeout_builtin_returns_bool_for_future_readiness() {
    let source = r#"
async def helper() -> i64 { 1 }
async def main() -> i64 {
    let fut = helper();
    let ready = await timeout(fut, 1);
    if ready { await fut } else { 0 }
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "timeout builtin should compile in async contexts, got: {:?}",
        result.err()
    );
}

#[test]
fn timeout_future_binding_can_be_awaited() {
    let source = r#"
async def child() -> i64 { 1 }
async def main() -> i64 {
    let fut = timeout(child(), 1);
    if await fut { 1 } else { 0 }
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "timeout future bindings should remain awaitable, got: {:?}",
        result.err()
    );
}

#[test]
fn timeout_lowering_emits_runtime_timeout_call() {
    let source = r#"
async def helper() -> i64 { 1 }
async def main() -> i64 {
    let fut = helper();
    let ready = await timeout(fut, 1);
    if ready { await fut } else { 0 }
}
"#;

    let mir_fns = compile_to_mir(source).expect("timeout source should lower to MIR");
    let has_timeout_start = mir_fns.iter().any(|mir_fn| {
        mir_fn.instructions.iter().any(|inst| match inst {
            Instruction::Call { func, .. } => func == "sengoo_async_timeout_bool__start",
            _ => false,
        })
    });

    assert!(
        has_timeout_start,
        "timeout lowering should emit a timeout future start call"
    );
}

#[test]
fn spawn_builtin_returns_awaitable_future() {
    let source = r#"
async def add_one(x: i64) -> i64 { x + 1 }
async def slow_step() -> i64 { 0 }
async def slow() -> i64 {
    let first = await slow_step();
    let second = await slow_step();
    0
}
async def main() -> i64 {
    let task = spawn(add_one(41));
    let waited = await slow();
    await task
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "spawn builtin should return an awaitable future, got: {:?}",
        result.err()
    );
}

#[test]
fn spawn_task_builtin_returns_task_id() {
    let source = r#"
async def child() -> i64 { 7 }
async def main() -> i64 {
    let task = spawn_task(child());
    task_status(task)
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "spawn_task should return a task id usable with task_status, got: {:?}",
        result.err()
    );
}

#[test]
fn spawn_task_builtin_requires_async_context() {
    let source = r#"
async def child() -> i64 { 1 }
def main() -> i64 {
    let task = spawn_task(child());
    task
}
"#;

    let err = compile_to_ir(source).expect_err("spawn_task outside async should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("spawn_task is only allowed in async contexts"),
        "error should mention async context restriction, got: {}",
        msg
    );
}

#[test]
fn spawn_task_builtin_rejects_non_future_argument() {
    let source = r#"
async def main() -> i64 {
    let task = spawn_task(1);
    task
}
"#;

    let err = compile_to_ir(source).expect_err("spawn_task should reject non-future input");
    let msg = err.to_string();
    assert!(
        msg.contains("spawn_task requires a Future value"),
        "error should mention Future requirement, got: {}",
        msg
    );
}

#[test]
fn cancel_task_builtin_requires_async_context() {
    let source = r#"
def main() -> i64 {
    let task: i64 = 1;
    if cancel_task(task) { 1 } else { 0 }
}
"#;

    let err = compile_to_ir(source).expect_err("cancel_task outside async should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("cancel_task is only allowed in async contexts"),
        "error should mention async context restriction, got: {}",
        msg
    );
}

#[test]
fn cancel_task_builtin_rejects_non_i64_task_id() {
    let source = r#"
async def main() -> i64 {
    if cancel_task(true) { 1 } else { 0 }
}
"#;

    let err = compile_to_ir(source).expect_err("cancel_task should reject non-i64 task ids");
    let msg = err.to_string();
    assert!(
        msg.contains("type mismatch")
            || msg.contains("cannot unify")
            || msg.contains("expected")
            || msg.contains("类型不匹配"),
        "unexpected cancel_task task-id diagnostic: {}",
        msg
    );
}

#[test]
fn task_status_builtin_requires_async_context() {
    let source = r#"
def main() -> i64 {
    let task: i64 = 1;
    task_status(task)
}
"#;

    let err = compile_to_ir(source).expect_err("task_status outside async should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("task_status is only allowed in async contexts"),
        "error should mention async context restriction, got: {}",
        msg
    );
}

#[test]
fn task_status_builtin_rejects_non_i64_task_id() {
    let source = r#"
async def main() -> i64 {
    task_status(false)
}
"#;

    let err = compile_to_ir(source).expect_err("task_status should reject non-i64 task ids");
    let msg = err.to_string();
    assert!(
        msg.contains("type mismatch")
            || msg.contains("cannot unify")
            || msg.contains("expected")
            || msg.contains("类型不匹配"),
        "unexpected task_status task-id diagnostic: {}",
        msg
    );
}

#[test]
fn spawn_lowering_emits_runtime_spawn_call() {
    let source = r#"
async def add_one(x: i64) -> i64 { x + 1 }
async def main() -> i64 {
    let task = spawn(add_one(41));
    await task
}
"#;

    let mir_fns = compile_to_mir(source).expect("spawn source should lower to MIR");
    let has_spawn_call = mir_fns.iter().any(|mir_fn| {
        mir_fn.instructions.iter().any(|inst| match inst {
            Instruction::Call { func, .. } => func == "sengoo_async_spawn_raw",
            _ => false,
        })
    });

    assert!(
        has_spawn_call,
        "spawn lowering should emit a runtime spawn call"
    );
}

#[test]
fn task_lifecycle_lowering_emits_runtime_calls() {
    let source = r#"
async def child() -> i64 { 7 }
async def main() -> i64 {
    let task = spawn_task(child());
    let canceled = cancel_task(task);
    if canceled { task_status(task) } else { 0 }
}
"#;

    let mir_fns = compile_to_mir(source).expect("task lifecycle source should lower to MIR");
    let mut call_names = std::collections::HashSet::new();
    for mir_fn in &mir_fns {
        for inst in &mir_fn.instructions {
            if let Instruction::Call { func, .. } = inst {
                call_names.insert(func.clone());
            }
        }
    }

    assert!(call_names.contains("sengoo_async_spawn_raw"));
    assert!(call_names.contains("sengoo_async_cancel_task"));
    assert!(call_names.contains("sengoo_async_task_status"));
}

#[test]
fn task_lifecycle_ir_contains_runtime_declarations() {
    let source = r#"
async def child() -> i64 { 7 }
async def main() -> i64 {
    let task = spawn_task(child());
    let canceled = cancel_task(task);
    if canceled { task_status(task) } else { 0 }
}
"#;

    let ir = compile_to_ir(source).expect("task lifecycle source should compile to IR");
    assert!(
        ir.contains("declare i64 @sengoo_async_spawn_raw(i64, i64)"),
        "IR should declare spawn_task runtime helper"
    );
    assert!(
        ir.contains("declare i1 @sengoo_async_cancel_task(i64)"),
        "IR should declare cancel_task runtime helper"
    );
    assert!(
        ir.contains("declare i64 @sengoo_async_task_status(i64)"),
        "IR should declare task_status runtime helper"
    );
}

#[test]
fn join_builtin_waits_spawned_futures_and_returns_unit() {
    let source = r#"
async def add_one(x: i64) -> i64 { x + 1 }
async def main() -> i64 {
    let first = spawn(add_one(1));
    let second = spawn(add_one(2));
    join(first, second);
    0
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "join builtin should accept spawned futures in async contexts, got: {:?}",
        result.err()
    );
}

#[test]
fn join_lowering_collects_results_from_each_future() {
    let source = r#"
async def first_step() -> i64 { 1 }
async def second_step() -> i64 { 2 }
async def main() -> i64 {
    let first = spawn(first_step());
    let second = spawn(second_step());
    join(first, second);
    0
}
"#;

    let mir_fns = compile_to_mir(source).expect("join source should lower to MIR");
    let mut result_calls = std::collections::HashSet::new();
    for mir_fn in &mir_fns {
        for inst in &mir_fn.instructions {
            if let Instruction::Call { func, .. } = inst {
                if func.ends_with("__result") {
                    result_calls.insert(func.clone());
                }
            }
        }
    }

    assert!(
        result_calls.contains("first_step__result"),
        "join lowering should collect the first future result, got: {:?}",
        result_calls
    );
    assert!(
        result_calls.contains("second_step__result"),
        "join lowering should collect the second future result, got: {:?}",
        result_calls
    );
}

#[test]
fn select_builtin_returns_first_completed_value() {
    let source = r#"
async def fast() -> i64 { 7 }
async def slow_step() -> i64 { 0 }
async def slow() -> i64 {
    let waited = await slow_step();
    9
}
async def main() -> i64 {
    let first = spawn(fast());
    let second = spawn(slow());
    select(first, second)
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "select builtin should compile for matching future types, got: {:?}",
        result.err()
    );
}

#[test]
fn select_builtin_returns_first_completed_bool_value() {
    let source = r#"
async def fast() -> bool { true }
async def slow_step() -> i64 { 0 }
async def slow() -> bool {
    let waited = await slow_step();
    false
}
async def main() -> i64 {
    let first = spawn(fast());
    let second = spawn(slow());
    if select(first, second) { 1 } else { 0 }
}
"#;

    assert!(
        compile_to_ir(source).is_ok(),
        "select builtin should compile for bool futures"
    );
}

#[test]
fn select_builtin_returns_first_completed_i32_value() {
    let source = r#"
extern "C" {
    fn get_i32() -> i32;
}

async def fast() -> i32 { get_i32() }
async def slow() -> i32 { get_i32() }

async def main() -> i64 {
    let first = spawn(fast());
    let second = spawn(slow());
    let picked = select(first, second);
    picked + 1
}
"#;

    assert!(
        compile_to_ir(source).is_ok(),
        "select builtin should compile for i32 futures"
    );
}

#[test]
fn select_lowering_emits_winner_runtime_call_and_result_phi() {
    let source = r#"
async def first_step() -> i64 { 1 }
async def second_step() -> i64 { 2 }
async def main() -> i64 {
    let first = spawn(first_step());
    let second = spawn(second_step());
    select(first, second)
}
"#;

    let mir_fns = compile_to_mir(source).expect("select source should lower to MIR");
    let mut saw_winner = false;
    let mut saw_first_result = false;
    let mut saw_second_result = false;
    let mut saw_phi = false;
    for mir_fn in &mir_fns {
        for inst in &mir_fn.instructions {
            match inst {
                Instruction::Call { func, .. } if func == "sengoo_async_select_winner" => {
                    saw_winner = true;
                }
                Instruction::Call { func, .. } if func == "first_step__result" => {
                    saw_first_result = true;
                }
                Instruction::Call { func, .. } if func == "second_step__result" => {
                    saw_second_result = true;
                }
                Instruction::Phi { incoming, .. } if incoming.len() == 2 => {
                    saw_phi = true;
                }
                _ => {}
            }
        }
    }

    assert!(
        saw_winner,
        "select lowering should emit the winner runtime call"
    );
    assert!(
        saw_first_result,
        "select lowering should emit the first future result call"
    );
    assert!(
        saw_second_result,
        "select lowering should emit the second future result call"
    );
    assert!(
        saw_phi,
        "select lowering should merge select results with a phi"
    );
}

#[test]
fn select_lowering_emits_winner_runtime_decl_for_bool_select() {
    let source = r#"
async def first_step() -> bool { true }
async def second_step() -> bool { false }
async def main() -> i64 {
    let first = spawn(first_step());
    let second = spawn(second_step());
    if select(first, second) { 1 } else { 0 }
}
"#;

    let llvm_ir = compile_to_ir(source).expect("bool select source should lower to LLVM IR");
    let has_select_call = llvm_ir.contains("@sengoo_async_select_winner(");

    assert!(
        has_select_call,
        "select lowering should emit the winner runtime declaration"
    );
}

#[test]
fn select_lowering_emits_winner_runtime_decl_for_f64_select() {
    let source = r#"
async def first_step() -> f64 { 3.5 }
async def second_step() -> f64 { 1.5 }
async def main() -> i64 {
    let first = spawn(first_step());
    let second = spawn(second_step());
    if select(first, second) > 3.0 { 1 } else { 0 }
}
"#;

    let llvm_ir = compile_to_ir(source).expect("f64 select source should lower to LLVM IR");
    let has_select_call = llvm_ir.contains("@sengoo_async_select_winner(");

    assert!(
        has_select_call,
        "select lowering should emit the winner runtime declaration"
    );
}

#[test]
fn select_accepts_timeout_bool_futures() {
    let source = r#"
async def worker() -> i64 {
    42
}

async def main() -> i64 {
    let first = timeout(worker(), 1);
    let second = timeout(worker(), 2);
    if select(first, second) { 1 } else { 0 }
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "select should accept timeout-produced bool futures, got: {:?}",
        result.err()
    );
}

#[test]
fn select_builtin_returns_first_completed_struct_value() {
    let source = r#"
struct Point { x: i64, y: i64 }

async def fast() -> Point { Point { x: 7, y: 9 } }
async def slow() -> Point { Point { x: 1, y: 2 } }

async def main() -> i64 {
    let first = spawn(fast());
    let second = spawn(slow());
    let picked = select(first, second);
    picked.x + picked.y
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "select builtin should compile for struct futures, got: {:?}",
        result.err()
    );
}

#[test]
fn select_builtin_non_scalar_struct_futures_lower_to_mir() {
    let source = r#"
struct Point { x: i64, y: i64 }

async def fast() -> Point { Point { x: 7, y: 9 } }
async def slow() -> Point { Point { x: 1, y: 2 } }

async def main() -> i64 {
    let first = spawn(fast());
    let second = spawn(slow());
    let picked = select(first, second);
    picked.x
}
"#;

    let result = compile_to_mir(source);
    assert!(
        result.is_ok(),
        "select builtin should lower for struct futures, got: {:?}",
        result.err()
    );
}

#[test]
fn select_rejects_mismatched_future_result_types() {
    let source = r#"
async def left() -> i64 { 1 }
async def right() -> bool { true }

async def main() -> i64 {
    let first = spawn(left());
    let second = spawn(right());
    let picked = select(first, second);
    if picked > 0 { picked } else { 0 }
}
"#;

    let err =
        compile_to_ir(source).expect_err("select should reject mismatched future result types");
    let msg = err.to_string();
    assert!(
        msg.contains("type mismatch")
            || msg.contains("cannot unify")
            || msg.contains("matching")
            || msg.contains("类型不匹配"),
        "unexpected mismatch diagnostic: {msg}"
    );
}

#[test]
fn returning_future_is_rejected() {
    let source = r#"
async def helper() -> i64 { 42 }
async def main() -> i64 {
    return helper();
    0
}
"#;

    let err = compile_to_ir(source).expect_err("returning a future should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("future values cannot escape"),
        "error should mention escape restriction, got: {}",
        msg
    );
    assert!(
        !msg.contains("phase-1"),
        "user-facing future escape diagnostics should not expose internal phase terminology, got: {}",
        msg
    );
}

#[test]
fn future_as_function_arg_is_rejected() {
    let source = r#"
async def helper() -> i64 { 42 }
def consume(x: i64) -> i64 { x }
async def main() -> i64 {
    consume(helper());
    0
}
"#;

    let err = compile_to_ir(source).expect_err("passing unawaited future as arg should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("future values cannot be passed as arguments"),
        "error should mention argument escape restriction, got: {}",
        msg
    );
}

#[test]
fn breaking_with_future_is_rejected() {
    let source = r#"
async def helper() -> i64 { 42 }
async def main() -> i64 {
    loop {
        break helper()
    }
}
"#;

    let err = compile_to_ir(source).expect_err("breaking with a future should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("future values cannot escape"),
        "error should mention escape restriction, got: {}",
        msg
    );
    assert!(
        !msg.contains("phase-1"),
        "user-facing future escape diagnostics should not expose internal phase terminology, got: {}",
        msg
    );
}

#[test]
fn phi_merged_future_binding_can_be_awaited_when_origins_match() {
    let source = r#"
async def helper() -> i64 { 42 }
async def main() -> i64 {
    let flag: bool = true;
    let fut = if flag { helper() } else { helper() };
    await fut
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "phi-merged future binding should remain awaitable, got: {:?}",
        result.err()
    );
}

#[test]
fn array_storing_future_is_rejected() {
    let source = r#"
async def helper() -> i64 { 42 }
async def main() -> i64 {
    let items = [helper()];
    0
}
"#;

    let err = compile_to_ir(source).expect_err("array storing a future should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("future values cannot escape"),
        "error should mention future escape restriction, got: {}",
        msg
    );
}

#[test]
fn struct_storing_future_is_rejected() {
    let source = r#"
struct Wrap<T> {
    value: T,
}

async def helper() -> i64 { 42 }
async def main() -> i64 {
    let wrapped = Wrap { value: helper() };
    0
}
"#;

    let err = compile_to_ir(source).expect_err("struct storing a future should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("future values cannot escape"),
        "error should mention future escape restriction, got: {}",
        msg
    );
}

#[test]
fn async_helpers_are_synthesized_in_mir() {
    let source = r#"
async def add_one(x: i64) -> i64 {
    x + 1
}
def main() -> i64 {
    0
}
"#;

    let mir_fns = compile_to_mir(source).expect("should compile");
    let names: Vec<&str> = mir_fns.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names.contains(&"add_one"),
        "original async function body should be present, got: {:?}",
        names
    );
    assert!(
        names.contains(&"add_one__start"),
        "__start helper should be synthesized, got: {:?}",
        names
    );
    assert!(
        names.contains(&"add_one__poll"),
        "__poll helper should be synthesized, got: {:?}",
        names
    );
    assert!(
        names.contains(&"add_one__result"),
        "__result helper should be synthesized, got: {:?}",
        names
    );
    assert!(
        names.contains(&"sengoo_async_cancel_dispatch"),
        "cancel dispatch helper should be synthesized, got: {:?}",
        names
    );
    assert!(
        names.contains(&"sengoo_async_drop_dispatch"),
        "drop dispatch helper should be synthesized, got: {:?}",
        names
    );
}

#[test]
fn async_main_wrapper_is_generated() {
    let source = r#"
async def main() -> i64 {
    42
}
"#;

    let mir_fns = compile_to_mir(source).expect("should compile");
    let names: Vec<&str> = mir_fns.iter().map(|f| f.name.as_str()).collect();

    assert!(
        names.contains(&"main__body"),
        "original main body should be renamed to main__body, got: {:?}",
        names
    );
    assert!(
        names.contains(&"main"),
        "async main wrapper should exist, got: {:?}",
        names
    );
    assert!(
        names.contains(&"main__start"),
        "__start helper should be present, got: {:?}",
        names
    );
    assert!(
        names.contains(&"main__poll"),
        "__poll helper should be present, got: {:?}",
        names
    );
    assert!(
        names.contains(&"main__result"),
        "__result helper should be present, got: {:?}",
        names
    );

    let main_fn = mir_fns.iter().find(|f| f.name == "main").unwrap();
    assert!(!main_fn.is_async, "wrapper main should not be marked async");

    let body_fn = mir_fns.iter().find(|f| f.name == "main__body").unwrap();
    assert!(
        body_fn.is_async,
        "body function should still be marked async"
    );
}

#[test]
fn async_function_ir_contains_helper_declarations() {
    let source = r#"
async def add_one(x: i64) -> i64 {
    x + 1
}
async def main() -> i64 {
    let result = await add_one(41);
    result
}
"#;

    let ir = compile_to_ir(source).expect("should compile to IR");
    assert!(
        ir.contains("@main__start"),
        "IR should contain main__start, got:\n{}",
        &ir[..ir.len().min(2000)]
    );
    assert!(ir.contains("@main__poll"), "IR should contain main__poll");
    assert!(
        ir.contains("@main__result"),
        "IR should contain main__result"
    );
    assert!(
        ir.contains("@sengoo_async_frame_alloc"),
        "IR should declare async frame alloc"
    );
}

#[test]
fn sleep_builtin_ir_contains_runtime_declarations() {
    let source = r#"
async def main() -> i64 {
    await sleep(1);
    0
}
"#;

    let ir = compile_to_ir(source).expect("sleep builtin should compile to IR");
    assert!(
        ir.contains("declare i64 @sengoo_async_sleep__start(i64)"),
        "IR should declare sleep start helper"
    );
    assert!(
        ir.contains("declare i64 @sengoo_async_sleep__poll(i64)"),
        "IR should declare sleep poll helper"
    );
    assert!(
        ir.contains("declare void @sengoo_async_sleep__result(i64)"),
        "IR should declare sleep result helper"
    );
    assert!(
        ir.contains("declare i1 @sengoo_async_sleep__cancel(i64)"),
        "IR should declare sleep cancel helper"
    );
    assert!(
        ir.contains("declare void @sengoo_async_sleep__drop(i64)"),
        "IR should declare sleep drop helper"
    );
}

#[test]
fn timeout_builtin_ir_contains_runtime_declarations() {
    let source = r#"
async def child() -> i64 { 1 }
async def main() -> i64 {
    let fut = timeout(child(), 1);
    if await fut { 1 } else { 0 }
}
"#;

    let ir = compile_to_ir(source).expect("timeout builtin should compile to IR");
    assert!(
        ir.contains("declare i64 @sengoo_async_timeout_bool__start(i64, i64, i64)"),
        "IR should declare timeout start helper"
    );
    assert!(
        ir.contains("declare i64 @sengoo_async_timeout_bool__poll(i64)"),
        "IR should declare timeout poll helper"
    );
    assert!(
        ir.contains("declare i1 @sengoo_async_timeout_bool__result(i64)"),
        "IR should declare timeout result helper"
    );
    assert!(
        ir.contains("declare i1 @sengoo_async_timeout_bool__cancel(i64)"),
        "IR should declare timeout cancel helper"
    );
    assert!(
        ir.contains("declare void @sengoo_async_timeout_bool__drop(i64)"),
        "IR should declare timeout drop helper"
    );
}

#[test]
fn async_helper_ir_uses_void_frame_runtime_calls() {
    let source = r#"
async def add_one(x: i64) -> i64 {
    x + 1
}

async def main() -> i64 {
    let result = await add_one(41);
    result
}
"#;

    let ir = compile_to_ir(source).expect("should compile to IR");
    assert!(
        ir.contains("call void @sengoo_async_frame_store"),
        "async frame store should be emitted as void call, got:
{}",
        &ir[..ir.len().min(4000)]
    );
    assert!(
        ir.contains("call void @sengoo_async_frame_free"),
        "async frame free should be emitted as void call, got:
{}",
        &ir[..ir.len().min(4000)]
    );
    assert!(
        !ir.contains("call i64 @sengoo_async_frame_store"),
        "async frame store should not be emitted as i64 call"
    );
    assert!(
        !ir.contains("call i64 @sengoo_async_frame_free"),
        "async frame free should not be emitted as i64 call"
    );
}

#[test]
fn multiple_sequential_awaits_compile() {
    let source = r#"
async def step1() -> i64 { 10 }
async def step2() -> i64 { 20 }
async def main() -> i64 {
    let a = await step1();
    let b = await step2();
    a + b
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "multiple sequential awaits should compile, got: {:?}",
        result.err()
    );
}

#[test]
fn local_future_binding_then_await_compiles() {
    let source = r#"
async def add_one(x: i64) -> i64 {
    x + 1
}
async def main() -> i64 {
    let f = add_one(41);
    await f
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "local future binding with await should compile, got: {:?}",
        result.err()
    );
}

#[test]
fn load_traced_future_binding_then_await_compiles() {
    let source = r#"
async def add_one(x: i64) -> i64 {
    x + 1
}
async def main() -> i64 {
    let f = add_one(41);
    let x = f;
    await x
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "future bindings routed through an extra local should remain awaitable, got: {:?}",
        result.err()
    );
}

#[test]
fn multi_await_poll_helper_polls_child_futures_without_reinvoking_body() {
    let source = r#"
async def step1() -> i64 { 10 }
async def step2(x: i64) -> i64 { x + 20 }
async def main() -> i64 {
    let a = await step1();
    let b = await step2(a);
    a + b
}
"#;

    let mir_fns = compile_to_mir(source).expect("should compile");
    let poll_fn = mir_fns
        .iter()
        .find(|f| f.name == "main__poll")
        .expect("main__poll helper should exist");

    let call_names = poll_fn
        .instructions
        .iter()
        .filter_map(|inst| match inst {
            Instruction::Call { func, .. } => Some(func.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        call_names.contains(&"step1__poll"),
        "main__poll should poll step1 future, got calls: {:?}",
        call_names
    );
    assert!(
        call_names.contains(&"step2__poll"),
        "main__poll should poll step2 future, got calls: {:?}",
        call_names
    );
    assert!(
        !call_names.contains(&"main__body"),
        "main__poll should not re-enter main__body once state-machine lowering is in place, got calls: {:?}",
        call_names
    );
}

#[test]
fn multi_await_poll_helper_resolves_bound_future_base_names() {
    let source = r#"
async def step1() -> i64 { 10 }
async def step2() -> i64 { 20 }
async def main() -> i64 {
    let first = step1();
    let second = step2();
    let a = await first;
    let b = await second;
    a + b
}
"#;

    let mir_fns = compile_to_mir(source).expect("should compile");
    let poll_fn = mir_fns
        .iter()
        .find(|f| f.name == "main__poll")
        .expect("main__poll helper should exist");

    let call_names = poll_fn
        .instructions
        .iter()
        .filter_map(|inst| match inst {
            Instruction::Call { func, .. } => Some(func.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        call_names.contains(&"step1__poll"),
        "main__poll should resolve the first bound future base name, got calls: {:?}",
        call_names
    );
    assert!(
        call_names.contains(&"step2__poll"),
        "main__poll should resolve the second bound future base name, got calls: {:?}",
        call_names
    );
    assert!(
        !call_names.contains(&"unknown__poll"),
        "bound future resolution should not fall back to unknown__poll, got calls: {:?}",
        call_names
    );
}

#[test]
fn multi_await_poll_helper_only_spills_live_user_locals() {
    let source = r#"
async def step1() -> i64 { 10 }
async def step2(x: i64) -> i64 { x + 20 }
async def main() -> i64 {
    let dead = 99;
    let a = await step1();
    let b = await step2(a);
    b
}
"#;

    let mir_fns = compile_to_mir(source).expect("should compile");
    let poll_fn = mir_fns
        .iter()
        .find(|f| f.name == "main__poll")
        .expect("main__poll helper should exist");

    let spilled_user_loads = poll_fn
        .instructions
        .iter()
        .filter_map(|inst| match inst {
            Instruction::Load { source, .. } if source.kind == LocalKind::User => Some(*source),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        spilled_user_loads.is_empty(),
        "dead user locals should not be spilled across await, got loads from: {:?}",
        spilled_user_loads
    );
}

#[test]
fn if_structured_multi_await_poll_helper_polls_child_futures_without_reinvoking_body() {
    let source = r#"
async def step1() -> i64 { 1 }
async def step2(x: i64) -> i64 { x + 1 }
async def main(flag: bool) -> i64 {
    let seed = if flag { 40 } else { 41 };
    let a = await step1();
    let b = await step2(a + seed);
    b
}
"#;

    let mir_fns = compile_to_mir(source).expect("should compile");
    let poll_fn = mir_fns
        .iter()
        .find(|f| f.name == "main__poll")
        .expect("main__poll helper should exist");

    let call_names = poll_fn
        .instructions
        .iter()
        .filter_map(|inst| match inst {
            Instruction::Call { func, .. } => Some(func.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        call_names.contains(&"step1__poll"),
        "main__poll should poll step1 future in if-structured async bodies, got calls: {:?}",
        call_names
    );
    assert!(
        call_names.contains(&"step2__poll"),
        "main__poll should poll step2 future in if-structured async bodies, got calls: {:?}",
        call_names
    );
    assert!(
        !call_names.contains(&"main__body"),
        "main__poll should not re-enter main__body for if-structured async bodies, got calls: {:?}",
        call_names
    );
}

#[test]
fn loop_with_await_polls_child_future_without_reinvoking_body() {
    let source = r#"
async def step() -> i64 { 1 }
async def main() -> i64 {
    let x = 0;
    while x < 2 {
        let y = await step();
        x = x + y;
    }
    x
}
"#;

    let mir_fns = compile_to_mir(source).expect("cyclic async cfg should lower to MIR");
    let poll_fn = mir_fns
        .iter()
        .find(|f| f.name == "main__poll")
        .expect("main__poll helper should exist");
    let call_names = poll_fn
        .instructions
        .iter()
        .filter_map(|inst| match inst {
            Instruction::Call { func, .. } => Some(func.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        call_names.contains(&"step__poll"),
        "main__poll should poll loop child futures, got calls: {:?}",
        call_names
    );
    assert!(
        !call_names.contains(&"main__body"),
        "main__poll should not re-enter main__body for loop async bodies, got calls: {:?}",
        call_names
    );
}

#[test]
fn match_with_await_arms_polls_child_futures_without_reinvoking_body() {
    let source = r#"
async def a() -> i64 { 10 }
async def b() -> i64 { 20 }
async def main() -> i64 {
    let x = 0;
    let y = match x {
        0 => await a(),
        _ => await b(),
    };
    y
}
"#;

    let mir_fns = compile_to_mir(source).expect("switch-shaped async cfg should lower to MIR");
    let poll_fn = mir_fns
        .iter()
        .find(|f| f.name == "main__poll")
        .expect("main__poll helper should exist");
    let call_names = poll_fn
        .instructions
        .iter()
        .filter_map(|inst| match inst {
            Instruction::Call { func, .. } => Some(func.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        call_names.contains(&"a__poll"),
        "main__poll should poll match arm future a, got calls: {:?}",
        call_names
    );
    assert!(
        call_names.contains(&"b__poll"),
        "main__poll should poll match arm future b, got calls: {:?}",
        call_names
    );
    assert!(
        !call_names.contains(&"main__body"),
        "main__poll should not re-enter main__body for match-shaped async bodies, got calls: {:?}",
        call_names
    );
}

#[test]
fn async_bool_local_survives_await() {
    let source = r#"
async def step1() -> i64 { 41 }
async def step2(x: i64) -> i64 { x + 1 }
async def main() -> i64 {
    let keep: bool = true;
    let first = await step1();
    let value = await step2(first);
    if keep { value } else { 0 }
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "bool local crossing await should compile, got: {:?}",
        result.err()
    );
}

#[test]
fn async_i32_local_survives_await() {
    let source = r#"
extern "C" {
    fn get_i32() -> i32;
}
async def step1() -> i64 { 41 }
async def step2(x: i64) -> i64 { x + 1 }
async def main() -> i64 {
    let keep = get_i32();
    let mirror = get_i32();
    let first = await step1();
    let value = await step2(first);
    if keep == mirror { value } else { 0 }
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "i32 local crossing await should compile, got: {:?}",
        result.err()
    );
}

#[test]
fn async_ref_local_survives_await() {
    let source = r#"
async def step1() -> i64 { 0 }
async def step2(x: i64) -> i64 { x + 42 }
async def main() -> i64 {
    let base = 41;
    let keep = &base;
    let first = await step1();
    let value = await step2(first);
    if *keep == 41 { value } else { 0 }
}
"#;

    let ir = compile_to_ir(source).unwrap_or_else(|err| {
        panic!("ref local crossing await should compile, got: {err:?}");
    });
    assert!(
        !ir.contains("ptrtoint i64 %"),
        "ref live-slot spill should encode the pointer value, not an i64 payload:\n{ir}"
    );
    assert!(
        ir.contains("load i64*, i64**"),
        "deref of a spilled ref local should load the pointer value before reading the pointee:\n{ir}"
    );
}

#[test]
fn async_struct_local_survives_await() {
    let source = r#"
struct Point { x: i64, y: i64 }
async def step1() -> i64 { 41 }
async def step2(x: i64) -> i64 { x + 1 }
async def main() -> i64 {
    let point = Point { x: 1, y: 2 };
    let first = await step1();
    let value = await step2(first);
    if point.x == 1 { value } else { 0 }
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "struct local crossing await should compile, got: {:?}",
        result.err()
    );
}

#[test]
fn async_array_local_survives_await() {
    let source = r#"
async def step1() -> i64 { 41 }
async def step2(x: i64) -> i64 { x + 1 }
async def main() -> i64 {
    let values = [1, 2, 3];
    let first = await step1();
    let value = await step2(first);
    if values[0] == 1 { value } else { 0 }
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "array local crossing await should compile, got: {:?}",
        result.err()
    );
}

/// Test that i8 locals can survive across await points.
/// i8 values should be encoded/decoded correctly through the async frame.
#[test]
fn async_i8_local_survives_await() {
    let source = r#"
extern "C" {
    fn get_i8() -> i8;
}
async def step1() -> i64 { 41 }
async def step2(x: i64) -> i64 { x + 1 }
async def main() -> i64 {
    let keep = get_i8();
    let mirror = get_i8();
    let first = await step1();
    let value = await step2(first);
    if keep == mirror { value } else { 0 }
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "i8 local crossing await should compile, got: {:?}",
        result.err()
    );
}

/// Test that i16 locals can survive across await points.
/// i16 values should be encoded/decoded correctly through the async frame.
#[test]
fn async_i16_local_survives_await() {
    let source = r#"
extern "C" {
    fn get_i16() -> i16;
}
async def step1() -> i64 { 41 }
async def step2(x: i64) -> i64 { x + 1 }
async def main() -> i64 {
    let keep = get_i16();
    let mirror = get_i16();
    let first = await step1();
    let value = await step2(first);
    if keep == mirror { value } else { 0 }
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "i16 local crossing await should compile, got: {:?}",
        result.err()
    );
}

#[test]
fn async_f64_local_survives_await() {
    let source = r#"
async def step1() -> i64 { 41 }
async def main() -> f64 {
    let keep: f64 = 3.14;
    let first = await step1();
    keep
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "f64 local crossing await should compile, got: {:?}",
        result.err()
    );
}

#[test]
fn async_f32_local_survives_await() {
    let source = r#"
extern "C" {
    fn get_f32() -> f32;
}
async def step1() -> i64 { 41 }
async def main() -> f32 {
    let keep = get_f32();
    let first = await step1();
    keep
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "f32 local crossing await should compile, got: {:?}",
        result.err()
    );
}

#[test]
fn async_helper_ir_uses_bitcast_for_float_frame_roundtrip() {
    let source = r#"
async def step1() -> i64 { 41 }
async def main() -> f64 {
    let keep: f64 = 3.14;
    let first = await step1();
    keep
}
"#;

    let ir = compile_to_ir(source).expect("should compile to IR");
    assert!(
        ir.contains("bitcast double") && ir.contains("bitcast i64"),
        "async float frame round-trip should use bitcast in generated IR, got:\n{}",
        &ir[..ir.len().min(4000)]
    );
}

/// Test that nested Future handles can survive across await points.
/// This tests that Future<T> handles are treated as pointer-like values.
#[test]
fn async_nested_future_handle_survives_await() {
    let source = r#"
async def inner() -> i64 { 42 }
async def middle() -> i64 {
    await inner()
}
async def outer() -> i64 {
    let fut = middle();
    let x = await inner();
    let y = await fut;
    x + y
}
async def main() -> i64 {
    await outer()
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "nested Future handle crossing await should compile, got: {:?}",
        result.err()
    );
}
