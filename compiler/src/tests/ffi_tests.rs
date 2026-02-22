use crate::hir::{HIRExternItem, HIRItem};
use crate::{compile_to_ir, lower_ast, parser::Parser, typeck::TypeChecker};

#[test]
fn extern_block_lowers_into_hir() {
    let source = r#"
extern "C" {
    pub fn c_add(a: i64, b: i64) -> i64;
}

def main() -> i64 {
    c_add(1, 2)
}
"#;

    let program = Parser::parse(source).expect("extern block should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("extern declarations should typecheck");
    let env = checker.into_env();

    let module = lower_ast(&program, &env);
    assert!(
        module.items.iter().any(|item| {
            matches!(
                item,
                HIRItem::ExternBlock(block)
                    if block.abi == "C" && block.items.iter().any(|it| matches!(
                        it,
                        HIRExternItem::Function(f) if f.name == "c_add"
                    ))
            )
        }),
        "expected lowered HIR to contain extern block declaration"
    );
}

#[test]
fn extern_call_codegen_emits_declaration() {
    let source = r#"
extern "C" {
    pub fn c_add(a: i64, b: i64) -> i64;
}

def main() -> i64 {
    c_add(40, 2)
}
"#;

    let ir = compile_to_ir(source).expect("extern call should compile");
    assert!(
        ir.contains("declare i64 @c_add(i64, i64)"),
        "expected extern declaration in LLVM IR, got:\n{}",
        ir
    );
    assert!(
        ir.contains("call i64 @c_add("),
        "expected call to extern symbol in LLVM IR, got:\n{}",
        ir
    );
}

#[test]
fn export_name_attribute_changes_emitted_symbol() {
    let source = r#"
#[export_name = "sengoo_add_export"]
pub extern "C" fn sengoo_add(a: i64, b: i64) -> i64 {
    a + b
}

def main() -> i64 {
    sengoo_add(1, 2)
}
"#;

    let ir = compile_to_ir(source).expect("extern exported function should compile");
    assert!(
        ir.contains("define i64 @sengoo_add_export("),
        "expected exported symbol name in function definition, got:\n{}",
        ir
    );
    assert!(
        ir.contains("%export_ret = call i64 @sengoo_add("),
        "expected exported wrapper to call internal symbol, got:\n{}",
        ir
    );
}

#[test]
fn ffi_rejects_unsupported_abi() {
    let source = r#"
extern "stdcall" {
    fn c_add(a: i64, b: i64) -> i64;
}

def main() -> i32 { 0 }
"#;

    let err = compile_to_ir(source).expect_err("unsupported ABI should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("unsupported ABI"),
        "expected unsupported ABI diagnostic, got: {}",
        msg
    );
}

#[test]
fn ffi_rejects_non_ffi_safe_types() {
    let source = r#"
extern "C" {
    fn bad_ref(arg: str) -> i64;
}

def main() -> i32 { 0 }
"#;

    let err = compile_to_ir(source).expect_err("reference in extern signature should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("FFI type"),
        "expected FFI type diagnostic, got: {}",
        msg
    );
}

#[test]
fn ffi_requires_unsafe_boundary_for_raw_pointer_signatures() {
    let source = r#"
extern "C" {
    fn read_buffer(ptr: *mut u8, len: usize) -> i64;
}

def main() -> i32 { 0 }
"#;

    let err = compile_to_ir(source).expect_err("raw-pointer extern should require unsafe");
    let msg = err.to_string();
    assert!(
        msg.contains("unsafe boundary"),
        "expected unsafe boundary diagnostic, got: {}",
        msg
    );
}
