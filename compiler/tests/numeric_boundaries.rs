use sengoo_compiler::{
    compile_to_ir, compile_to_ir_for_target, CompileOptions, TargetPointerWidth,
};

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
fn based_separated_and_boundary_literals_compile() {
    let source = r#"
def main() -> i64 {
    let binary = 0b1010_1010u16;
    let octal = 0o52i16;
    let hexadecimal = 0xff_ffu32;
    let decimal = 1_000_000i64;
    let signed_min = -9223372036854775808i64;
    let unsigned_max = 18446744073709551615u64;
    let narrow_min = -128i8;
    let narrow_max = 255u8;
    if binary == 170u16
        && octal == 42i16
        && hexadecimal == 65535u32
        && decimal == 1000000i64
        && signed_min < 0i64
        && unsigned_max > 0u64
        && narrow_min < 0i8
        && narrow_max == 255u8 {
        0
    } else {
        1
    }
}
"#;

    compile_to_ir(source).expect("valid based, separated, and boundary literals should compile");
}

#[test]
fn malformed_or_out_of_range_literals_have_stable_diagnostics() {
    for (literal, expected) in [
        ("1__0", "invalid numeric literal"),
        ("1_", "invalid numeric literal"),
        ("0x_FF", "invalid numeric literal"),
        ("42u128", "invalid numeric literal"),
        ("18446744073709551616u64", "invalid numeric literal"),
        ("256u8", "exceeds range of `u8`"),
        ("128i8", "exceeds range of `i8`"),
        ("32768i16", "exceeds range of `i16`"),
        ("2147483648i32", "exceeds range of `i32`"),
        ("9223372036854775808i64", "exceeds range of `i64`"),
    ] {
        let source = format!("def main() -> i64 {{ let value = {literal}; 0 }}");
        let error = compile_to_ir(&source)
            .expect_err("malformed or out-of-range literal should fail")
            .to_string();
        assert!(
            error.contains(expected),
            "literal {literal} should report {expected:?}, got: {error}"
        );
    }
}

#[test]
fn pointer_sized_integers_follow_the_selected_32_or_64_bit_target() {
    let source = r#"
def signed(value: isize) -> isize { value }
def unsigned(value: usize) -> usize { value }
def main() -> isize { signed(unsigned(7usize) as isize) }
"#;

    let ir32 =
        compile_to_ir_for_target(source, CompileOptions::default(), "i686-unknown-linux-gnu")
            .expect("32-bit target should compile pointer-sized integers");
    assert!(ir32.contains("target triple = \"i686-unknown-linux-gnu\""));
    assert!(ir32.contains("define i32 @signed(i32"), "{ir32}");
    assert!(ir32.contains("define i32 @unsigned(i32"), "{ir32}");

    let ir64 = compile_to_ir_for_target(
        source,
        CompileOptions::default(),
        "x86_64-unknown-linux-gnu",
    )
    .expect("64-bit target should compile pointer-sized integers");
    assert!(ir64.contains("target triple = \"x86_64-unknown-linux-gnu\""));
    assert!(ir64.contains("define i64 @signed(i64"), "{ir64}");
    assert!(ir64.contains("define i64 @unsigned(i64"), "{ir64}");

    assert_eq!(
        TargetPointerWidth::from_target_triple("wasm32-unknown-unknown"),
        Some(TargetPointerWidth::Bits32)
    );
    assert_eq!(
        TargetPointerWidth::from_target_triple("aarch64-apple-darwin"),
        Some(TargetPointerWidth::Bits64)
    );
}

