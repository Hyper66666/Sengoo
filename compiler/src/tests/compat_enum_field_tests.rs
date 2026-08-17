//! Compiler-known `.is_ok` / `.is_some` / `.value` / `.error` accessors over
//! enum `Option` and `Result` (OpenSpec 2.3).

use crate::error::CompileWarning;
use crate::mir::{Instruction, MirFunction};
use crate::{collect_compile_warnings, compile_to_ir, compile_to_mir};
use std::fs;
use std::path::Path;

fn decls() -> &'static str {
    r#"
enum Option<T> { None, Some(T) }
enum Result<T, E> { Ok(T), Err(E) }
"#
}

fn with_decls(body: &str) -> String {
    format!("{}\n{}", decls(), body)
}

fn load_stdlib(modules: &[&str]) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_root = manifest_dir
        .parent()
        .unwrap_or(manifest_dir)
        .join("tools")
        .join("stdlib");
    modules
        .iter()
        .map(|module| {
            fs::read_to_string(stdlib_root.join(module))
                .unwrap_or_else(|err| panic!("failed to read {module}: {err}"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn function<'a>(mir_fns: &'a [MirFunction], name: &str) -> &'a MirFunction {
    mir_fns
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("expected function {name}"))
}

fn warning_named<'a>(warnings: &'a [CompileWarning], field: &str) -> &'a CompileWarning {
    warnings
        .iter()
        .find(|warning| match warning {
            CompileWarning::DeprecatedUse { name, .. } => name.ends_with(field),
        })
        .unwrap_or_else(|| panic!("expected deprecated-use warning for {field}, got {warnings:?}"))
}

#[test]
fn compat_accessors_typecheck_and_compile() {
    compile_to_ir(&with_decls(
        r#"
def read_result(result: Result<i64, i64>) -> i64 {
    if result.is_ok { result.value } else { result.error }
}
def read_option(option: Option<i64>) -> i64 {
    if option.is_some { option.value } else { 0 }
}
def main() -> i64 {
    read_result(Ok(1)) + read_option(Some(2)) + read_result(Err(3))
}
"#,
    ))
    .expect("legacy Option/Result field reads should compile against the enum form");
}

#[test]
fn compat_accessors_emit_deprecated_use_with_pattern_replacements() {
    let warnings = collect_compile_warnings(&with_decls(
        r#"
def read_result(result: Result<i64, i64>) -> i64 {
    if result.is_ok { result.value } else { result.error }
}
def read_option(option: Option<i64>) -> i64 {
    if option.is_some { option.value } else { 0 }
}
def main() -> i64 { 0 }
"#,
    ))
    .expect("compatibility field reads should typecheck");

    assert_eq!(
        warnings.len(),
        5,
        "one warning per field read, got {warnings:?}"
    );
    for warning in &warnings {
        assert_eq!(warning.code(), "attributes::deprecated_use");
        assert_eq!(warning.removal(), Some("the release after next"));
        assert!(
            warning
                .to_string()
                .contains("is an enum; direct field access is deprecated"),
            "unexpected warning text: {warning}"
        );
    }

    let is_ok = warning_named(&warnings, ".is_ok");
    assert_eq!(is_ok.replacement(), Some("match on `Ok(..)` / `Err(..)`"));
    let is_some = warning_named(&warnings, ".is_some");
    assert_eq!(is_some.replacement(), Some("match on `Some(..)` / `None`"));
    let value = warning_named(&warnings, ".value");
    let value_hint = value.replacement().expect("value replacement");
    assert!(
        value_hint.contains("`Ok(value)`") || value_hint.contains("`Some(value)`"),
        "value hint should name a payload pattern, got {value_hint}"
    );
    let error = warning_named(&warnings, ".error");
    assert_eq!(
        error.replacement(),
        Some("bind the payload with a `Err(error)` pattern")
    );
}

#[test]
fn compat_method_call_does_not_emit_field_deprecation() {
    let warnings = collect_compile_warnings(&with_decls(
        r#"
impl<T> Option<T> {
    def is_some(&self) -> bool { true }
}
def main() -> i64 {
    let option: Option<i64> = Some(1);
    if option.is_some() { 1 } else { 0 }
}
"#,
    ))
    .expect("method form should typecheck");
    assert!(
        warnings.iter().all(|warning| match warning {
            CompileWarning::DeprecatedUse { name, .. } => !name.contains(".is_some"),
        }),
        "`.is_some()` must not be reported as a deprecated field: {warnings:?}"
    );
}

