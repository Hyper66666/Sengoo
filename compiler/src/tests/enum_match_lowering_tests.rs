//! Runtime lowering for enum match arms.

use crate::compile_to_ir;

#[test]
fn enum_match_dispatches_by_variant_discriminant() {
    let ir = compile_to_ir(
        r#"
enum Color { Red, Green }
def pick(c: Color) -> i64 {
    match c {
        Color::Red => 1,
        Color::Green => 2,
        _ => 0,
    }
}
def main() -> i64 { 0 }
"#,
    )
    .expect("enum match should compile");
    assert!(
        ir.contains("switch") || ir.contains("Discriminant"),
        "expected enum dispatch in IR:\n{ir}"
    );
}

#[test]
fn enum_or_pattern_switch_maps_both_variants() {
    let ir = compile_to_ir(
        r#"
enum Color { Red, Green, Blue }
def pick(c: Color) -> i64 {
    match c {
        Color::Red => 1,
        Color::Green | Color::Blue => 2,
        _ => 0,
    }
}
def main() -> i64 { 0 }
"#,
    )
    .expect("or-pattern enum match should compile");
    assert!(
        ir.contains("switch") && ir.contains("i64 1") && ir.contains("i64 2"),
        "expected switch with Green/Blue targets in IR:\n{ir}"
    );
}

#[test]
fn match_guard_filters_arm() {
    let ir = compile_to_ir(
        r#"
def main() -> i64 {
    let x = 2;
    match x {
        y if y > 1 => 10,
        _ => 0,
    }
}
"#,
    )
    .expect("guarded match should compile");
    assert!(ir.contains("br i1"), "expected guard branch in IR:\n{ir}");
}
