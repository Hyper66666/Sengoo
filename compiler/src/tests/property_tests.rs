//! Property-based tests for the Sengoo compiler
//!
//! Uses `proptest` to verify correctness properties across many random inputs.

use proptest::prelude::*;
use std::collections::HashSet;

use crate::{compile_to_ir, Keyword};

// ============================================================================
// Property 10: Compilation pipeline produces valid output or descriptive error
//
// *For any* Sengoo source string, the `compile_to_ir` function SHALL either
// return a valid LLVM IR string (for valid programs) or return a `CompileError`
// with a descriptive message indicating the compilation stage and nature of the
// error (for invalid programs). The function SHALL never panic.
//
// **Validates: Requirements 6.2, 6.3**
// ============================================================================

/// Strategy to generate random arbitrary strings (including invalid source code).
/// These test that the compiler never panics on any input.
fn arbitrary_source_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Completely random strings
        ".*",
        // Random ASCII strings
        "[a-zA-Z0-9 \t\n\\{\\}\\(\\)\\+\\-\\*/=<>;:,\\.!@#$%^&]*",
        // Empty and whitespace-only strings
        "[ \t\n]*",
        // Strings with Sengoo keywords mixed with random content
        prop_oneof![
            Just("def".to_string()),
            Just("let".to_string()),
            Just("if".to_string()),
            Just("else".to_string()),
            Just("while".to_string()),
            Just("loop".to_string()),
            Just("break".to_string()),
            Just("return".to_string()),
            Just("struct".to_string()),
            Just("impl".to_string()),
            Just("trait".to_string()),
            Just("fn".to_string()),
        ],
        // Partial/malformed programs
        Just("def".to_string()),
        Just("def main".to_string()),
        Just("def main()".to_string()),
        Just("def main() ->".to_string()),
        Just("def main() -> i64".to_string()),
        Just("def main() -> i64 {".to_string()),
        Just("def main() -> i64 { }".to_string()),
        Just("def main() -> i64 { 42".to_string()),
    ]
}

/// Strategy to generate valid Sengoo programs that should compile successfully.
/// These programs follow the pattern: `def main() -> i64 { <expr> }`
fn valid_program_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple integer return values
        valid_integer_program_strategy(),
        // Arithmetic expressions
        valid_arithmetic_program_strategy(),
        // Variable binding programs
        valid_variable_program_strategy(),
        // If-else programs
        valid_if_else_program_strategy(),
    ]
}

/// Generate programs that return a simple integer literal.
fn valid_integer_program_strategy() -> impl Strategy<Value = String> {
    // Use a range that avoids i64 overflow issues in the source representation
    (-999_999i64..=999_999i64).prop_map(|n| {
        if n < 0 {
            // Sengoo may not support negative literals directly;
            // use (0 - abs_val) pattern instead
            format!("def main() -> i64 {{ 0 - {} }}", n.unsigned_abs())
        } else {
            format!("def main() -> i64 {{ {} }}", n)
        }
    })
}

/// Generate programs with simple arithmetic expressions.
fn valid_arithmetic_program_strategy() -> impl Strategy<Value = String> {
    (
        1i64..=1000,
        1i64..=1000,
        prop_oneof![Just("+"), Just("-"), Just("*")],
    )
        .prop_map(|(a, b, op)| format!("def main() -> i64 {{ {} {} {} }}", a, op, b))
}

/// Generate programs with variable bindings.
fn valid_variable_program_strategy() -> impl Strategy<Value = String> {
    (1i64..=1000, 1i64..=1000).prop_map(|(a, b)| {
        format!(
            "def main() -> i64 {{\n    let x = {};\n    let y = {};\n    x + y\n}}",
            a, b
        )
    })
}

/// Generate programs with if-else expressions.
fn valid_if_else_program_strategy() -> impl Strategy<Value = String> {
    (1i64..=100, 1i64..=100, 1i64..=1000, 1i64..=1000).prop_map(|(a, b, then_val, else_val)| {
        format!(
            "def main() -> i64 {{\n    let x = {};\n    if x > {} {{ {} }} else {{ {} }}\n}}",
            a, b, then_val, else_val
        )
    })
}

