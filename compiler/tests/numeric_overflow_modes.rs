use sengoo_compiler::{compile_to_ir_for_target, CompileOptions, MirOptLevel};

fn compile(source: &str, target: &str, level: MirOptLevel) -> String {
    compile_to_ir_for_target(
        source,
        CompileOptions {
            mir_opt_level: level,
            runtime_contract_checks: false,
        },
        target,
    )
    .unwrap_or_else(|error| panic!("numeric fixture should compile: {error}\n{source}"))
}

#[test]
fn every_integer_width_has_checked_debug_and_wrapping_release_codegen() {
    let cases = [
        ("i8", "i8", "s", "x86_64-unknown-linux-gnu"),
        ("i16", "i16", "s", "x86_64-unknown-linux-gnu"),
        ("i32", "i32", "s", "x86_64-unknown-linux-gnu"),
        ("i64", "i64", "s", "x86_64-unknown-linux-gnu"),
        ("u8", "i8", "u", "x86_64-unknown-linux-gnu"),
        ("u16", "i16", "u", "x86_64-unknown-linux-gnu"),
        ("u32", "i32", "u", "x86_64-unknown-linux-gnu"),
        ("u64", "i64", "u", "x86_64-unknown-linux-gnu"),
        ("isize", "i32", "s", "i686-unknown-linux-gnu"),
        ("usize", "i32", "u", "i686-unknown-linux-gnu"),
        ("isize", "i64", "s", "x86_64-unknown-linux-gnu"),
        ("usize", "i64", "u", "x86_64-unknown-linux-gnu"),
    ];

    for (source_ty, llvm_ty, signedness, target) in cases {
        let source = format!(
            r#"
extern "C" {{ fn input() -> {source_ty}; }}
def main() -> {source_ty} {{
    let value = input();
    let added = value + (1 as {source_ty});
    let subtracted = added - (1 as {source_ty});
    subtracted * (2 as {source_ty})
}}
"#
        );
        let debug = compile(&source, target, MirOptLevel::O0);
        for operation in ["add", "sub", "mul"] {
            let intrinsic = format!("llvm.{signedness}{operation}.with.overflow.{llvm_ty}");
            assert!(
                debug.contains(&intrinsic),
                "{source_ty} on {target} should emit {intrinsic}:\n{debug}"
            );
        }
        assert!(debug.contains("call void @sengoo_panic_integer_overflow"));

        let release = compile(&source, target, MirOptLevel::O2);
        assert!(!release.contains("with.overflow"), "{release}");
        for operation in ["add", "sub", "mul"] {
            assert!(
                release.contains(&format!(" = {operation} {llvm_ty} ")),
                "{source_ty} on {target} should emit wrapping {operation}:\n{release}"
            );
        }
    }
}

#[test]
fn every_integer_width_checks_debug_division_and_remainder_by_zero() {
    let cases = [
        ("i8", "i8", "sdiv", "srem", "x86_64-unknown-linux-gnu"),
        ("i16", "i16", "sdiv", "srem", "x86_64-unknown-linux-gnu"),
        ("i32", "i32", "sdiv", "srem", "x86_64-unknown-linux-gnu"),
        ("i64", "i64", "sdiv", "srem", "x86_64-unknown-linux-gnu"),
        ("u8", "i8", "udiv", "urem", "x86_64-unknown-linux-gnu"),
        ("u16", "i16", "udiv", "urem", "x86_64-unknown-linux-gnu"),
        ("u32", "i32", "udiv", "urem", "x86_64-unknown-linux-gnu"),
        ("u64", "i64", "udiv", "urem", "x86_64-unknown-linux-gnu"),
        ("isize", "i32", "sdiv", "srem", "i686-unknown-linux-gnu"),
        ("usize", "i32", "udiv", "urem", "i686-unknown-linux-gnu"),
        ("isize", "i64", "sdiv", "srem", "x86_64-unknown-linux-gnu"),
        ("usize", "i64", "udiv", "urem", "x86_64-unknown-linux-gnu"),
    ];

    for (source_ty, llvm_ty, div, rem, target) in cases {
        let source = format!(
            r#"
extern "C" {{ fn input() -> {source_ty}; }}
def main() -> {source_ty} {{
    let divisor = input();
    ((84 as {source_ty}) / divisor) + ((84 as {source_ty}) % divisor)
}}
"#
        );
        let debug = compile(&source, target, MirOptLevel::O0);
        assert_eq!(
            debug
                .matches("call void @sengoo_panic_division_by_zero")
                .count(),
            2,
            "{source_ty} should check both division and remainder:\n{debug}"
        );
        assert!(debug.contains(&format!("{div} {llvm_ty}")), "{debug}");
        assert!(debug.contains(&format!("{rem} {llvm_ty}")), "{debug}");

        let release = compile(&source, target, MirOptLevel::O2);
        assert!(!release.contains("call void @sengoo_panic_division_by_zero"));
        assert!(release.contains(&format!("{div} {llvm_ty}")), "{release}");
        assert!(release.contains(&format!("{rem} {llvm_ty}")), "{release}");
    }
}
