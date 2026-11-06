use crate::ast::{DeclKind, ExprKind, StmtKind, UnOp};
use crate::hir::{HIRExpr, HIRItem};
use crate::{Parser, TypeChecker};

#[test]
fn parser_interns_same_identifier_symbol_in_function_body() {
    let source = r#"
def main() -> i64 {
    let value = 1;
    value
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let func = match &program.decls[0].kind {
        DeclKind::Function(func) => func,
        other => panic!("expected function decl, got {:?}", other),
    };

    let let_symbol = match &func.body.stmts[0].kind {
        StmtKind::Let { name, .. } => name.symbol,
        other => panic!("expected let stmt, got {:?}", other),
    };

    let use_symbol = match &func.body.stmts[1].kind {
        StmtKind::Expr(expr) => match &expr.kind {
            ExprKind::Ident(ident) => ident.symbol,
            ExprKind::Path(path) => {
                path.as_simple()
                    .expect("path expression should be a single segment")
                    .symbol
            }
            other => panic!(
                "expected identifier or simple path expression, got {:?}",
                other
            ),
        },
        other => panic!("expected expression stmt, got {:?}", other),
    };

    assert!(let_symbol.is_valid(), "interned symbol should be valid");
    assert_eq!(
        let_symbol, use_symbol,
        "same textual identifier should map to same SymbolId"
    );
}

#[test]
fn hir_lowering_keeps_parameter_symbol_identity() {
    let source = r#"
def main(v: i64) -> i64 {
    v
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&program)
        .expect("source should pass type checking");
    let type_env = type_checker.into_env();

    let hir_module = crate::lower_ast(&program, &type_env);
    let main_fn = hir_module
        .items
        .iter()
        .find_map(|item| match item {
            HIRItem::Function(func) if func.name == "main" => Some(func),
            _ => None,
        })
        .expect("expected lowered main function");

    let param_symbol = main_fn
        .params
        .first()
        .expect("expected one parameter")
        .symbol;
    assert!(param_symbol.is_valid(), "parameter symbol should be valid");

    let body_symbol = match main_fn.body.expr.as_deref() {
        Some(HIRExpr::Var { symbol, .. }) => *symbol,
        other => panic!(
            "expected body expr to be variable reference, got {:?}",
            other
        ),
    };
    assert_eq!(
        body_symbol, param_symbol,
        "HIR variable reference should retain the same symbol identity as parameter binding"
    );
}

#[test]
fn borrow_check_reports_mutable_and_immutable_conflict() {
    let source = r#"
def main() -> i64 {
    let x = 1;
    let a = &x;
    let b = &x;
    *a
}

"#;

    let mut program = Parser::parse(source).expect("source should parse");
    let func = match &mut program.decls[0].kind {
        DeclKind::Function(func) => func,
        other => panic!("expected function decl, got {:?}", other),
    };
    let let_b = func
        .body
        .stmts
        .get_mut(2)
        .expect("expected third stmt for let b");
    if let StmtKind::Let {
        value: Some(value), ..
    } = &mut let_b.kind
    {
        if let ExprKind::Unary { op, .. } = &mut value.kind {
            *op = UnOp::RefMut;
        } else {
            panic!("expected unary borrow expression in let b");
        }
    } else {
        panic!("expected let statement with value for let b");
    }

    let mut type_checker = TypeChecker::new();
    let err = type_checker
        .check_program(&program)
        .expect_err("borrow check should reject mutable and immutable conflict");
    let msg = err.to_string();
    assert!(
        msg.contains("borrow check failed")
            && (msg.contains("mutable borrow conflicts")
                || msg.contains("multiple mutable borrows")),
        "unexpected borrow-check error message: {msg}"
    );
}

#[test]
fn borrow_check_deduplicates_one_explicit_mut_borrow_ast_node() {
    let source = r#"
def read_mut(value: &mut i64) -> i64 { 1 }

def main() -> i64 {
    let mut value = 1;
    let observed = read_mut(&mut value);
    observed
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&program)
        .expect("one explicit mutable borrow must not conflict with its alias-tracking visit");
}

#[test]
fn borrow_check_method_scalar_result_does_not_retain_argument_loan_in_loop() {
    let source = r#"
struct Source {}
impl Source { def read(&self, out: &mut i64) -> i64 { *out } }
def inspect(value: &i64) -> i64 { *value }
def main() -> i64 {
    let source = Source {};
    let mut buffer = 3;
    loop {
        let count = source.read(&mut buffer);
        if count > 0 { inspect(&buffer); (); };
        break;
    };
    buffer
}
"#;
    let parsed = Parser::parse(source).unwrap();
    TypeChecker::new()
        .check_program(&parsed)
        .expect("scalar method results do not retain a temporary mutable argument loan");
}

#[test]
fn borrow_check_method_owned_result_does_not_alias_mutable_argument() {
    let source = r#"
struct Observation { count: i64 }
struct Source {}
impl Source { def read(&self, out: &mut i64) -> Observation { Observation { count: *out } } }
def inspect(value: &i64) -> i64 { *value }
def main() -> i64 {
    let source = Source {};
    let mut buffer = 3;
    loop {
        let observed = source.read(&mut buffer);
        if observed.count > 0 { inspect(&buffer); (); };
        break;
    };
    buffer
}
"#;
    let parsed = Parser::parse(source).unwrap();
    TypeChecker::new()
        .check_program(&parsed)
        .expect("a plain owned aggregate result does not borrow the input");
}

#[test]
fn borrow_check_method_reference_result_still_retains_argument_loan() {
    let source = r#"
struct Source {}
impl Source { def lend(&self, value: &mut i64) -> &mut i64 { value } }
def main() -> i64 {
    let source = Source {};
    let mut value = 3;
    let alias = source.lend(&mut value);
    let conflicting = &value;
    *alias + *conflicting
}
"#;
    let parsed = Parser::parse(source).unwrap();
    let error = TypeChecker::new()
        .check_program(&parsed)
        .expect_err("a true reference result must retain its input loan");
    assert!(error.to_string().contains("borrow check failed"), "{error}");
}

#[test]
fn borrow_check_method_scalar_result_still_checks_simultaneous_argument_loans() {
    let source = r#"
struct Source {}
impl Source { def read(&self, write: &mut i64, read: &i64) -> i64 { *read } }
def main() -> i64 {
    let source = Source {};
    let mut value = 3;
    source.read(&mut value, &value)
}
"#;
    let parsed = Parser::parse(source).unwrap();
    let error = TypeChecker::new()
        .check_program(&parsed)
        .expect_err("the call's own mutable and shared arguments still overlap");
    assert!(error.to_string().contains("borrow check failed"), "{error}");
}

#[test]
fn borrow_check_method_wrapped_reference_result_retains_argument_loan() {
    let source = r#"
struct Wrapped<T> { value: T }
struct Source {}
impl Source {
    def lend(&self, value: &mut i64) -> Wrapped<&mut i64> { Wrapped { value: value } }
}
def main() -> i64 {
    let source = Source {};
    let mut value = 3;
    let alias = source.lend(&mut value);
    let conflicting = &value;
    *alias.value + *conflicting
}
"#;
    let parsed = Parser::parse(source).unwrap();
    let error = TypeChecker::new()
        .check_program(&parsed)
        .expect_err("a generic wrapper still carries its reference payload");
    assert!(error.to_string().contains("borrow check failed"), "{error}");
}

#[test]
fn borrow_check_method_struct_reference_field_retains_argument_loan() {
    let source = r#"
struct View { borrowed: &mut i64 }
struct Source {}
impl Source {
    def lend(&self, value: &mut i64) -> View { View { borrowed: value } }
}
def main() -> i64 {
    let source = Source {};
    let mut value = 3;
    let alias = source.lend(&mut value);
    let conflicting = &value;
    *alias.borrowed + *conflicting
}
"#;
    let parsed = Parser::parse(source).unwrap();
    let error = TypeChecker::new()
        .check_program(&parsed)
        .expect_err("non-generic reference fields retain their source loan too");
    assert!(error.to_string().contains("borrow check failed"), "{error}");
}
