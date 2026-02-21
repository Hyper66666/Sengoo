use crate::hir::HIRItem;
use crate::{lower_ast, Parser, TypeChecker};

#[test]
fn lower_ast_preserves_generic_bounds_in_hir_function() {
    let source = r#"
trait Add {}
trait Copy {}

def foo<T: Add + Copy>(x: i64) -> i64 {
    x
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("typecheck should succeed");
    let env = checker.into_env();
    let module = lower_ast(&program, &env);

    let function = module
        .items
        .iter()
        .find_map(|item| match item {
            HIRItem::Function(function) if function.name == "foo" => Some(function),
            _ => None,
        })
        .expect("expected function foo in HIR");

    assert_eq!(function.type_params.len(), 1);
    let tp = &function.type_params[0];
    assert_eq!(tp.name, "T");
    let bounds = tp
        .bounds
        .iter()
        .map(|bound| bound.trait_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(bounds, vec!["Add", "Copy"]);
}

#[test]
fn lower_ast_preserves_where_clause_bounds_in_hir_function() {
    let source = r#"
trait Add {}
trait Copy {}

def foo<T>(x: i64) -> i64 where T: Add + Copy {
    x
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("typecheck should succeed");
    let env = checker.into_env();
    let module = lower_ast(&program, &env);

    let function = module
        .items
        .iter()
        .find_map(|item| match item {
            HIRItem::Function(function) if function.name == "foo" => Some(function),
            _ => None,
        })
        .expect("expected function foo in HIR");

    assert_eq!(function.type_params.len(), 1);
    let tp = &function.type_params[0];
    assert_eq!(tp.name, "T");
    let bounds = tp
        .bounds
        .iter()
        .map(|bound| bound.trait_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(bounds, vec!["Add", "Copy"]);
}

#[test]
fn lower_ast_preserves_generic_params_on_trait_method() {
    let source = r#"
trait Mapper {
    def map<T>(x: T) -> T {
        x
    }
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("typecheck should succeed");
    let env = checker.into_env();
    let module = lower_ast(&program, &env);

    let trait_item = module
        .items
        .iter()
        .find_map(|item| match item {
            HIRItem::Trait(trait_item) if trait_item.name == "Mapper" => Some(trait_item),
            _ => None,
        })
        .expect("expected trait Mapper in HIR");

    let method = trait_item
        .items
        .iter()
        .find_map(|item| match item {
            crate::hir::HIRTraitItem::Function(function) if function.name == "map" => {
                Some(function)
            }
            _ => None,
        })
        .expect("expected method map in trait Mapper");

    assert_eq!(method.type_params.len(), 1);
    assert_eq!(method.type_params[0].name, "T");
}
