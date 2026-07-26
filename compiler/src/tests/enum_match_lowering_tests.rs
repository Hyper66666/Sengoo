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

#[test]
fn generic_enum_payload_free_variant_infers_from_annotation() {
    let ir = compile_to_ir(
        r#"
enum MyOption<T> { MyNone, MySome(T) }
def main() -> i64 {
    let x: MyOption<i64> = MyOption::MyNone;
    match x {
        MyOption::MyNone => 7,
        MyOption::MySome(inner) => inner,
    }
}
"#,
    )
    .expect("annotated payload-free generic variant should compile");
    assert!(
        ir.contains("switch") || ir.contains("Discriminant"),
        "expected enum dispatch in IR:\n{ir}"
    );
}

#[test]
fn generic_enum_payload_free_variant_infers_from_return_type() {
    compile_to_ir(
        r#"
enum MyOption<T> { MyNone, MySome(T) }
def pick(flag: bool, v: i64) -> MyOption<i64> {
    if flag { MyOption::MySome(v) } else { MyOption::MyNone }
}
def main() -> i64 { 0 }
"#,
    )
    .expect("return-position payload-free generic variant should compile");
}

#[test]
fn bare_variant_expressions_and_patterns_resolve_via_unique_owner() {
    compile_to_ir(
        r#"
enum MyOption<T> { MyNone, MySome(T) }
def find(n: i64) -> MyOption<i64> {
    if n > 0 { MySome(n) } else { MyNone }
}
def main() -> i64 {
    match find(41) {
        MyNone => 0,
        MySome(v) => v + 1,
    }
}
"#,
    )
    .expect("bare Some/None-style variants should resolve to their unique owner");
}

#[test]
fn bare_variant_shared_by_two_enums_is_ambiguous() {
    let err = compile_to_ir(
        r#"
enum A { Hit, Miss }
enum B { Hit, Stop }
def main() -> i64 {
    let x = Hit;
    0
}
"#,
    )
    .expect_err("a variant declared by two enums must not resolve bare");
    let text = format!("{err:?}");
    assert!(
        text.contains("ambiguous-enum-variant") || text.contains("qualify"),
        "expected ambiguity diagnostic, got:\n{text}"
    );
}

#[test]
fn question_mark_propagates_user_enum_result_and_option() {
    compile_to_ir(
        r#"
enum Result<T, E> { Ok(T), Err(E) }
enum Option<T> { None, Some(T) }
def divide(a: i64, b: i64) -> Result<i64, i64> {
    if b == 0 { Err(1) } else { Ok(a / b) }
}
def chain(a: i64, b: i64) -> Result<i64, i64> {
    let v = divide(a, b)?;
    Ok(v + 100)
}
def find(n: i64) -> Option<i64> {
    if n > 0 { Some(n) } else { None }
}
def bump(n: i64) -> Option<i64> {
    let v = find(n)?;
    Some(v + 1)
}
def main() -> i64 { 0 }
"#,
    )
    .expect("`?` should propagate enum-shaped Result and Option");
}

#[test]
fn single_element_struct_payload_binds_whole_in_match_arm() {
    compile_to_ir(
        r#"
struct Wrap { handle: i64 }
enum Holder { Empty, Has(Wrap) }
def main() -> i64 {
    let a = Holder::Has(Wrap { handle: 3 });
    match a {
        Holder::Empty => 0,
        Holder::Has(w) => w.handle,
    }
}
"#,
    )
    .expect("single struct payload must bind whole, not split per field");
}
