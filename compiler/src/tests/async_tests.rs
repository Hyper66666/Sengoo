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
fn async_block_is_rejected() {
    let source = r#"
async def main() -> i64 {
    let f = async { 42 };
    0
}
"#;

    let err = compile_to_ir(source).expect_err("async blocks should be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("async blocks are not yet supported"),
        "error should mention async block restriction, got: {}",
        msg
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
    assert!(
        !main_fn.is_async,
        "wrapper main should not be marked async"
    );

    let body_fn = mir_fns.iter().find(|f| f.name == "main__body").unwrap();
    assert!(body_fn.is_async, "body function should still be marked async");
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
    assert!(
        ir.contains("@main__poll"),
        "IR should contain main__poll"
    );
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

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "ref local crossing await should compile, got: {:?}",
        result.err()
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

/// Test that f64 locals crossing await are currently rejected.
/// Float types require bitcast support which is not yet implemented in MIR.
#[test]
fn async_f64_local_across_await_rejected() {
    let source = r#"
async def step1() -> i64 { 41 }
async def main() -> f64 {
    let keep: f64 = 3.14;
    let first = await step1();
    keep
}
"#;

    let err = compile_to_ir(source).expect_err("f64 crossing await should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("float types require bitcast support"),
        "f64-crossing-await error should mention bitcast requirement, got: {}",
        msg
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


