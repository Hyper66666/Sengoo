use crate::{
    ast::{DeclKind, TypeKind},
    compile_to_ir, Parser,
};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

const MARKER_TRAITS: &str = r#"
trait Send {}
trait Sync {}
"#;

#[test]
fn negative_marker_impl_parses_and_emits_no_runtime_function() {
    let source = format!(
        "{MARKER_TRAITS}\n{}",
        r#"
struct LocalOnly {}
impl !Send for LocalOnly {}
impl !Sync for LocalOnly {}

def main() -> i64 { 0 }
"#
    );

    let program = Parser::parse(&source).expect("negative marker impls should parse");
    let negative_count = program
        .decls
        .iter()
        .filter(|decl| matches!(&decl.kind, DeclKind::Impl(item) if item.is_negative))
        .count();
    assert_eq!(negative_count, 2);

    let ir = compile_to_ir(&source).expect("empty negative marker impls should typecheck");
    assert!(!ir.contains("LocalOnly_Send"));
    assert!(!ir.contains("LocalOnly_Sync"));
}

#[test]
fn generic_negative_marker_impl_rejects_every_instantiation() {
    let source = format!(
        "{MARKER_TRAITS}\n{}",
        r#"
struct LocalBox<T> { value: T }
impl<T> !Send for LocalBox<T> {}
def require_send<T: Send>(value: T) -> i64 { 1 }
def main() -> i64 { require_send(LocalBox { value: 42 }) }
"#
    );
    let err = compile_to_ir(&source)
        .expect_err("a generic !Send impl should reject each concrete instantiation");
    assert!(
        err.to_string().contains("does not implement `Send`"),
        "unexpected generic negative-impl diagnostic: {err}"
    );
}

#[test]
fn negative_marker_impl_is_order_independent() {
    let source = format!(
        "{MARKER_TRAITS}\n{}",
        r#"
struct LocalOnly {}
def require_send<T: Send>(value: T) -> i64 { 1 }
def main() -> i64 { require_send(LocalOnly {}) }
impl !Send for LocalOnly {}
"#
    );
    let err = compile_to_ir(&source)
        .expect_err("a later negative impl must still affect earlier function bodies");
    assert!(
        err.to_string().contains("does not implement `Send`"),
        "unexpected order-independent negative-impl diagnostic: {err}"
    );
}

#[test]
fn negative_send_impl_rejects_direct_and_nested_struct_bounds() {
    for (label, body) in [
        (
            "direct",
            r#"
struct LocalOnly {}
impl !Send for LocalOnly {}
def main() -> i64 { require_send(LocalOnly {}) }
"#,
        ),
        (
            "nested struct",
            r#"
struct LocalOnly {}
impl !Send for LocalOnly {}
struct Wrapper { value: LocalOnly }
def main() -> i64 { require_send(Wrapper { value: LocalOnly {} }) }
"#,
        ),
    ] {
        let source = format!(
            "{MARKER_TRAITS}\n{}\n{body}",
            "def require_send<T: Send>(value: T) -> i64 { 1 }"
        );
        let err = compile_to_ir(&source)
            .expect_err(&format!("{label} !Send value should fail a Send bound"));
        let message = err.to_string();
        assert!(
            message.contains("does not implement `Send`") || message.contains("not Send"),
            "expected stable Send-bound diagnostic for {label}, got: {message}"
        );
    }
}

#[test]
fn negative_sync_impl_is_trait_specific_and_propagates_through_enum_payloads() {
    let send_source = format!(
        "{MARKER_TRAITS}\n{}",
        r#"
struct LocalSyncOnly {}
impl !Sync for LocalSyncOnly {}
def require_send<T: Send>(value: T) -> i64 { 1 }
def main() -> i64 { require_send(LocalSyncOnly {}) }
"#
    );
    compile_to_ir(&send_source).expect("!Sync must not suppress structural Send");

    let sync_source = format!(
        "{MARKER_TRAITS}\n{}",
        r#"
struct LocalSyncOnly {}
impl !Sync for LocalSyncOnly {}
enum Envelope { Item(LocalSyncOnly), Empty }
def require_sync<T: Sync>(value: T) -> i64 { 1 }
def main() -> i64 { require_sync(Envelope::Item(LocalSyncOnly {})) }
"#
    );
    let err = compile_to_ir(&sync_source)
        .expect_err("an enum carrying a !Sync payload should fail a Sync bound");
    let message = err.to_string();
    assert!(
        message.contains("does not implement `Sync`") || message.contains("not Sync"),
        "expected stable Sync-bound diagnostic, got: {message}"
    );
}

