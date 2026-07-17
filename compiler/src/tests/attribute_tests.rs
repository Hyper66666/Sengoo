//! Attribute matrix tests for phase 4a surface expansion.

use crate::error::{CompileError, ParseError};
use crate::{compile_to_ir, compile_to_mir, parser, Parser, TypeChecker};

#[test]
fn cfg_target_os_filters_false_declarations() {
    let other_os = if cfg!(target_os = "windows") {
        "linux"
    } else {
        "windows"
    };
    let source = format!(
        r#"
#[cfg(target_os = "{other_os}")]
struct Hidden {{}}

struct Visible {{ x: i64 }}

def main() -> i64 {{
    let v = Visible {{ x: 1 }};
    v.x
}}
"#
    );
    let result = compile_to_ir(&source);
    assert!(
        result.is_ok(),
        "visible declarations should compile after cfg filtering, got {:?}",
        result.err()
    );
}

#[test]
fn cfg_target_family_keeps_current_family() {
    let current_family = if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unix"
    };
    let source = format!(
        r#"
#[cfg(target_family = "{current_family}")]
struct Visible {{ x: i64 }}

def main() -> i64 {{
    let v = Visible {{ x: 7 }};
    v.x
}}
"#
    );

    compile_to_ir(&source).expect("current target_family cfg should compile");
}

#[test]
fn cfg_feature_defaults_false_in_standalone_mode() {
    let source = r#"
#[cfg(feature = "experimental")]
def hidden() -> i64 { missing_name }

def main() -> i64 { 0 }
"#;

    compile_to_ir(source).expect("standalone feature cfg should filter false declarations");
}

#[test]
fn cfg_feature_can_be_enabled_by_package_feature_context() {
    let source = r#"
#[cfg(feature = "experimental")]
def visible() -> i64 { 41 }

def main() -> i64 { visible() + 1 }
"#;

    parser::with_cfg_features(["experimental"], || {
        compile_to_ir(source).expect("package feature cfg should read selected feature context")
    });
}

#[test]
fn cfg_all_any_not_composes_predicates() {
    let current_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    let other_os = if current_os == "windows" {
        "linux"
    } else {
        "windows"
    };
    let source = format!(
        r#"
#[cfg(all(target_os = "{current_os}", any(target_family = "unix", target_family = "windows"), not(target_os = "{other_os}")))]
struct Visible {{ x: i64 }}

#[cfg(any(feature = "missing", target_os = "{other_os}"))]
def hidden() -> i64 {{ missing_name }}

def main() -> i64 {{
    let v = Visible {{ x: 9 }};
    v.x
}}
"#
    );

    compile_to_ir(&source).expect("composed cfg predicates should compile");
}

#[test]
fn malformed_cfg_reports_stable_attribute_diagnostic() {
    let source = r#"
#[cfg(target_arch = "x86_64")]
struct Bad {}

def main() -> i64 { 0 }
"#;

    let err = Parser::parse(source).expect_err("unsupported cfg predicate should fail");
    match err {
        CompileError::ParseError(ParseError::UnsupportedAttribute { message, .. }) => {
            assert!(message.contains("unsupported cfg predicate"));
        }
        other => panic!("expected unsupported attribute diagnostic, got {other:?}"),
    }
}

#[test]
fn unsupported_attribute_reports_stable_diagnostic() {
    let source = r#"
#[must_use]
struct Bad {}
def main() -> i64 { 0 }
"#;
    let err = Parser::parse(source).expect_err("must_use should fail");
    let message = err.to_string();
    assert!(
        message.contains("unsupported attribute"),
        "expected stable unsupported attribute diagnostic, got: {message}"
    );
}

#[test]
fn deprecated_use_emits_warning() {
    let source = r#"
#[deprecated("use new_main instead")]
def old_main() -> i64 { 1 }

def main() -> i64 {
    old_main()
}
"#;
    let program = Parser::parse(source).expect("deprecated decl should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("deprecated use should still typecheck");
    let warnings = checker.warnings();
    assert_eq!(warnings.len(), 1, "expected one deprecated-use warning");
    assert!(warnings[0].to_string().contains("deprecated"));
    assert!(warnings[0].to_string().contains("old_main"));
}

#[test]
fn deprecated_use_preserves_structured_migration_metadata() {
    let source = r#"
#[deprecated(replacement = "new_main", removal = "v0.3.0", note = "use the fallible entry point")]
def old_main() -> i64 { 1 }

def new_main() -> i64 { 2 }

def main() -> i64 { old_main() }
"#;
    let program = Parser::parse(source).expect("structured deprecated attribute should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("deprecated use should remain valid");

    let warning = checker.warnings().first().expect("deprecated warning");
    assert_eq!(warning.replacement(), Some("new_main"));
    assert_eq!(warning.removal(), Some("v0.3.0"));
    assert!(warning.to_string().contains("use the fallible entry point"));
}

#[test]
fn structured_deprecated_metadata_requires_replacement_and_removal() {
    let source = r#"
#[deprecated(replacement = "new_main")]
def old_main() -> i64 { 1 }
def main() -> i64 { old_main() }
"#;
    let error = Parser::parse(source).expect_err("missing removal horizon must fail");
    assert!(
        error
            .to_string()
            .contains("requires `replacement` and `removal`"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn derive_on_class_still_expands() {
    let source = r#"
#[derive(Auto)]
class Widget {
    id: i64;
}

def main() -> i64 {
    let w = Widget { id: 1 };
    w.__derive_auto()
}
"#;
    let mir = compile_to_mir(source).expect("derive on class should lower");
    assert!(
        mir.iter()
            .any(|func| func.name.contains("derive_auto") || func.name.contains("Widget")),
        "derive expansion should remain available for classes"
    );
}
