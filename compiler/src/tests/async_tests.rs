use crate::ast::DeclKind;
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
