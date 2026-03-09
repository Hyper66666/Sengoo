use crate::compile_to_ir;

#[test]
fn derive_macro_generated_impl_is_available() {
    let source = r#"
#[derive(Auto)]
struct User {
    id: i64,
}

def main() -> i64 {
    let user = User { id: 1 };
    user.__derive_auto()
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "derive expansion should generate impl items visible to later phases, got: {:?}",
        result.err()
    );
}

#[test]
fn derive_on_non_type_declaration_is_rejected() {
    let source = r#"
#[derive(Auto)]
const FLAG: i64 = 1;

def main() -> i64 {
    FLAG
}
"#;

    let err = compile_to_ir(source).expect_err("derive on const should fail");
    assert!(
        err.to_string().contains("derive attribute is only supported"),
        "error should mention derive target restriction, got: {}",
        err
    );
}