#[test]
fn every_integer_width_compares_and_casts_on_the_production_backend() {
    let cases = [
        ("i8", "i8", "slt", "x86_64-unknown-linux-gnu"),
        ("i16", "i16", "slt", "x86_64-unknown-linux-gnu"),
        ("i32", "i32", "slt", "x86_64-unknown-linux-gnu"),
        ("i64", "i64", "slt", "x86_64-unknown-linux-gnu"),
        ("u8", "i8", "ult", "x86_64-unknown-linux-gnu"),
        ("u16", "i16", "ult", "x86_64-unknown-linux-gnu"),
        ("u32", "i32", "ult", "x86_64-unknown-linux-gnu"),
        ("u64", "i64", "ult", "x86_64-unknown-linux-gnu"),
        ("isize", "i32", "slt", "i686-unknown-linux-gnu"),
        ("usize", "i32", "ult", "i686-unknown-linux-gnu"),
    ];

    for (source_ty, llvm_ty, predicate, target) in cases {
        let source = format!(
            r#"
extern "C" {{ fn input() -> {source_ty}; }}
def main() -> i64 {{
    let value = input();
    if value < (7 as {source_ty}) {{ value as i64 }} else {{ 0 }}
}}
"#
        );
        let ir = compile_to_ir_for_target(&source, CompileOptions::default(), target)
            .unwrap_or_else(|error| panic!("{source_ty} should compile for {target}: {error}"));
        assert!(
            ir.contains(&format!("icmp {predicate} {llvm_ty}")),
            "{source_ty} should use {predicate} on {target}:\n{ir}"
        );
    }
}

#[test]
fn pointer_sized_literal_bounds_are_target_specific() {
    compile_to_ir_for_target(
        "def main() -> usize { 4294967295usize }",
        CompileOptions::default(),
        "i686-unknown-linux-gnu",
    )
    .expect("u32::MAX should fit 32-bit usize");

    let usize_error = compile_to_ir_for_target(
        "def main() -> usize { 4294967296usize }",
        CompileOptions::default(),
        "i686-unknown-linux-gnu",
    )
    .expect_err("2^32 must not fit 32-bit usize")
    .to_string();
    assert!(
        usize_error.contains("exceeds range of `usize`"),
        "{usize_error}"
    );

    compile_to_ir_for_target(
        "def main() -> isize { -2147483648isize }",
        CompileOptions::default(),
        "i686-unknown-linux-gnu",
    )
    .expect("i32::MIN should fit 32-bit isize");

    let isize_error = compile_to_ir_for_target(
        "def main() -> isize { 2147483648isize }",
        CompileOptions::default(),
        "i686-unknown-linux-gnu",
    )
    .expect_err("positive 2^31 must not fit 32-bit isize")
    .to_string();
    assert!(
        isize_error.contains("exceeds range of `isize`"),
        "{isize_error}"
    );
}

#[test]
fn checked_u64_conversion_family_covers_signed_and_pointer_targets() {
    let source = with_math(
        r#"
def main() -> i64 {
    let fits_i64 = checked_u64_to_i64(9223372036854775807u64).is_ok;
    let over_i64 = !checked_u64_to_i64(9223372036854775808u64).is_ok;
    let fits_usize = checked_u64_to_usize(7u64).is_ok;
    let fits_isize = checked_u64_to_isize(7u64).is_ok;
    if fits_i64 && over_i64 && fits_usize && fits_isize { 0 } else { 1 }
}
"#,
    );

    compile_to_ir(&source).expect("u64 checked conversions should be part of std::math");
}

#[test]
fn checked_conversion_family_exposes_every_non_identity_integer_pair() {
    let types = [
        "i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize",
    ];
    let mut body = String::from("def main() -> i64 {\n");
    for source in types {
        for target in types {
            if source != target {
                body.push_str(&format!(
                    "    let converted_{source}_{target} = checked_{source}_to_{target}(0{source});\n"
                ));
            }
        }
    }
    body.push_str("    0\n}\n");

    compile_to_ir(&with_math(&body))
        .expect("every non-identity integer pair should expose a checked conversion");
}

#[test]
fn std_math_pointer_helpers_compile_for_a_32_bit_target() {
    let source = with_math(
        r#"
def main() -> i64 {
    let max = 4294967295usize;
    let wrapped = max.wrapping_add(1usize);
    let checked = max.checked_add(1usize);
    let fits = checked_u64_to_usize(4294967295u64);
    let overflow = checked_u64_to_usize(4294967296u64);
    if wrapped == 0usize && checked.is_none() && fits.is_ok && !overflow.is_ok {
        0
    } else {
        1
    }
}
"#,
    );

    compile_to_ir_for_target(&source, CompileOptions::default(), "i686-unknown-linux-gnu")
        .expect("std::math pointer-sized helpers should honor the selected 32-bit target");
}
