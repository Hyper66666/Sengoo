use crate::ast::DeclKind;
use crate::{compile_to_ir, compile_to_mir, lower_ast, Parser, TypeChecker};

#[test]
fn async_function_with_non_async_await_is_rejected() {
    let source = r#"
async def main() -> i64 {
    let value = await 1;
    value
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "phase-1 await should reject non-async operands, got: {:?}",
        result.ok()
    );
    let msg = result.err().expect("non-async await should fail").to_string();
    assert!(
        msg.contains("phase-1 await requires an async call result"),
        "error should explain phase-1 await restriction, got: {}",
        msg
    );
}

#[test]
fn await_outside_async_context_is_rejected() {
    let source = r#"
def main() -> i64 {
    let value = await 1;
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
fn lower_ast_preserves_await_nodes_in_hir() {
    let source = r#"
async def add_one(x: i64) -> i64 {
    x + 1
}

async def main() -> i64 {
    let value = await add_one(41);
    value
}
"#;

    let program = Parser::parse(source).expect("parser should accept async program");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("type checker should accept async program");
    let env = checker.into_env();
    let module = lower_ast(&program, &env);
    let dumped = format!("{module:#?}");
    assert!(
        dumped.contains("Await"),
        "HIR should preserve await nodes for async lowering, got:\n{}",
        dumped
    );
}

#[test]
fn async_block_is_rejected_in_phase1() {
    let source = r#"
async def main() -> i64 {
    let value = async {
        1
    };
    await value
}
"#;

    let err = compile_to_ir(source).expect_err("phase-1 async blocks should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("async blocks are not supported in phase 1"),
        "error should explain phase-1 async block restriction, got: {}",
        msg
    );
}

#[test]
fn compile_to_mir_synthesizes_async_helpers_and_main_wrapper() {
    let source = r#"
async def add_one(x: i64) -> i64 {
    x + 1
}

async def main() -> i64 {
    await add_one(41)
}
"#;

    let mir = compile_to_mir(source).expect("async program should lower to MIR");
    let names: Vec<&str> = mir.iter().map(|f| f.name.as_str()).collect();

    for required in [
        "add_one",
        "add_one__start",
        "add_one__poll",
        "add_one__result",
        "main__async_body",
        "main__start",
        "main__poll",
        "main__result",
        "main",
    ] {
        assert!(
            names.contains(&required),
            "expected synthesized async MIR function `{required}`, got {:?}",
            names
        );
    }
}

#[test]
fn compile_to_ir_emits_async_helper_calls_and_sync_main_wrapper() {
    let source = r#"
async def add_one(x: i64) -> i64 {
    x + 1
}

async def main() -> i64 {
    await add_one(41)
}
"#;

    let ir = compile_to_ir(source).expect("async program should compile to LLVM IR");

    for required in [
        "define i64 @add_one__start(i64 %l_1)",
        "define i64 @add_one__poll(i64 %l_1)",
        "define i64 @add_one__result(i64 %l_1)",
        "define i64 @main__start()",
        "define i64 @main__poll(i64 %l_1)",
        "define i64 @main__result(i64 %l_1)",
        "define i64 @main()",
        "declare i64 @sengoo_async_run_main_i64()",
        "call i64 @add_one__start(",
        "call i64 @add_one__poll(",
        "call i64 @add_one__result(",
        "call i64 @sengoo_async_run_main_i64()",
    ] {
        assert!(
            ir.contains(required),
            "expected async IR to contain `{required}`, got:\n{}",
            ir
        );
    }

    for forbidden in [
        "call i64 @main__start()",
        "call i64 @main__poll(",
        "call i64 @main__result(",
    ] {
        assert!(
            !ir.contains(forbidden),
            "sync main wrapper should delegate through runtime bridge instead of `{forbidden}`, got:\n{}",
            ir
        );
    }
}
