use crate::compile_to_ir;

#[test]
fn declarative_macro_invocation_expands_before_typecheck() {
    let source = r#"
macro_rules! make_main {
    ($value:expr) => {
        def main() -> i64 {
            $value
        }
    };
}

make_main!(42)
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "declarative macro should expand into valid source, got: {:?}",
        result.err()
    );
}

#[test]
fn macro_invocation_without_matching_arm_is_rejected() {
    let source = r#"
macro_rules! add {
    ($lhs:expr, $rhs:expr) => { $lhs + $rhs };
}

def main() -> i64 {
    add!(1)
}
"#;

    let err = compile_to_ir(source).expect_err("macro argument mismatch should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("no matching macro arm"),
        "error should mention macro arm mismatch, got: {}",
        msg
    );
}