#[test]
fn compat_is_ok_lowers_through_the_discriminant() {
    let mir = compile_to_mir(&with_decls(
        r#"
def flag(result: Result<i64, i64>) -> bool { result.is_ok }
def main() -> i64 { 0 }
"#,
    ))
    .expect("is_ok accessor should lower");
    let flag = function(&mir, "flag");
    assert!(
        flag.instructions
            .iter()
            .any(|inst| matches!(inst, Instruction::Discriminant { .. })),
        "`.is_ok` must compare the enum discriminant"
    );
    assert!(
        !flag
            .instructions
            .iter()
            .any(|inst| matches!(inst, Instruction::ExtractPayload { .. })),
        "flag accessors must not extract a payload"
    );
}

#[test]
fn compat_value_and_error_lower_to_a_join_slot() {
    let mir = compile_to_mir(&with_decls(
        r#"
def payloads(result: Result<i64, i64>) -> i64 {
    result.value + result.error
}
def main() -> i64 { 0 }
"#,
    ))
    .expect("payload accessors should lower");
    let payloads = function(&mir, "payloads");
    let extract_count = payloads
        .instructions
        .iter()
        .filter(|inst| matches!(inst, Instruction::ExtractPayload { .. }))
        .count();
    assert_eq!(
        extract_count, 2,
        "`.value` and `.error` should each extract a payload"
    );
}

#[test]
fn compat_error_on_option_is_rejected() {
    let error = compile_to_ir(&with_decls(
        r#"
def main() -> i64 {
    let option: Option<i64> = None;
    option.error
}
"#,
    ))
    .expect_err("Option has no `.error` field");
    let message = error.to_string();
    assert!(
        message.contains("error"),
        "expected a missing-field diagnostic, got {message}"
    );
}

#[test]
fn compat_accessors_do_not_apply_to_unrelated_enums() {
    let error = compile_to_ir(
        r#"
enum Maybe<T> { None, Some(T) }
def main() -> i64 {
    let maybe: Maybe<i64> = Maybe::Some(1);
    if maybe.is_some { maybe.value } else { 0 }
}
"#,
    )
    .expect_err("user enums must not inherit Option field accessors");
    let message = error.to_string();
    assert!(
        message.contains("is_some") || message.contains("field"),
        "expected a missing-field diagnostic, got {message}"
    );
}

#[test]
fn stdlib_option_result_compat_fields_compile() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg"]),
        r#"
def read_result(result: Result<i64, i64>) -> i64 {
    if result.is_ok { result.value } else { result.error }
}
def read_option(option: Option<i64>) -> i64 {
    if option.is_some { option.value } else { 0 }
}
def main() -> i64 {
    read_result(Ok(1)) + read_option(Some(2))
}
"#
    );
    compile_to_ir(&source)
        .expect("stdlib enum Option/Result must still accept the deprecated field surface");
}

#[test]
fn placeholder_constructors_remain_usable_with_deprecation() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg"]),
        r#"
def main() -> i64 {
    let none_flag = option_none_with(0);
    let ok_flag = result_ok_with(1, 9);
    let err_flag = result_err_with(0, 2);
    none_flag.unwrap_or(3) + ok_flag.unwrap_or(0) + err_flag.unwrap_or(4)
}
"#
    );
    compile_to_ir(&source).expect("placeholder constructors must still compile");
    let warnings = collect_compile_warnings(&source).expect("placeholder uses should typecheck");
    let names: Vec<_> = warnings
        .iter()
        .map(|warning| {
            let CompileWarning::DeprecatedUse { name, .. } = warning;
            name.as_str()
        })
        .collect();
    assert!(
        names.contains(&"option_none_with"),
        "expected option_none_with deprecation, got {warnings:?}"
    );
    assert!(
        names.contains(&"result_ok_with"),
        "expected result_ok_with deprecation, got {warnings:?}"
    );
    assert!(
        names.contains(&"result_err_with"),
        "expected result_err_with deprecation, got {warnings:?}"
    );
    let none = warnings
        .iter()
        .find(|warning| matches!(warning, CompileWarning::DeprecatedUse { name, .. } if name == "option_none_with"))
        .expect("option_none_with warning");
    assert_eq!(none.replacement(), Some("None"));
}