#[test]
fn negative_send_capture_cannot_cross_spawn_blocking_boundary() {
    let source = format!(
        "{MARKER_TRAITS}\n{}",
        r#"
struct LocalOnly { value: i64 }
impl !Send for LocalOnly {}

def consume(value: LocalOnly) -> i64 { value.value }
def spawn_blocking_i64(callback: fn() -> i64) -> i64 { callback() }

def main() -> i64 {
    let local = LocalOnly { value: 42 };
    spawn_blocking_i64(| | consume(local))
}
"#
    );
    let err =
        compile_to_ir(&source).expect_err("a captured !Send value must not cross spawn_blocking");
    assert!(
        err.to_string().contains("is not Send"),
        "unexpected cross-thread negative-impl diagnostic: {err}"
    );
}

#[test]
fn negative_impl_rejects_non_marker_traits_and_associated_items() {
    let non_marker = r#"
trait Display {}
struct Widget {}
impl !Display for Widget {}
def main() -> i64 { 0 }
"#;
    let err = compile_to_ir(non_marker)
        .expect_err("negative impls for non-marker traits must be rejected");
    assert!(
        err.to_string()
            .contains("negative impls are only supported for marker traits `Send` and `Sync`"),
        "unexpected non-marker diagnostic: {err}"
    );

    let with_method = format!(
        "{MARKER_TRAITS}\n{}",
        r#"
struct Widget {}
impl !Send for Widget {
    def forbidden(&self) -> i64 { 0 }
}
def main() -> i64 { 0 }
"#
    );
    let err =
        compile_to_ir(&with_method).expect_err("negative marker impls must not define methods");
    assert!(
        err.to_string().contains(
            "negative impl `!Send` for `Widget` must not define methods or associated types"
        ),
        "unexpected negative-item diagnostic: {err}"
    );

    let with_associated_type = format!(
        "{MARKER_TRAITS}\n{}",
        r#"
struct Widget {}
impl !Sync for Widget {
    type Item = i64;
}
def main() -> i64 { 0 }
"#
    );
    let err = compile_to_ir(&with_associated_type)
        .expect_err("negative marker impls must not define associated types");
    assert!(
        err.to_string().contains(
            "negative impl `!Sync` for `Widget` must not define methods or associated types"
        ),
        "unexpected negative associated-item diagnostic: {err}"
    );
}

#[test]
fn positive_and_negative_marker_impls_conflict_in_either_order() {
    for impls in [
        "impl Send for Token {}\nimpl !Send for Token {}",
        "impl !Send for Token {}\nimpl Send for Token {}",
    ] {
        let source =
            format!("{MARKER_TRAITS}\nstruct Token {{}}\n{impls}\ndef main() -> i64 {{ 0 }}");
        let err =
            compile_to_ir(&source).expect_err("positive and negative marker impls must conflict");
        assert!(
            err.to_string().contains(
                "conflicting positive and negative implementations of trait `Send` for type `Token`"
            ),
            "unexpected marker conflict diagnostic: {err}"
        );
    }
}

#[test]
fn stdlib_single_thread_handles_declare_negative_marker_impls() {
    let cases: &[(&str, &[&str])] = &[
        ("ffi.sg", &["CLib", "CppObject", "CallbackToken", "Buffer"]),
        ("collections.sg", &["Rc"]),
        ("json.sg", &["JsonDoc", "JsonValue"]),
        (
            "process.sg",
            &["ProcessCommand", "ProcessOutput", "ProcessHandle"],
        ),
        ("dir.sg", &["DirWalk"]),
        ("db.sg", &["Db", "DbResult"]),
        ("lua54.sg", &["Lua54"]),
        ("async.sg", &["MutexGuardI64"]),
        ("async_futures.sg", &["AsyncContext"]),
    ];
    let stdlib_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("compiler crate should have a workspace parent")
        .join("tools")
        .join("stdlib");

    for (module, expected_types) in cases {
        let source = fs::read_to_string(stdlib_root.join(module))
            .unwrap_or_else(|err| panic!("failed to read {module}: {err}"));
        let program =
            Parser::parse(&source).unwrap_or_else(|err| panic!("failed to parse {module}: {err}"));
        let declarations = program
            .decls
            .iter()
            .filter_map(|decl| {
                let DeclKind::Impl(item) = &decl.kind else {
                    return None;
                };
                if !item.is_negative {
                    return None;
                }
                let trait_name = item.trait_path.as_ref()?.as_simple()?.name.clone();
                let target_name = match &item.target_type.kind {
                    TypeKind::Path(path) => path.as_simple()?.name.clone(),
                    TypeKind::PathWithArgs { path, .. } => path.as_simple()?.name.clone(),
                    _ => return None,
                };
                Some((trait_name, target_name))
            })
            .collect::<HashSet<_>>();

        for expected_type in *expected_types {
            assert!(
                declarations.contains(&("Send".to_string(), (*expected_type).to_string())),
                "{module} must explicitly declare `{expected_type}: !Send`"
            );
            assert!(
                declarations.contains(&("Sync".to_string(), (*expected_type).to_string())),
                "{module} must explicitly declare `{expected_type}: !Sync`"
            );
        }
    }
}
