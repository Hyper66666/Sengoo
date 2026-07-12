use sengoo_compiler::{compile_to_ir, Parser, TypeChecker};

fn with_math(body: &str) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        include_str!("../../tools/stdlib/option.sg"),
        include_str!("../../tools/stdlib/result.sg"),
        include_str!("../../tools/stdlib/ffi.sg"),
        include_str!("../../tools/stdlib/status.sg"),
        include_str!("../../tools/stdlib/math.sg"),
        body
    )
}

#[test]
fn into_uses_let_annotation_to_select_the_lossless_target() {
    let source = with_math(
        r#"
def main() -> i64 {
    let widened: i16 = (7i8).into();
    widened as i64
}
"#,
    );

    let ir = compile_to_ir(&source).expect("let annotation should select Into<i16> for i8");
    assert!(ir.contains("@i8_Into_i16_into"), "{ir}");
}

#[test]
fn into_uses_return_type_to_select_the_lossless_target() {
    let source = with_math(
        r#"
def widen_tail(value: i8) -> i32 {
    value.into()
}

def widen_explicit(value: i8) -> i32 {
    return value.into();
}

def main() -> i64 {
    (widen_tail(7i8) + widen_explicit(8i8)) as i64
}
"#,
    );

    let ir = compile_to_ir(&source).expect("return type should select Into<i32> for i8");
    assert!(ir.matches("@i8_Into_i32_into").count() >= 2, "{ir}");
}

#[test]
fn into_uses_function_parameter_to_select_the_lossless_target() {
    let source = with_math(
        r#"
def consume(value: i64) -> i64 { value }

def main() -> i64 {
    consume((7i8).into())
}
"#,
    );

    let ir = compile_to_ir(&source).expect("parameter type should select Into<i64> for i8");
    assert!(ir.contains("@i8_Into_i64_into"), "{ir}");
}

#[test]
fn from_is_callable_on_the_destination_type() {
    let source = with_math(
        r#"
def main() -> i64 {
    i64::from_value(7i8)
}
"#,
    );

    let ir =
        compile_to_ir(&source).expect("destination type should expose From<Source>::from_value");
    assert!(ir.contains("@i64_From_i8_from_value"), "{ir}");
}

#[test]
fn std_math_exposes_the_complete_lossless_widening_matrix() {
    let source = with_math(
        r#"
def main() -> i64 {
    let i8_i16: i16 = (1i8).into();
    let i8_i32: i32 = (1i8).into();
    let i8_i64: i64 = (1i8).into();
    let i8_isize: isize = (1i8).into();
    let i16_i32: i32 = (1i16).into();
    let i16_i64: i64 = (1i16).into();
    let i16_isize: isize = (1i16).into();
    let i32_i64: i64 = (1i32).into();
    let i32_isize: isize = (1i32).into();
    let isize_i64: i64 = (1isize).into();

    let u8_u16: u16 = (1u8).into();
    let u8_u32: u32 = (1u8).into();
    let u8_u64: u64 = (1u8).into();
    let u8_usize: usize = (1u8).into();
    let u8_i16: i16 = (1u8).into();
    let u8_i32: i32 = (1u8).into();
    let u8_i64: i64 = (1u8).into();
    let u8_isize: isize = (1u8).into();
    let u16_u32: u32 = (1u16).into();
    let u16_u64: u64 = (1u16).into();
    let u16_usize: usize = (1u16).into();
    let u16_i32: i32 = (1u16).into();
    let u16_i64: i64 = (1u16).into();
    let u16_isize: isize = (1u16).into();
    let u32_u64: u64 = (1u32).into();
    let u32_usize: usize = (1u32).into();
    let u32_i64: i64 = (1u32).into();
    let usize_u64: u64 = (1usize).into();
    let f32_f64: f64 = (1.5f32).into();

    let from_i8_i16 = i16::from_value(1i8);
    let from_i8_i32 = i32::from_value(1i8);
    let from_i8_i64 = i64::from_value(1i8);
    let from_i8_isize = isize::from_value(1i8);
    let from_i16_i32 = i32::from_value(1i16);
    let from_i16_i64 = i64::from_value(1i16);
    let from_i16_isize = isize::from_value(1i16);
    let from_i32_i64 = i64::from_value(1i32);
    let from_i32_isize = isize::from_value(1i32);
    let from_isize_i64 = i64::from_value(1isize);
    let from_u8_u16 = u16::from_value(1u8);
    let from_u8_u32 = u32::from_value(1u8);
    let from_u8_u64 = u64::from_value(1u8);
    let from_u8_usize = usize::from_value(1u8);
    let from_u8_i16 = i16::from_value(1u8);
    let from_u8_i32 = i32::from_value(1u8);
    let from_u8_i64 = i64::from_value(1u8);
    let from_u8_isize = isize::from_value(1u8);
    let from_u16_u32 = u32::from_value(1u16);
    let from_u16_u64 = u64::from_value(1u16);
    let from_u16_usize = usize::from_value(1u16);
    let from_u16_i32 = i32::from_value(1u16);
    let from_u16_i64 = i64::from_value(1u16);
    let from_u16_isize = isize::from_value(1u16);
    let from_u32_u64 = u64::from_value(1u32);
    let from_u32_usize = usize::from_value(1u32);
    let from_u32_i64 = i64::from_value(1u32);
    let from_usize_u64 = u64::from_value(1usize);
    let from_f32_f64 = f64::from_value(1.5f32);

    0
}
"#,
    );

    let ir = compile_to_ir(&source).expect("every documented widening should compile");
    for expected in [
        "@i8_Into_isize_into",
        "@u8_Into_usize_into",
        "@f32_Into_f64_into",
        "@isize_From_i8_from_value",
        "@usize_From_u32_from_value",
        "@f64_From_f32_from_value",
    ] {
        assert!(ir.contains(expected), "missing {expected} in IR:\n{ir}");
    }
}

#[test]
fn narrowing_has_no_from_or_into_candidate() {
    let source = with_math(
        r#"
def main() -> i32 {
    let narrowed: i32 = (7i64).into();
    narrowed
}
"#,
    );

    let program = Parser::parse(&source).expect("narrowing probe should parse");
    let mut checker = TypeChecker::new();
    let error = checker
        .check_program(&program)
        .expect_err("lossy narrowing must not receive an Into implementation")
        .to_string();
    assert!(
        error.contains("Into<i32>") || error.contains("method 'into' not found"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn narrowing_has_no_from_associated_function() {
    let source = with_math(
        r#"
def main() -> i32 {
    i32::from_value(7i64)
}
"#,
    );

    let program = Parser::parse(&source).expect("narrowing From probe should parse");
    let mut checker = TypeChecker::new();
    let error = checker
        .check_program(&program)
        .expect_err("lossy narrowing must not receive a From implementation")
        .to_string();
    assert!(
        error.contains("i32::from_value"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn target_dependent_u32_to_isize_has_no_from_or_into_candidate() {
    let into_source = with_math(
        r#"
def main() -> isize {
    let value: isize = (7u32).into();
    value
}
"#,
    );
    let program = Parser::parse(&into_source).expect("u32 to isize probe should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect_err("u32 to isize is not lossless on every supported target");

    let from_source = with_math(
        r#"
def main() -> isize {
    isize::from_value(7u32)
}
"#,
    );
    let program = Parser::parse(&from_source).expect("u32 From probe should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect_err("From<u32> for isize must not be portable");
}