/// Strategy to generate strings that look like Sengoo code but have subtle errors.
fn near_valid_source_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Missing return type
        (0i64..=100).prop_map(|n| format!("def main() {{ {} }}", n)),
        // Wrong return type keyword
        (0i64..=100).prop_map(|n| format!("def main() -> int {{ {} }}", n)),
        // Missing closing brace
        (0i64..=100).prop_map(|n| format!("def main() -> i64 {{ {}", n)),
        // Extra tokens after program
        (0i64..=100).prop_map(|n| format!("def main() -> i64 {{ {} }} extra_stuff", n)),
        // Invalid identifier as function name
        Just("def 123invalid() -> i64 { 42 }".to_string()),
        // Multiple functions (may or may not be valid)
        (0i64..=100, 0i64..=100).prop_map(|(a, b)| {
            format!(
                "def foo() -> i64 {{ {} }}\ndef main() -> i64 {{ {} }}",
                a, b
            )
        }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// **Validates: Requirements 6.2, 6.3**
    ///
    /// Property 10: For any arbitrary source string, `compile_to_ir` SHALL
    /// either return Ok with a valid LLVM IR string or Err with a descriptive
    /// CompileError. It SHALL never panic.
    #[test]
    fn prop_compile_to_ir_never_panics_on_arbitrary_input(
        source in arbitrary_source_strategy()
    ) {
        // The function should never panic — it must always return Ok or Err
        let result = compile_to_ir(&source);
        // We just verify it returns a Result without panicking
        match &result {
            Ok(ir) => {
                // If compilation succeeds, the IR should be non-empty
                prop_assert!(!ir.is_empty(), "Successful compilation produced empty IR");
            }
            Err(_) => {
                // Errors are expected for invalid input — this is fine
            }
        }
    }

    /// **Validates: Requirements 6.2, 6.3**
    ///
    /// Property 10: For any valid Sengoo program (well-formed `def main() -> i64 { <expr> }`),
    /// `compile_to_ir` SHALL return Ok with a non-empty LLVM IR string containing
    /// a `main` function definition.
    #[test]
    fn prop_valid_programs_compile_successfully(
        source in valid_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "Valid program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();
        prop_assert!(!ir.is_empty(), "Compiled IR is empty for source: {}", source);
        // The IR should contain a main function definition
        prop_assert!(
            ir.contains("@main") || ir.contains("define"),
            "Compiled IR does not contain expected function definition.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }

    /// **Validates: Requirements 6.2, 6.3**
    ///
    /// Property 10: For near-valid source strings (syntactically close to valid
    /// programs but with subtle errors), `compile_to_ir` SHALL return a Result
    /// (Ok or Err) without panicking.
    #[test]
    fn prop_near_valid_programs_do_not_panic(
        source in near_valid_source_strategy()
    ) {
        // Should not panic regardless of whether it succeeds or fails
        let result = compile_to_ir(&source);
        match &result {
            Ok(ir) => {
                prop_assert!(!ir.is_empty(), "Successful compilation produced empty IR");
            }
            Err(_) => {
                // Errors are acceptable for near-valid input
            }
        }
    }
}

// ============================================================================
// Property 8: `for x in arr` should keep generated SSA names unique
//
// **Validates: Requirements 6.1, 6.2, 6.3**
// ============================================================================

fn for_loop_ssa_program_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec(0i64..=50, 2..=8).prop_map(|values| {
        let elems = values
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "def main() -> i64 {{\n    let mut acc = 0;\n    for x in [{}] {{\n        acc = acc + x;\n    }}\n    acc\n}}",
            elems
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_for_loop_generates_unique_ssa_names(
        source in for_loop_ssa_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "for-loop program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();

        let mut seen = HashSet::new();
        for line in ir.lines() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with('%') {
                continue;
            }
            if let Some((lhs, _)) = trimmed.split_once(" = ") {
                prop_assert!(
                    seen.insert(lhs.to_string()),
                    "Duplicate SSA definition `{}` found.\nSource: {}\nIR: {}",
                    lhs,
                    source,
                    ir
                );
            }
        }

        prop_assert!(
            ir.contains("getelementptr") && ir.contains("icmp slt"),
            "for-loop IR is missing expected loop/index structure.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }
}

// ============================================================================
// Property 9: `print(struct_value)` should lower to runtime print call sequence
//
// **Validates: Requirements 7.1, 7.2**
// ============================================================================

fn print_struct_program_strategy() -> impl Strategy<Value = String> {
    (0i64..=999, 0i64..=999).prop_map(|(x, y)| {
        format!(
            "struct Point {{ x: i64, y: i64 }}\n\
             def main() -> i64 {{\n\
                 let p = Point {{ x: {}, y: {} }};\n\
                 print(p);\n\
                 0\n\
             }}",
            x, y
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_print_struct_generates_runtime_call_sequence(
        source in print_struct_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "print(struct) program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();

        prop_assert!(
            ir.contains("@sengoo_print_str("),
            "Expected string print calls for struct formatting.\nSource: {}\nIR: {}",
            source,
            ir
        );
        prop_assert!(
            ir.contains("@sengoo_print_i64("),
            "Expected field print calls for struct numeric fields.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }
}

// ============================================================================
// Property 1: Print call generates correct runtime function
//
// *For any* Sengoo program containing a `print(expr)` call where `expr` has a
// supported type (i64, bool, f64, or string literal), compiling through the
// full pipeline SHALL produce LLVM IR containing a call to the type-specific
// runtime function (`sengoo_print_i64`, `sengoo_print_bool`, `sengoo_print_f64`,
// or `sengoo_print_str` respectively).
//
// **Validates: Requirements 1.1, 1.3, 1.4, 1.6**
// ============================================================================

/// Strategy to generate programs with `print(N)` where N is a random i64 value.
/// Programs follow the pattern: `def main() -> i64 { print(N); 0 }`
fn print_i64_program_strategy() -> impl Strategy<Value = String> {
    (0i64..=999_999).prop_map(|n| format!("def main() -> i64 {{ print({}); 0 }}", n))
}

/// Strategy to generate programs with `print(F)` where F is a random f64 value.
/// Programs follow the pattern: `def main() -> i64 { print(F); 0 }`
/// Float literals always include a decimal point (e.g., `3.14`).
fn print_f64_program_strategy() -> impl Strategy<Value = String> {
    // Generate integer and fractional parts separately to ensure valid float syntax
    (0u32..=9999, 1u32..=99).prop_map(|(int_part, frac_part)| {
        format!(
            "def main() -> i64 {{ print({}.{}); 0 }}",
            int_part, frac_part
        )
    })
}

/// Strategy to generate programs with `print("S")` where S is a random safe string.
/// Programs follow the pattern: `def main() -> i64 { print("S"); 0 }`
/// Strings use only simple ASCII characters to avoid parsing issues.
fn print_str_program_strategy() -> impl Strategy<Value = String> {
    // Generate simple ASCII strings: letters, digits, spaces
    "[a-zA-Z0-9 ]{1,20}".prop_map(|s| format!("def main() -> i64 {{ print(\"{}\"); 0 }}", s))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 1.1, 1.6**
    ///
    /// Property 1: For any program containing `print(N)` where N is an i64 value,
    /// the generated LLVM IR SHALL contain a call to `sengoo_print_i64`.
    #[test]
    fn prop_print_i64_generates_correct_runtime_call(
        source in print_i64_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "print(i64) program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();
        prop_assert!(
            ir.contains("call void @sengoo_print_i64("),
            "IR does not contain call to sengoo_print_i64.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }

    /// **Validates: Requirements 1.3, 1.6**
    ///
    /// Property 1: For any program containing `print(F)` where F is an f64 value,
    /// the generated LLVM IR SHALL contain a call to `sengoo_print_f64`.
    #[test]
    fn prop_print_f64_generates_correct_runtime_call(
        source in print_f64_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "print(f64) program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();
        prop_assert!(
            ir.contains("call void @sengoo_print_f64("),
            "IR does not contain call to sengoo_print_f64.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }

    /// **Validates: Requirements 1.4, 1.6**
    ///
    /// Property 1: For any program containing `print("S")` where S is a string literal,
    /// the generated LLVM IR SHALL contain a call to `sengoo_print_str`.
    #[test]
    fn prop_print_str_generates_correct_runtime_call(
        source in print_str_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "print(str) program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();
        prop_assert!(
            ir.contains("call void @sengoo_print_str("),
            "IR does not contain call to sengoo_print_str.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }
}

// ============================================================================
// Property 2: While loop generates correct block structure
//
// *For any* Sengoo program containing a `while cond { body }` loop, the
// generated MIR SHALL contain at least three distinct basic blocks (condition,
// body, exit) where the condition block ends with a conditional branch to
// either the body block or the exit block, and the body block branches back
// to the condition block.
//
// **Validates: Requirements 2.1, 2.2**
// ============================================================================

/// Strategy to generate programs with a while loop: `def main() -> i64 { let mut x = 0; while x < N { x = x + 1; } x }`
/// where N is a random positive integer. The loop iterates x from 0 up to N.
fn while_loop_program_strategy() -> impl Strategy<Value = String> {
    (1u32..=100).prop_map(|n| {
        format!(
            "def main() -> i64 {{ let mut x = 0; while x < {} {{ x = x + 1; }} x }}",
            n
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 2.1, 2.2**
    ///
    /// Property 2: For any program containing a `while x < N { x = x + 1; }` loop
    /// where N is a positive integer, the generated LLVM IR SHALL contain:
    /// - A conditional branch (`br i1`) for the while condition check
    /// - An unconditional branch (`br label`) for the loop-back from body to condition
    /// - A comparison instruction (`icmp slt`) for the `<` condition
    #[test]
    fn prop_while_loop_generates_correct_block_structure(
        source in while_loop_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "While loop program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();

        // The while condition `x < N` must produce an `icmp slt` comparison
        prop_assert!(
            ir.contains("icmp slt"),
            "IR does not contain 'icmp slt' for the while condition.\nSource: {}\nIR: {}",
            source,
            ir
        );

        // The condition block must end with a conditional branch (`br i1`)
        // that directs control to either the body block or the exit block
        prop_assert!(
            ir.contains("br i1"),
            "IR does not contain 'br i1' for the conditional branch.\nSource: {}\nIR: {}",
            source,
            ir
        );

        // The body block must contain an unconditional branch (`br label`)
        // to loop back to the condition block
        prop_assert!(
            ir.contains("br label"),
            "IR does not contain 'br label' for the unconditional loop-back branch.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }
}

// ============================================================================
// Property 4: Break and continue target correct blocks in nested loops
//
// *For any* Sengoo program with nested loops (any combination of while/loop/for),
// a `break` statement SHALL generate a branch to the exit block of the innermost
// enclosing loop, and a `continue` statement SHALL generate a branch to the
// condition block (for while) or body start (for loop) of the innermost
// enclosing loop.
//
// **Validates: Requirements 2.4, 2.5, 2.6**
// ============================================================================

/// Strategy to generate programs with nested while loops containing break.
/// Pattern: `def main() -> i64 { let mut x = 0; while x < N { let mut y = 0; while y < M { if y > K { break; } y = y + 1; } x = x + 1; } x }`
/// where N, M, K are random positive integers with K < M to ensure the break is reachable.
fn nested_while_break_program_strategy() -> impl Strategy<Value = String> {
    (1u32..=20, 2u32..=20).prop_flat_map(|(n, m)| {
        // K must be less than M so the break condition is reachable
        let k_max = m - 1;
        (Just(n), Just(m), 0u32..=k_max)
    }).prop_map(|(n, m, k)| {
        format!(
            "def main() -> i64 {{ let mut x = 0; while x < {} {{ let mut y = 0; while y < {} {{ if y > {} {{ break; }} y = y + 1; }} x = x + 1; }} x }}",
            n, m, k
        )
    })
}

/// Strategy to generate programs with nested while loops containing continue.
/// Pattern: `def main() -> i64 { let mut x = 0; while x < N { let mut y = 0; while y < M { y = y + 1; if y > K { continue; } } x = x + 1; } x }`
/// where N, M, K are random positive integers with K < M.
fn nested_while_continue_program_strategy() -> impl Strategy<Value = String> {
    (1u32..=20, 2u32..=20).prop_flat_map(|(n, m)| {
        let k_max = m - 1;
        (Just(n), Just(m), 0u32..=k_max)
    }).prop_map(|(n, m, k)| {
        format!(
            "def main() -> i64 {{ let mut x = 0; let mut total = 0; while x < {} {{ let mut y = 0; while y < {} {{ y = y + 1; if y > {} {{ continue; }} total = total + 1; }} x = x + 1; }} total }}",
            n, m, k
        )
    })
}

/// Strategy to generate programs with mixed nested loops (while + loop) containing break.
/// Pattern: `def main() -> i64 { let mut x = 0; while x < N { loop { x = x + 1; if x > K { break; } } } x }`
/// where N and K are random positive integers with K < N.
fn while_loop_break_program_strategy() -> impl Strategy<Value = String> {
    (2u32..=20).prop_flat_map(|n| {
        let k_max = n - 1;
        (Just(n), 1u32..=k_max)
    }).prop_map(|(n, k)| {
        format!(
            "def main() -> i64 {{ let mut x = 0; while x < {} {{ loop {{ x = x + 1; if x > {} {{ break; }} }} }} x }}",
            n, k
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 2.4, 2.5, 2.6**
    ///
    /// Property 4: For any program with nested while loops where the inner loop
    /// contains a `break` guarded by `if y > K`, the generated LLVM IR SHALL contain:
    /// - At least 3 conditional branches (outer while cond, inner while cond, if for break)
    /// - Unconditional branches for loop-backs (body → condition for both loops)
    /// This verifies that break targets the correct (innermost) loop's exit block.
    #[test]
    fn prop_nested_while_break_targets_correct_block(
        source in nested_while_break_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "Nested while loop with break failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();

        // Must have at least 3 conditional branches:
        // 1. outer while condition (x < N)
        // 2. inner while condition (y < M)
        // 3. if condition (y > K) guarding break
        let br_i1_count = ir.matches("br i1").count();
        prop_assert!(
            br_i1_count >= 3,
            "Expected at least 3 conditional branches (outer while + inner while + if for break), got {}.\nSource: {}\nIR: {}",
            br_i1_count,
            source,
            ir
        );

        // Must have unconditional branches for loop-backs
        // At minimum: entry→outer_cond, outer_body→outer_cond, inner_body→inner_cond, break→inner_exit
        let br_label_count = ir.matches("br label").count();
        prop_assert!(
            br_label_count >= 4,
            "Expected at least 4 unconditional branches for nested while loops with break, got {}.\nSource: {}\nIR: {}",
            br_label_count,
            source,
            ir
        );
    }

    /// **Validates: Requirements 2.4, 2.5, 2.6**
    ///
    /// Property 4: For any program with nested while loops where the inner loop
    /// contains a `continue` guarded by `if y > K`, the generated LLVM IR SHALL contain:
    /// - At least 3 conditional branches (outer while cond, inner while cond, if for continue)
    /// - Unconditional branches for loop-backs including the continue branch
    /// This verifies that continue targets the correct (innermost) loop's condition block.
    #[test]
    fn prop_nested_while_continue_targets_correct_block(
        source in nested_while_continue_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "Nested while loop with continue failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();

        // Must have at least 3 conditional branches:
        // 1. outer while condition (x < N)
        // 2. inner while condition (y < M)
        // 3. if condition (y > K) guarding continue
        let br_i1_count = ir.matches("br i1").count();
        prop_assert!(
            br_i1_count >= 3,
            "Expected at least 3 conditional branches (outer while + inner while + if for continue), got {}.\nSource: {}\nIR: {}",
            br_i1_count,
            source,
            ir
        );

        // Must have unconditional branches for loop-backs
        // The continue generates an additional unconditional branch to the inner while's condition
        let br_label_count = ir.matches("br label").count();
        prop_assert!(
            br_label_count >= 4,
            "Expected at least 4 unconditional branches for nested while loops with continue, got {}.\nSource: {}\nIR: {}",
            br_label_count,
            source,
            ir
        );
    }

    /// **Validates: Requirements 2.4, 2.5, 2.6**
    ///
    /// Property 4: For any program with a while loop containing an inner `loop`
    /// with a break, the generated LLVM IR SHALL contain:
    /// - At least 2 conditional branches (while cond + if for break)
    /// - Unconditional branches including the break targeting the inner loop's exit
    /// This verifies that break in a `loop` inside a `while` targets the loop's exit,
    /// not the while's exit, using the LoopContext stack correctly.
    #[test]
    fn prop_while_containing_loop_break_targets_inner_loop(
        source in while_loop_break_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "While containing loop with break failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();

        // Must have at least 2 conditional branches:
        // 1. while condition (x < N)
        // 2. if condition (x > K) guarding break inside loop
        let br_i1_count = ir.matches("br i1").count();
        prop_assert!(
            br_i1_count >= 2,
            "Expected at least 2 conditional branches (while cond + if for break), got {}.\nSource: {}\nIR: {}",
            br_i1_count,
            source,
            ir
        );

        // Must have unconditional branches for:
        // - entry into while cond, while body → while cond loop-back
        // - entry into loop body, loop body → loop body loop-back
        // - break → loop exit
        let br_label_count = ir.matches("br label").count();
        prop_assert!(
            br_label_count >= 3,
            "Expected at least 3 unconditional branches for while+loop with break, got {}.\nSource: {}\nIR: {}",
            br_label_count,
            source,
            ir
        );
    }
}

// ============================================================================
// Property 5: Impl block methods produce correctly mangled function definitions
//
// *For any* Sengoo program containing an `impl TypeName { def method_name(...) }`
// block, the generated LLVM IR SHALL contain a function definition named
// `TypeName_method_name` with the correct parameter list including `self` as
// the first parameter.
//
// **Validates: Requirements 3.1, 3.4**
// ============================================================================

/// Strategy to generate valid method names for impl blocks.
/// Generates lowercase alphabetic identifiers of length 3-10 that are not
/// Sengoo keywords.
fn impl_method_name_strategy() -> impl Strategy<Value = String> {
    "[a-z]{3,10}".prop_filter("must not be a Sengoo keyword", |name| {
        // Keep the property generator in sync with the lexer keyword table.
        Keyword::lookup(name).is_none() && !matches!(name.as_str(), "print" | "mut")
    })
}

/// Strategy to generate programs with impl blocks for i64 with random method names.
/// Pattern: `impl i64 { def METHOD(self) -> i64 { self } } def main() -> i64 { let x: i64 = 42; x.METHOD() }`
/// where METHOD is a random valid identifier.
fn impl_method_program_strategy() -> impl Strategy<Value = (String, String)> {
    impl_method_name_strategy().prop_map(|method_name| {
        let source = format!(
            "impl i64 {{ def {}(self) -> i64 {{ self }} }} def main() -> i64 {{ let x: i64 = 42; x.{}() }}",
            method_name, method_name
        );
        (source, method_name)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 3.1, 3.4**
    ///
    /// Property 5: For any program containing `impl i64 { def METHOD(self) -> i64 { self } }`
    /// where METHOD is a valid identifier, the generated LLVM IR SHALL contain a
    /// function definition with the mangled name `i64_METHOD`.
    #[test]
    fn prop_impl_method_produces_correctly_mangled_function(
        (source, method_name) in impl_method_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "impl i64 method program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();

        // The LLVM IR must contain a function definition with the mangled name
        let mangled_name = format!("i64_{}", method_name);
        let define_pattern = format!("define {} @{}", "i64", mangled_name);
        prop_assert!(
            ir.contains(&define_pattern),
            "IR does not contain function definition '{}' for method '{}'.\nSource: {}\nIR: {}",
            define_pattern,
            method_name,
            source,
            ir
        );
    }
}

// ============================================================================
// Property 6: Method calls resolve to correct mangled function with receiver
// as first argument
//
// *For any* Sengoo program containing a method call `receiver.method(args)`
// where the method exists in an impl block, the generated LLVM IR SHALL
// contain a `call` instruction to the correctly mangled function name with
// the receiver value as the first argument followed by the remaining arguments.
//
// **Validates: Requirements 3.2, 3.3, 3.5**
// ============================================================================

/// Strategy to generate programs with impl blocks and method calls on i64.
/// The program defines a method on i64 that takes self and returns self + 1,
/// then calls it via dot syntax. Returns (source, method_name).
fn method_call_program_strategy() -> impl Strategy<Value = (String, String)> {
    impl_method_name_strategy().prop_map(|method_name| {
        let source = format!(
            "impl i64 {{ def {}(self) -> i64 {{ self + 1 }} }} def main() -> i64 {{ let x: i64 = 42; x.{}() }}",
            method_name, method_name
        );
        (source, method_name)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 3.2, 3.3, 3.5**
    ///
    /// Property 6: For any program containing `impl i64 { def METHOD(self) -> i64 { self + 1 } }`
    /// and a call `x.METHOD()`, the generated LLVM IR SHALL contain a `call`
    /// instruction to the mangled function `i64_METHOD` with the receiver as
    /// the first argument.
    #[test]
    fn prop_method_call_resolves_to_correct_mangled_function(
        (source, method_name) in method_call_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "Method call program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();

        // The LLVM IR must contain a call instruction to the mangled function name
        let mangled_name = format!("i64_{}", method_name);
        let call_pattern = format!("call i64 @{}", mangled_name);
        prop_assert!(
            ir.contains(&call_pattern),
            "IR does not contain call instruction '{}' for method call '{}'.\nSource: {}\nIR: {}",
            call_pattern,
            method_name,
            source,
            ir
        );

        // The call must include the receiver (x) as the first argument.
        // The call instruction should have at least one argument (the receiver).
        // We look for the call pattern followed by an opening parenthesis with
        // an i64 argument, e.g. `call i64 @i64_METHOD(i64 %...)`
        let call_with_receiver = format!("call i64 @{}(i64 ", mangled_name);
        prop_assert!(
            ir.contains(&call_with_receiver),
            "Call to '{}' does not pass receiver as first i64 argument.\nSource: {}\nIR: {}",
            mangled_name,
            source,
            ir
        );
    }
}

// ============================================================================
// Property 7: Trait impl methods produce correctly mangled function definitions
//
// *For any* Sengoo program containing an `impl TraitName for TypeName
// { def method_name(...) }` block, the generated LLVM IR SHALL contain a
// function definition named `TypeName_TraitName_method_name`.
//
// **Validates: Requirements 4.2, 4.4**
// ============================================================================

/// Strategy to generate valid trait names.
/// Trait names start with an uppercase letter followed by 2-8 lowercase letters,
/// and must not collide with Sengoo keywords.
fn trait_name_strategy() -> impl Strategy<Value = String> {
    "[A-Z][a-z]{2,8}".prop_filter("must not be a Sengoo keyword", |name| {
        !matches!(
            name.as_str(),
            "Self"
                | "None"
                | "True"
                | "False"
                | "Printable"
                | "Describable"
                | "Showable"
                | "HasDefault"
                | "MultiMethod"
                | "MixedTrait"
        )
    })
}

/// Strategy to generate valid method names for trait impl blocks.
/// Reuses the same filtering logic as `impl_method_name_strategy`.
fn trait_method_name_strategy() -> impl Strategy<Value = String> {
    "[a-z]{3,10}".prop_filter("must not be a Sengoo keyword", |name| {
        Keyword::lookup(name).is_none() && !matches!(name.as_str(), "print" | "mut")
    })
}

/// Strategy to generate programs with trait impl blocks for i64 with random
/// trait names and method names.
/// Pattern:
/// ```text
/// trait TRAIT { def METHOD(self) -> i64 { 0 } }
/// impl TRAIT for i64 { def METHOD(self) -> i64 { self } }
/// def main() -> i64 { let x: i64 = 42; x.METHOD() }
/// ```
/// Returns (source, trait_name, method_name).
fn trait_impl_program_strategy() -> impl Strategy<Value = (String, String, String)> {
    (trait_name_strategy(), trait_method_name_strategy()).prop_map(
        |(trait_name, method_name)| {
            let source = format!(
                "trait {} {{ def {}(self) -> i64 {{ 0 }} }}\nimpl {} for i64 {{ def {}(self) -> i64 {{ self }} }}\ndef main() -> i64 {{ let x: i64 = 42; x.{}() }}",
                trait_name, method_name, trait_name, method_name, method_name
            );
            (source, trait_name, method_name)
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 4.2, 4.4**
    ///
    /// Property 7: For any program containing
    /// `impl TRAIT for i64 { def METHOD(self) -> i64 { self } }`
    /// where TRAIT is a valid trait name and METHOD is a valid method name,
    /// the generated LLVM IR SHALL contain a function definition with the
    /// three-part mangled name `i64_TRAIT_METHOD`.
    #[test]
    fn prop_trait_impl_produces_correctly_mangled_function(
        (source, trait_name, method_name) in trait_impl_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "Trait impl program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();

        // The LLVM IR must contain a function definition with the three-part mangled name
        let mangled_name = format!("i64_{}_{}", trait_name, method_name);
        let define_pattern = format!("define i64 @{}", mangled_name);
        prop_assert!(
            ir.contains(&define_pattern),
            "IR does not contain function definition '{}' for trait impl method.\nSource: {}\nIR: {}",
            define_pattern,
            source,
            ir
        );
    }
}

// ============================================================================
// Property 9: String operations generate correct runtime function calls
//
// *For any* Sengoo program containing string operations (`.len()`, `+`
// concatenation, `==`/`!=` comparison), the generated LLVM IR SHALL contain
// calls to the corresponding runtime functions (`sengoo_str_len`,
// `sengoo_str_concat`, `sengoo_str_eq`).
//
// **Validates: Requirements 5.1, 5.2, 5.3**
// ============================================================================

/// Strategy to generate programs with string `.len()` calls.
/// Pattern: `def main() -> i64 { "RANDOM_STRING".len() }`
/// where RANDOM_STRING is a random ASCII string of letters and digits.
fn str_len_program_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9]{1,20}".prop_map(|s| format!("def main() -> i64 {{ \"{}\".len() }}", s))
}

/// Strategy to generate programs with string `+` concatenation.
/// Pattern: `def main() -> i64 { let s = "A" + "B"; 0 }`
/// where A and B are random ASCII strings of letters and digits.
fn str_concat_program_strategy() -> impl Strategy<Value = String> {
    ("[a-zA-Z0-9]{1,15}", "[a-zA-Z0-9]{1,15}")
        .prop_map(|(a, b)| format!("def main() -> i64 {{ let s = \"{}\" + \"{}\"; 0 }}", a, b))
}

/// Strategy to generate programs with string `==` comparison.
/// Pattern: `def main() -> i64 { if "A" == "B" { 1 } else { 0 } }`
/// where A and B are random ASCII strings of letters and digits.
fn str_eq_program_strategy() -> impl Strategy<Value = String> {
    ("[a-zA-Z0-9]{1,15}", "[a-zA-Z0-9]{1,15}").prop_map(|(a, b)| {
        format!(
            "def main() -> i64 {{ if \"{}\" == \"{}\" {{ 1 }} else {{ 0 }} }}",
            a, b
        )
    })
}

/// Strategy to generate programs with string `!=` comparison.
/// Pattern: `def main() -> i64 { if "A" != "B" { 1 } else { 0 } }`
/// where A and B are random ASCII strings of letters and digits.
fn str_ne_program_strategy() -> impl Strategy<Value = String> {
    ("[a-zA-Z0-9]{1,15}", "[a-zA-Z0-9]{1,15}").prop_map(|(a, b)| {
        format!(
            "def main() -> i64 {{ if \"{}\" != \"{}\" {{ 1 }} else {{ 0 }} }}",
            a, b
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 5.1**
    ///
    /// Property 9: For any program containing `"S".len()` where S is a random
    /// ASCII string, the generated LLVM IR SHALL contain a call to `sengoo_str_len`.
    #[test]
    fn prop_str_len_generates_correct_runtime_call(
        source in str_len_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "String .len() program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();
        prop_assert!(
            ir.contains("@sengoo_str_len"),
            "IR does not contain call to sengoo_str_len.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }

    /// **Validates: Requirements 5.2**
    ///
    /// Property 9: For any program containing `"A" + "B"` where A and B are
    /// random ASCII strings, the generated LLVM IR SHALL contain a call to
    /// `sengoo_str_concat`.
    #[test]
    fn prop_str_concat_generates_correct_runtime_call(
        source in str_concat_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "String concatenation program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();
        prop_assert!(
            ir.contains("@sengoo_str_concat"),
            "IR does not contain call to sengoo_str_concat.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }

    /// **Validates: Requirements 5.3**
    ///
    /// Property 9: For any program containing `"A" == "B"` where A and B are
    /// random ASCII strings, the generated LLVM IR SHALL contain a call to
    /// `sengoo_str_eq`.
    #[test]
    fn prop_str_eq_generates_correct_runtime_call(
        source in str_eq_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "String equality program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();
        prop_assert!(
            ir.contains("@sengoo_str_eq"),
            "IR does not contain call to sengoo_str_eq.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }

    /// **Validates: Requirements 5.3**
    ///
    /// Property 9: For any program containing `"A" != "B"` where A and B are
    /// random ASCII strings, the generated LLVM IR SHALL contain a call to
    /// `sengoo_str_eq` (inequality also uses sengoo_str_eq with result inversion).
    #[test]
    fn prop_str_ne_generates_correct_runtime_call(
        source in str_ne_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "String inequality program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();
        prop_assert!(
            ir.contains("@sengoo_str_eq"),
            "IR does not contain call to sengoo_str_eq for != comparison.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }
}

// ============================================================================
// Property 11: Every MIR basic block has exactly one terminator
//
// *For any* Sengoo program that compiles successfully through MIR lowering,
// every basic block in every generated MIR function SHALL have exactly one
// terminator instruction (Return, Goto, If, Switch, Break, Continue, or
// Unreachable).
//
// **Validates: Requirements 7.2**
// ============================================================================

use crate::compile_to_mir;

/// Strategy to generate a variety of valid Sengoo programs that exercise
/// different control flow patterns, ensuring diverse MIR basic block structures.
fn diverse_valid_program_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple integer return
        (0i64..=999_999).prop_map(|n| {
            format!("def main() -> i64 {{ {} }}", n)
        }),
        // Arithmetic with variables
        (1i64..=1000, 1i64..=1000).prop_map(|(a, b)| {
            format!(
                "def main() -> i64 {{ let x = {}; let y = {}; x + y }}",
                a, b
            )
        }),
        // If-else expression
        (1i64..=100, 1i64..=100, 1i64..=1000, 1i64..=1000).prop_map(|(a, b, t, e)| {
            format!(
                "def main() -> i64 {{ let x = {}; if x > {} {{ {} }} else {{ {} }} }}",
                a, b, t, e
            )
        }),
        // While loop
        (1u32..=50).prop_map(|n| {
            format!(
                "def main() -> i64 {{ let mut x = 0; while x < {} {{ x = x + 1; }} x }}",
                n
            )
        }),
        // While loop with break
        (2u32..=50).prop_flat_map(|n| {
            (Just(n), 1u32..n)
        }).prop_map(|(n, k)| {
            format!(
                "def main() -> i64 {{ let mut x = 0; while x < {} {{ if x > {} {{ break; }} x = x + 1; }} x }}",
                n, k
            )
        }),
        // Nested while loops
        (1u32..=20, 1u32..=20).prop_map(|(n, m)| {
            format!(
                "def main() -> i64 {{ let mut x = 0; let mut total = 0; while x < {} {{ let mut y = 0; while y < {} {{ y = y + 1; total = total + 1; }} x = x + 1; }} total }}",
                n, m
            )
        }),
        // Multiple functions
        (1i64..=100, 1i64..=100).prop_map(|(a, b)| {
            format!(
                "def add(x: i64, y: i64) -> i64 {{ x + y }}\ndef main() -> i64 {{ add({}, {}) }}",
                a, b
            )
        }),
        // Impl method with call
        Just("impl i64 { def double(self) -> i64 { self + self } } def main() -> i64 { let x: i64 = 21; x.double() }".to_string()),
        // Print call
        (0i64..=999).prop_map(|n| {
            format!("def main() -> i64 {{ print({}); 0 }}", n)
        }),
        // String operations
        Just("def main() -> i64 { \"hello\".len() }".to_string()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// **Validates: Requirements 7.2**
    ///
    /// Property 11: For any Sengoo program that compiles successfully through
    /// MIR lowering, every basic block in every generated MIR function SHALL
    /// have exactly one terminator instruction. A basic block with no terminator
    /// (terminator is None) violates the MIR well-formedness invariant.
    #[test]
    fn prop_every_basic_block_has_exactly_one_terminator(
        source in diverse_valid_program_strategy()
    ) {
        let result = compile_to_mir(&source);

        // Only check programs that compile successfully to MIR
        if let Ok(mir_fns) = result {
            for mir_fn in &mir_fns {
                for bb in &mir_fn.basic_blocks {
                    prop_assert!(
                        bb.terminator.is_some(),
                        "Basic block bb{} in function '{}' has no terminator.\n\
                         Source: {}\n\
                         Function has {} basic blocks, {} locals.\n\
                         Block instructions: {:?}",
                        bb.id,
                        mir_fn.name,
                        source,
                        mir_fn.basic_blocks.len(),
                        mir_fn.locals.len(),
                        bb.instructions.len()
                    );
                }
            }
        }
        // If compilation fails, that's fine — we only check successfully compiled programs
    }
}

// ============================================================================
// Property 12: MIR lowering returns structured errors instead of panicking
//
// *For any* Sengoo program containing unsupported HIR node types or type
// mismatches, the MIR lowering phase SHALL return a `Result::Err` with a
// descriptive error message instead of panicking via `unwrap()`, `panic!()`,
// or `unreachable!()`.
//
// **Validates: Requirements 7.1, 7.4**
// ============================================================================

/// Strategy to generate programs with unsupported constructs, type mismatches,
/// and edge cases that stress the MIR lowering phase. These programs are
/// designed to exercise error paths in the compiler without causing panics.
fn unsupported_construct_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Type mismatches in binary operations (Requirement 7.4)
        // Mixing bool and integer in arithmetic
        Just("def main() -> i64 { let x = true; let y = 42; x + y }".to_string()),
        // Mixing float and integer
        Just("def main() -> i64 { let x = 3.14; let y = 42; x + y }".to_string()),
        // Boolean arithmetic
        Just("def main() -> i64 { let x = true; let y = false; x - y }".to_string()),

        // Calling non-existent functions
        (1u32..=100).prop_map(|n| {
            format!("def main() -> i64 {{ nonexistent_func_{0}({0}) }}", n)
        }),

        // Method calls on types without impl blocks
        (1i64..=100).prop_map(|n| {
            format!("def main() -> i64 {{ let x = {}; x.nonexistent_method() }}", n)
        }),

        // Break/continue outside of loops (Requirement 7.1)
        Just("def main() -> i64 { break; 0 }".to_string()),
        Just("def main() -> i64 { continue; 0 }".to_string()),

        // Deeply nested expressions
        (2u32..=8).prop_map(|depth| {
            let mut expr = "1".to_string();
            for _ in 0..depth {
                expr = format!("({} + 1)", expr);
            }
            format!("def main() -> i64 {{ {} }}", expr)
        }),

        // Empty function bodies
        Just("def main() -> i64 { }".to_string()),

        // Multiple return paths with type mismatches
        Just("def main() -> i64 { if true { 42 } else { 0 } }".to_string()),

        // Nested if-else with various types
        (1i64..=50, 1i64..=50).prop_map(|(a, b)| {
            format!(
                "def main() -> i64 {{ if {} > {} {{ {} }} else {{ if {} < 0 {{ 0 }} else {{ {} }} }} }}",
                a, b, a, b, b
            )
        }),

        // While loops with complex conditions
        (1u32..=20).prop_map(|n| {
            format!(
                "def main() -> i64 {{ let mut x = 0; while x < {} {{ x = x + 1; }} x }}",
                n
            )
        }),

        // Loop with break returning a value
        Just("def main() -> i64 { let mut x = 0; loop { if x > 10 { break; } x = x + 1; } x }".to_string()),

        // Struct with method call on unknown method
        Just("def main() -> i64 { let x: i64 = 5; x.unknown_method() }".to_string()),

        // Programs with string operations mixed with integer operations
        Just("def main() -> i64 { let s = \"hello\"; s.len() }".to_string()),

        // Chained operations
        (1i64..=100, 1i64..=100, 1i64..=100).prop_map(|(a, b, c)| {
            format!("def main() -> i64 {{ {} + {} + {} }}", a, b, c)
        }),

        // Programs with multiple functions and cross-calls
        (1i64..=50).prop_map(|n| {
            format!(
                "def helper(x: i64) -> i64 {{ x + 1 }}\ndef main() -> i64 {{ helper({}) }}",
                n
            )
        }),

        // Comparison operations with various types
        Just("def main() -> i64 { if 1 > 2 { 1 } else { 0 } }".to_string()),
        Just("def main() -> i64 { if true { 1 } else { 0 } }".to_string()),

        // Unary operations
        Just("def main() -> i64 { let x = 42; -x }".to_string()),

        // Variable shadowing
        Just("def main() -> i64 { let x = 1; let x = 2; x }".to_string()),
    ]
}

/// Strategy to generate random semi-valid source strings that may trigger
/// edge cases in MIR lowering. These include malformed programs, partial
/// constructs, and random combinations of valid tokens.
fn mir_stress_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Random function definitions with various return types
        prop_oneof![
            (0i64..=1000).prop_map(|n| format!("def main() -> i64 {{ {} }}", n)),
            Just("def main() -> bool { true }".to_string()),
            Just("def main() -> bool { false }".to_string()),
        ],
        // Programs with print of various types
        prop_oneof![
            (0i64..=999).prop_map(|n| format!("def main() -> i64 {{ print({}); 0 }}", n)),
            Just("def main() -> i64 { print(true); 0 }".to_string()),
            Just("def main() -> i64 { print(\"test\"); 0 }".to_string()),
        ],
        // Programs with impl blocks
        Just(
            "impl i64 { def double(self) -> i64 { self + self } }\n\
             def main() -> i64 { let x: i64 = 5; x.double() }"
                .to_string()
        ),
        // Programs with trait impls
        Just(
            "trait Showable { def show(self) -> i64; }\n\
             impl Showable for i64 { def show(self) -> i64 { self } }\n\
             def main() -> i64 { let x: i64 = 42; x.show() }"
                .to_string()
        ),
        // Completely random short strings that may parse partially
        "[a-zA-Z0-9 \\{\\}\\(\\)\\+\\-\\*/=<>;:,]{0,80}",
        // Random combinations of keywords
        prop_oneof![
            Just("def".to_string()),
            Just("def main".to_string()),
            Just("let x = 42".to_string()),
            Just("while true { }".to_string()),
            Just("if true { 1 } else { 0 }".to_string()),
            Just("impl i64 { }".to_string()),
            Just("trait Foo { }".to_string()),
        ],
        // Edge case: very large integer literals
        Just("def main() -> i64 { 9999999999999999 }".to_string()),
        // Edge case: deeply nested blocks
        Just("def main() -> i64 { { { { 42 } } } }".to_string()),
        // Edge case: multiple semicolons
        Just("def main() -> i64 { let x = 1; let y = 2; let z = 3; x + y + z }".to_string()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// **Validates: Requirements 7.1, 7.4**
    ///
    /// Property 12: For any Sengoo program containing unsupported constructs
    /// or type mismatches, the MIR lowering phase SHALL return a `Result::Err`
    /// with a descriptive error message instead of panicking. The full
    /// compilation pipeline (`compile_to_ir`) must never panic regardless of
    /// input.
    #[test]
    fn prop_mir_lowering_returns_errors_instead_of_panicking(
        source in unsupported_construct_strategy()
    ) {
        // Use catch_unwind to detect any panics in the compilation pipeline
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compile_to_ir(&source)
        }));

        prop_assert!(
            result.is_ok(),
            "compile_to_ir PANICKED instead of returning an error!\n\
             Source: {}\n\
             This violates Property 12: MIR lowering must return structured errors.",
            source
        );

        // If it didn't panic, verify the Result is well-formed
        match result.unwrap() {
            Ok(ir) => {
                // Successful compilation is fine — IR should be non-empty
                prop_assert!(
                    !ir.is_empty(),
                    "Successful compilation produced empty IR for source: {}",
                    source
                );
            }
            Err(_) => {
                // Returning an error is the expected behavior for unsupported
                // constructs — this is correct behavior per Property 12
            }
        }
    }

    /// **Validates: Requirements 7.1, 7.4**
    ///
    /// Property 12: For random/semi-valid source strings that stress the MIR
    /// lowering phase, the compilation pipeline must never panic. It must
    /// always return either Ok or Err.
    #[test]
    fn prop_mir_lowering_never_panics_on_stress_inputs(
        source in mir_stress_strategy()
    ) {
        // Use catch_unwind to detect any panics
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compile_to_ir(&source)
        }));

        prop_assert!(
            result.is_ok(),
            "compile_to_ir PANICKED on stress input!\n\
             Source: {}\n\
             This violates Property 12: MIR lowering must return structured errors.",
            source
        );
    }

    /// **Validates: Requirements 7.1, 7.4**
    ///
    /// Property 12: The `compile_to_mir` function (which isolates the MIR
    /// lowering phase) must also never panic. For any input, it returns
    /// Ok with MIR functions or Err with a descriptive error.
    #[test]
    fn prop_compile_to_mir_never_panics(
        source in unsupported_construct_strategy()
    ) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compile_to_mir(&source)
        }));

        prop_assert!(
            result.is_ok(),
            "compile_to_mir PANICKED instead of returning an error!\n\
             Source: {}\n\
             This violates Property 12: MIR lowering must return structured errors.",
            source
        );

        // If it didn't panic, the Result is well-formed
        if let Ok(Ok(mir_fns)) = &result {
            // If MIR lowering succeeded, verify basic structure
            for mir_fn in mir_fns {
                prop_assert!(
                    !mir_fn.name.is_empty(),
                    "MIR function has empty name for source: {}",
                    source
                );
            }
        }
    }
}

// ============================================================================
// Property 14: Codegen local name lookup is O(1)
//
// *For any* MIR function, after building the name cache, every call to
// `local_name_cached(local)` SHALL return the same string as the original
// `local_name(local)` function, and SHALL do so via direct index access
// without iteration.
//
// **Validates: Requirements 8.1, 8.2, 8.9**
// ============================================================================

use crate::codegen::Codegen;
use crate::mir::{Local, LocalKind};

/// Compute the expected local name directly from a Local's kind and id,
/// matching the original `local_name` logic without using the cache.
fn expected_local_name(local: Local) -> String {
    match local.kind {
        LocalKind::Param => format!("%l_{}", local.id),
        LocalKind::Temp => format!("%t_{}", local.id),
        LocalKind::User => format!("%u_{}", local.id),
        LocalKind::Return => format!("%ret_{}", local.id),
    }
}

/// Strategy to generate a variety of valid Sengoo programs that produce MIR
/// functions with different numbers and kinds of locals.
fn name_cache_program_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple function with just a return local
        (0i64..=999).prop_map(|n| { format!("def main() -> i64 {{ {} }}", n) }),
        // Function with user variables (let bindings)
        (1i64..=100, 1i64..=100).prop_map(|(a, b)| {
            format!(
                "def main() -> i64 {{ let x = {}; let y = {}; x + y }}",
                a, b
            )
        }),
        // Function with multiple user variables and temporaries
        (1i64..=50, 1i64..=50, 1i64..=50).prop_map(|(a, b, c)| {
            format!(
                "def main() -> i64 {{ let x = {}; let y = {}; let z = {}; x + y + z }}",
                a, b, c
            )
        }),
        // Function with parameters (helper + main)
        (1i64..=100, 1i64..=100).prop_map(|(a, b)| {
            format!(
                "def add(a: i64, b: i64) -> i64 {{ a + b }} def main() -> i64 {{ add({}, {}) }}",
                a, b
            )
        }),
        // Function with if-else (generates more temporaries)
        (1i64..=100, 1i64..=100).prop_map(|(a, b)| {
            format!(
                "def main() -> i64 {{ let x = {}; if x > {} {{ x }} else {{ 0 }} }}",
                a, b
            )
        }),
        // Function with while loop (generates many locals)
        (1u32..=20).prop_map(|n| {
            format!(
                "def main() -> i64 {{ let mut x = 0; while x < {} {{ x = x + 1; }} x }}",
                n
            )
        }),
        // Function with print (generates call-related locals)
        (0i64..=999).prop_map(|n| { format!("def main() -> i64 {{ print({}); 0 }}", n) }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 8.1, 8.2, 8.9**
    ///
    /// Property 14: For any MIR function produced by compiling a valid Sengoo
    /// program, after calling `build_name_cache()`, the cached name at
    /// `name_cache[local.id]` SHALL match the expected name computed from
    /// the local's kind and id. This verifies that the O(1) cache lookup
    /// returns the same result as the original format!()-based computation.
    #[test]
    fn prop_name_cache_matches_local_name_for_all_locals(
        source in name_cache_program_strategy()
    ) {
        let mir_result = compile_to_mir(&source);
        prop_assert!(
            mir_result.is_ok(),
            "Program failed to compile to MIR:\nSource: {}\nError: {:?}",
            source,
            mir_result.err()
        );
        let mir_fns = mir_result.unwrap();

        for mir_fn in &mir_fns {
            let mut codegen = Codegen::new();
            codegen.build_name_cache(mir_fn);

            // Verify the cache is large enough for all locals
            for (local, _ty) in &mir_fn.locals {
                let local_idx = local.index();
                prop_assert!(
                    local_idx < codegen.name_cache.len(),
                    "name_cache is too small: local.id={} but cache len={} in function '{}'",
                    local.id,
                    codegen.name_cache.len(),
                    mir_fn.name
                );

                // The cached name must match the expected name computed from kind + id
                let cached_name = &codegen.name_cache[local_idx];
                let expected = expected_local_name(*local);
                prop_assert_eq!(
                    cached_name,
                    &expected,
                    "name_cache mismatch for local {:?} in function '{}': cached='{}', expected='{}'",
                    local,
                    mir_fn.name,
                    cached_name,
                    expected
                );

                // Also verify the cached name is non-empty (valid)
                prop_assert!(
                    !cached_name.is_empty(),
                    "name_cache entry is empty for local {:?} in function '{}'",
                    local,
                    mir_fn.name
                );
            }
        }
    }
}

// ============================================================================
// Property 13: Constant folding produces correct results
//
// *For any* MIR function containing a Binary instruction where both operands
// are Assign instructions with integer constant values, after running the
// constant folding optimization pass, the Binary instruction SHALL be replaced
// with an Assign instruction whose value equals the result of applying the
// binary operator to the two constants.
//
// **Validates: Requirements 8.6, 8.7**
// ============================================================================

use crate::mir::opt::{ConstantFolding, MirPass};
use crate::mir::Instruction;

/// Strategy to generate programs with constant arithmetic expressions.
/// Generates `def main() -> i64 { a OP b }` where a and b are random i64
/// values and OP is one of +, -, *.
/// Division is handled separately to avoid division by zero.
/// Returns (source, expected_result, op_name).
fn constant_fold_program_strategy() -> impl Strategy<Value = (String, i64, String)> {
    prop_oneof![
        // Addition: a + b
        (0i64..=100_000, 0i64..=100_000).prop_map(|(a, b)| {
            let source = format!("def main() -> i64 {{ {} + {} }}", a, b);
            (source, a.wrapping_add(b), "Add".to_string())
        }),
        // Subtraction: a - b
        (0i64..=100_000, 0i64..=100_000).prop_map(|(a, b)| {
            let source = format!("def main() -> i64 {{ {} - {} }}", a, b);
            (source, a.wrapping_sub(b), "Sub".to_string())
        }),
        // Multiplication: a * b
        (0i64..=10_000, 0i64..=10_000).prop_map(|(a, b)| {
            let source = format!("def main() -> i64 {{ {} * {} }}", a, b);
            (source, a.wrapping_mul(b), "Mul".to_string())
        }),
        // Division: a / b (b != 0)
        (0i64..=100_000, 1i64..=100_000).prop_map(|(a, b)| {
            let source = format!("def main() -> i64 {{ {} / {} }}", a, b);
            (source, a.wrapping_div(b), "Div".to_string())
        }),
    ]
}

/// Helper: check if an instruction is an Assign with a specific integer constant value.
fn is_assign_with_value(inst: &Instruction, expected: i64) -> bool {
    match inst {
        Instruction::Assign {
            value: crate::mir::MirConstant::Int(v),
            ..
        } => *v == expected,
        _ => false,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 8.6, 8.7**
    ///
    /// Property 13: For any program containing a constant arithmetic expression
    /// `a OP b` where a and b are integer literals and OP is +, -, *, or /,
    /// after running the ConstantFolding pass on the MIR, the Binary instruction
    /// SHALL be replaced with an Assign instruction whose value equals the
    /// correct computed result.
    #[test]
    fn prop_constant_folding_produces_correct_results(
        (source, expected_result, op_name) in constant_fold_program_strategy()
    ) {
        // Step 1: Compile to MIR (before optimization)
        let mir_result = compile_to_mir(&source);
        prop_assert!(
            mir_result.is_ok(),
            "Constant expression program failed to compile to MIR:\nSource: {}\nError: {:?}",
            source,
            mir_result.err()
        );
        let mut mir_fns = mir_result.unwrap();

        // Step 2: Verify the MIR contains a Binary instruction before optimization
        let main_fn = mir_fns.iter().find(|f| f.name == "main");
        prop_assert!(
            main_fn.is_some(),
            "No 'main' function found in MIR for source: {}",
            source
        );
        let main_fn = main_fn.unwrap();
        let has_binary_before = main_fn.basic_blocks.iter().any(|bb| {
            main_fn
                .block_instructions(bb)
                .any(|inst| matches!(inst, Instruction::Binary { .. }))
        });
        prop_assert!(
            has_binary_before,
            "MIR for constant expression does not contain a Binary instruction before optimization.\n\
             Source: {}\nOp: {}",
            source,
            op_name
        );

        // Step 3: Run the ConstantFolding pass
        let pass = ConstantFolding;
        let main_fn_mut = mir_fns.iter_mut().find(|f| f.name == "main").unwrap();
        let changed = pass.run(main_fn_mut);
        prop_assert!(
            changed,
            "ConstantFolding pass did not modify the MIR for constant expression.\n\
             Source: {}\nOp: {}",
            source,
            op_name
        );

        // Step 4: Verify the Binary instruction was replaced with an Assign
        // containing the correct computed result
        let main_fn = mir_fns.iter().find(|f| f.name == "main").unwrap();
        let has_binary_after = main_fn.basic_blocks.iter().any(|bb| {
            main_fn
                .block_instructions(bb)
                .any(|inst| matches!(inst, Instruction::Binary { .. }))
        });
        prop_assert!(
            !has_binary_after,
            "MIR still contains a Binary instruction after constant folding.\n\
             Source: {}\nOp: {}\nExpected folded result: {}",
            source,
            op_name,
            expected_result
        );

        // Verify the folded constant value is present in the MIR
        let has_correct_assign = main_fn.basic_blocks.iter().any(|bb| {
            main_fn
                .block_instructions(bb)
                .any(|inst| is_assign_with_value(inst, expected_result))
        });
        prop_assert!(
            has_correct_assign,
            "MIR does not contain an Assign with the expected folded value {}.\n\
             Source: {}\nOp: {}\nInstructions: {:?}",
            expected_result,
            source,
            op_name,
            main_fn
                .basic_blocks
                .iter()
                .flat_map(|bb| main_fn.block_instructions(bb))
                .collect::<Vec<_>>()
        );
    }
}

// ============================================================================
// Property 2: 递归函数调用类型检查通过
//
// *For any* Sengoo program, when a function body contains a recursive call to
// itself with argument types matching the declared signature, the TypeChecker
// SHALL successfully pass type checking without errors.
//
// **Validates: Requirements 2.1, 2.2, 2.3**
// ============================================================================

/// Strategy to generate a random parameter type: either "i64" or "bool".
fn param_type_strategy() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("i64"), Just("bool"),]
}

/// Generate a default/base-case value for a given type.
fn base_value_for_type(ty: &str) -> &str {
    match ty {
        "i64" => "0",
        "bool" => "true",
        _ => "0",
    }
}

/// Generate a recursive-call argument expression for a given type and param name.
/// For i64 params, we pass `param - 1` or just `param` to ensure type correctness.
/// For bool params, we pass `param` directly (bool has no natural decrement).
fn recursive_arg_for_type(ty: &str, param_name: &str) -> String {
    match ty {
        "i64" => format!("{} - 1", param_name),
        "bool" => param_name.to_string(),
        _ => param_name.to_string(),
    }
}

/// Strategy to generate valid recursive Sengoo functions with 1-3 parameters
/// of types i64 and bool. The function contains a base case and a recursive call
/// with correctly-typed arguments.
///
/// Generated pattern:
/// ```
/// def func_name(p0: T0, p1: T1, ...) -> ReturnType {
///     if <base_condition> {
///         <base_value>
///     } else {
///         func_name(<recursive_args>)
///     }
/// }
/// def main() -> i64 { func_name(<initial_args>); 0 }
/// ```
fn recursive_function_strategy() -> impl Strategy<Value = String> {
    // Generate 0-2 additional parameter types (first param is always i64 for base case)
    prop::collection::vec(param_type_strategy(), 0..=2).prop_flat_map(|extra_types| {
        // First param is always i64 to ensure a valid base condition
        let mut param_types = vec!["i64"];
        param_types.extend(extra_types.iter().copied());
        let param_types_clone = param_types.clone();

        // Generate a function name suffix to add variety
        (0..100u32).prop_map(move |name_idx| {
            let func_name = format!("rec_fn_{}", name_idx);
            let param_types = &param_types_clone;
            // Build parameter list: p0: T0, p1: T1, ...
            let params: Vec<String> = param_types
                .iter()
                .enumerate()
                .map(|(i, ty)| format!("p{}: {}", i, ty))
                .collect();
            let params_str = params.join(", ");

            // Return type is always i64 (first param type)
            let ret_type = "i64";

            // Base condition: p0 is always i64, so use a comparison
            let base_cond = "p0 < 1".to_string();

            // Base case value
            let base_val = base_value_for_type(ret_type);

            // Recursive call arguments
            let rec_args: Vec<String> = param_types
                .iter()
                .enumerate()
                .map(|(i, ty)| recursive_arg_for_type(ty, &format!("p{}", i)))
                .collect();
            let rec_args_str = rec_args.join(", ");

            // Initial call arguments from main
            let init_args: Vec<String> = param_types
                .iter()
                .map(|ty| match *ty {
                    "i64" => "5".to_string(),
                    "bool" => "true".to_string(),
                    _ => "0".to_string(),
                })
                .collect();
            let init_args_str = init_args.join(", ");

            // main calls the recursive function and returns 0
            let main_call = format!("let _r = {}({});\n    0", func_name, init_args_str);

            format!(
                "def {}({}) -> {} {{\n    if {} {{ {} }} else {{ {}({}) }}\n}}\ndef main() -> i64 {{\n    {}\n}}",
                func_name, params_str, ret_type,
                base_cond, base_val,
                func_name, rec_args_str,
                main_call
            )
        })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 2.1, 2.2, 2.3**
    ///
    /// Property 2: For any valid recursive function with correctly-typed arguments
    /// in the recursive call, `compile_to_ir` SHALL succeed without errors.
    /// This validates that the TypeChecker pre-registers function signatures
    /// before checking the function body, enabling recursive calls.
    #[test]
    fn prop_recursive_function_type_check_passes(
        source in recursive_function_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "Valid recursive function failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();
        prop_assert!(
            !ir.is_empty(),
            "Compiled IR is empty for recursive function:\nSource: {}",
            source
        );
        // The IR should contain the recursive function definition
        prop_assert!(
            ir.contains("define") && ir.contains("@main"),
            "Compiled IR missing expected function definitions.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }
}

// ============================================================================
// Property 4: 数组元素赋值生成有效 LLVM IR
//
// *For any* Sengoo program containing `arr[index] = value` array element
// assignment, the compilation pipeline SHALL produce LLVM IR containing
// `getelementptr` and `store` instructions, and SHALL not produce any
// compilation errors.
//
// **Validates: Requirements 3.1, 3.2, 3.3**
// ============================================================================

/// Strategy to generate valid Sengoo programs with array element assignment.
/// Generates programs that:
/// - Declare an array with random size (2-5 elements)
/// - Assign a random i64 value to a random valid index (0 to size-1)
///
/// Pattern:
/// ```
/// def main() -> i64 {
///     let arr = [0, 0, ...];   // array of `size` zeros
///     arr[INDEX] = VALUE;
///     arr[INDEX]
/// }
/// ```
fn array_assign_program_strategy() -> impl Strategy<Value = String> {
    (2usize..=5)
        .prop_flat_map(|size| {
            // index must be valid: 0..size
            (Just(size), 0..size, 0i64..=999_999)
        })
        .prop_map(|(size, index, value)| {
            // Build array literal with `size` zeros
            let elems: Vec<String> = (0..size).map(|_| "0".to_string()).collect();
            let arr_literal = elems.join(", ");
            format!(
                "def main() -> i64 {{\n    let mut arr = [{}];\n    arr[{}] = {};\n    arr[{}]\n}}",
                arr_literal, index, value, index
            )
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 3.1, 3.2, 3.3**
    ///
    /// Property 4: For any program containing `arr[index] = value` where arr is
    /// a valid array, index is within bounds, and value is an i64, the generated
    /// LLVM IR SHALL contain `getelementptr` (from IndexAddr codegen) and `store`
    /// (from Store codegen) instructions.
    #[test]
    fn prop_array_assign_generates_valid_llvm_ir(
        source in array_assign_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "Array assignment program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();

        // The IR must contain getelementptr for the IndexAddr instruction
        prop_assert!(
            ir.contains("getelementptr"),
            "IR does not contain 'getelementptr' for array element address computation.\nSource: {}\nIR: {}",
            source,
            ir
        );

        // The IR must contain store for the Store instruction
        prop_assert!(
            ir.contains("store"),
            "IR does not contain 'store' for array element assignment.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }
}

// ============================================================================
// Property 1: 条件上下文中比较运算符优先于结构体字面量
//
// *For any* Sengoo program where an `if`/`while` condition contains
// `<identifier> <comparison_op> <identifier> {`, the Parser SHALL parse the
// comparison as a binary expression and treat `{` as the start of the block
// body, not as a struct literal. `compile_to_ir` SHALL succeed.
//
// **Validates: Requirements 1.1, 1.2**
// ============================================================================

/// Strategy to generate valid Sengoo programs with if conditions containing
/// comparison operators. The generated programs declare two i64 variables with
/// random values and use a random comparison operator in the if condition.
///
/// Pattern:
/// ```
/// def main() -> i64 {
///     let VAR_LEFT = LEFT_VAL;
///     let VAR_RIGHT = RIGHT_VAL;
///     if VAR_LEFT OP VAR_RIGHT {
///         THEN_VAL
///     } else {
///         ELSE_VAL
///     }
/// }
/// ```
fn if_comparison_program_strategy() -> impl Strategy<Value = String> {
    let var_names = prop_oneof![
        Just("a"),
        Just("b"),
        Just("c"),
        Just("x"),
        Just("y"),
        Just("z"),
    ];
    let comp_ops = prop_oneof![
        Just(">"),
        Just("<"),
        Just(">="),
        Just("<="),
        Just("=="),
        Just("!="),
    ];
    (
        var_names.clone(),
        var_names,
        comp_ops,
        0i64..=999,
        0i64..=999,
        0i64..=999,
        0i64..=999,
    )
        .prop_filter("variable names must differ", |(l, r, _, _, _, _, _)| l != r)
        .prop_map(|(left_var, right_var, op, left_val, right_val, then_val, else_val)| {
            format!(
                "def main() -> i64 {{\n    let {} = {};\n    let {} = {};\n    if {} {} {} {{\n        {}\n    }} else {{\n        {}\n    }}\n}}",
                left_var, left_val,
                right_var, right_val,
                left_var, op, right_var,
                then_val,
                else_val,
            )
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 1.1, 1.2**
    ///
    /// Feature: sengoo-compiler-optimization, Property 1: 条件上下文中比较运算符优先于结构体字面量
    ///
    /// For any program with `if VAR_LEFT OP VAR_RIGHT { ... }` where OP is a
    /// comparison operator (>, <, >=, <=, ==, !=), `compile_to_ir` SHALL succeed,
    /// confirming the parser treats `{` as the block body start rather than a
    /// struct literal.
    #[test]
    fn prop_comparison_in_if_condition_parses_correctly(
        source in if_comparison_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "Program with comparison in if condition failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();
        prop_assert!(
            !ir.is_empty(),
            "Compiled IR is empty for source: {}",
            source
        );
        // The IR should contain a comparison instruction (icmp) proving the
        // parser produced a binary comparison node, not a struct literal
        prop_assert!(
            ir.contains("icmp"),
            "IR does not contain 'icmp' comparison instruction, suggesting the parser \
             may not have parsed the condition as a binary comparison.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }
}

// ============================================================================
// Property 5: 结构体定义、构造和字段访问生成有效 LLVM IR
//
// *For any* Sengoo program containing struct definition, construction, and
// field access, the generated LLVM IR SHALL contain correct named struct type
// declaration, `insertvalue` construction instructions, and `extractvalue`
// access instructions.
//
// **Validates: Requirements 4.1, 4.2, 4.3, 4.4**
// ============================================================================

/// Strategy to generate valid Sengoo programs with struct definition, construction,
/// and field access. Generates programs that:
/// - Define a struct with 2-4 fields (using x, y, z, w as field names, all i64)
/// - Construct the struct with random i64 values
/// - Access a field (e.g., p.x) and return it
///
/// IMPORTANT: The MIR lowering uses hardcoded field name-to-index mapping.
/// Only these field names work: x (index 0), y (index 1), z (index 2), w (index 3).
fn struct_codegen_program_strategy() -> impl Strategy<Value = String> {
    // Choose number of fields: 2, 3, or 4
    (2usize..=4)
        .prop_flat_map(|num_fields| {
            let field_names = &["x", "y", "z", "w"];
            let fields_used: Vec<&str> = field_names[..num_fields].to_vec();

            // Generate random i64 values for each field
            let values = proptest::collection::vec(0i64..=999, num_fields..=num_fields);

            // Choose which field to access (index into fields_used)
            let access_idx = 0..num_fields;

            (Just(fields_used), values, access_idx)
        })
        .prop_map(|(fields, values, access_idx)| {
            // Build struct definition: struct MyStruct { x: i64, y: i64, ... }
            let field_defs: Vec<String> =
                fields.iter().map(|name| format!("{}: i64", name)).collect();
            let struct_def = format!("struct MyStruct {{ {} }}", field_defs.join(", "));

            // Build struct construction: MyStruct { x: val0, y: val1, ... }
            let field_inits: Vec<String> = fields
                .iter()
                .zip(values.iter())
                .map(|(name, val)| format!("{}: {}", name, val))
                .collect();
            let struct_init = format!("MyStruct {{ {} }}", field_inits.join(", "));

            // Access a field
            let accessed_field = fields[access_idx];

            format!(
                "{}\ndef main() -> i64 {{\n    let p = {};\n    p.{}\n}}",
                struct_def, struct_init, accessed_field
            )
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 4.1, 4.2, 4.3, 4.4**
    ///
    /// Feature: sengoo-compiler-optimization, Property 5: 结构体定义、构造和字段访问生成有效 LLVM IR
    ///
    /// For any program containing a struct definition with 2-4 i64 fields,
    /// struct construction, and field access, `compile_to_ir` SHALL succeed
    /// and the generated LLVM IR SHALL contain:
    /// - `insertvalue` instructions for struct construction
    /// - `extractvalue` instruction for field access
    #[test]
    fn prop_struct_codegen_generates_valid_llvm_ir(
        source in struct_codegen_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "Struct program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();

        // The IR must contain insertvalue for struct construction (Requirement 4.2)
        prop_assert!(
            ir.contains("insertvalue"),
            "IR does not contain 'insertvalue' for struct construction.\nSource: {}\nIR: {}",
            source,
            ir
        );

        // The IR must contain extractvalue for struct field access (Requirement 4.3)
        prop_assert!(
            ir.contains("extractvalue"),
            "IR does not contain 'extractvalue' for struct field access.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }
}

// ============================================================================
// Property 6: if/else 表达式生成 Phi 指令
//
// *For any* Sengoo program where `if/else` is used as an expression
// (e.g., `let x = if cond { a } else { b }`), the generated LLVM IR SHALL
// contain a `phi` instruction whose incoming values correspond to the then
// and else branch results.
//
// **Validates: Requirements 5.1, 5.2**
// ============================================================================

/// Strategy to generate valid Sengoo programs that use if/else as an expression
/// assigned to a variable. The generated programs declare two i64 variables with
/// random values, use a random comparison operator to form the condition, and
/// assign the if/else result to a variable which is then returned.
///
/// Pattern:
/// ```
/// def main() -> i64 {
///     let a = LEFT_VAL;
///     let b = RIGHT_VAL;
///     let result = if a OP b { THEN_VAL } else { ELSE_VAL };
///     result
/// }
/// ```
fn if_else_expr_phi_program_strategy() -> impl Strategy<Value = String> {
    let comp_ops = prop_oneof![
        Just(">"),
        Just("<"),
        Just(">="),
        Just("<="),
        Just("=="),
        Just("!="),
    ];
    (
        comp_ops,
        0i64..=999,
        0i64..=999,
        0i64..=999,
        0i64..=999,
    )
        .prop_map(|(op, left_val, right_val, then_val, else_val)| {
            format!(
                "def main() -> i64 {{\n    let a = {};\n    let b = {};\n    let result = if a {} b {{ {} }} else {{ {} }};\n    result\n}}",
                left_val,
                right_val,
                op,
                then_val,
                else_val,
            )
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 5.1, 5.2**
    ///
    /// Feature: sengoo-compiler-optimization, Property 6: if/else 表达式生成 Phi 指令
    ///
    /// For any program with `let result = if a OP b { THEN } else { ELSE }` where
    /// OP is a comparison operator and THEN/ELSE are i64 literals, `compile_to_ir`
    /// SHALL succeed and the generated LLVM IR SHALL contain a `phi` instruction,
    /// confirming the MIR lowering produced a Phi node and the codegen emitted it.
    #[test]
    fn prop_if_else_expr_generates_phi_instruction(
        source in if_else_expr_phi_program_strategy()
    ) {
        let result = compile_to_ir(&source);
        prop_assert!(
            result.is_ok(),
            "if/else expression program failed to compile:\nSource: {}\nError: {:?}",
            source,
            result.err()
        );
        let ir = result.unwrap();

        // The IR must contain a phi instruction (Requirement 5.2)
        prop_assert!(
            ir.contains("phi"),
            "IR does not contain 'phi' instruction for if/else expression.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }
}
