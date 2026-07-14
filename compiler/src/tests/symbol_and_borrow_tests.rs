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
    0
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
