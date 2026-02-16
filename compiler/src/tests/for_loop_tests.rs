//! Unit tests for `for` loop MIR lowering and codegen.
//!
//! _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

use std::collections::HashSet;

use crate::compile_to_ir;

/// `for x in [..]` should compile to IR with:
/// - index comparison (`icmp slt`)
/// - element address calculation (`getelementptr`)
/// - print call in loop body
/// - unique `%load.*` temporary names
#[test]
fn test_for_loop_generates_expected_ir_and_unique_load_names() {
    let source = r#"
def main() -> i64 {
    for x in [1, 2, 3] {
        print(x);
    }
    0
}
"#;

    let ir = compile_to_ir(source).expect("for loop should compile successfully");

    assert!(
        ir.contains("icmp slt"),
        "Expected for-loop condition compare (icmp slt), got:\n{}",
        ir
    );
    assert!(
        ir.contains("getelementptr"),
        "Expected element address calculation (getelementptr), got:\n{}",
        ir
    );
    assert!(
        ir.contains("@sengoo_print_i64("),
        "Expected print call for loop variable, got:\n{}",
        ir
    );

    let mut seen = HashSet::new();
    let mut load_defs = 0usize;
    for line in ir.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("%load.") {
            continue;
        }
        if let Some((lhs, _)) = trimmed.split_once(" = ") {
            load_defs += 1;
            assert!(
                seen.insert(lhs.to_string()),
                "Duplicate SSA temp `{}` found in IR:\n{}",
                lhs,
                ir
            );
        }
    }
    assert!(load_defs > 0, "Expected at least one `%load.*` definition");
}

/// Nested `for` loops should compile and generate multiple compare/branch blocks.
#[test]
fn test_nested_for_loops_compile() {
    let source = r#"
def main() -> i64 {
    let sum = 0;
    for x in [1, 2, 3] {
        for y in [4, 5] {
            sum = sum + x + y;
        }
    }
    sum
}
"#;

    let ir = compile_to_ir(source).expect("nested for loops should compile successfully");

    let cond_count = ir.matches("icmp slt").count();
    assert!(
        cond_count >= 2,
        "Expected at least two loop condition comparisons for nested for loops, got {}, IR:\n{}",
        cond_count,
        ir
    );
}

/// `break` should exit the for-loop and `continue` should jump to increment block.
#[test]
fn test_for_loop_break_and_continue_compile() {
    let source = r#"
def main() -> i64 {
    let acc = 0;
    for x in [1, 2, 3, 4, 5] {
        if x > 3 {
            break;
        }
        if x == 2 {
            continue;
        }
        acc = acc + x;
    }
    acc
}
"#;

    let ir =
        compile_to_ir(source).expect("for loop with break/continue should compile successfully");

    assert!(
        ir.contains("br i1"),
        "Expected conditional branches in for loop with break/continue, got:\n{}",
        ir
    );
    let br_count = ir.matches("br label").count();
    assert!(
        br_count >= 3,
        "Expected multiple unconditional branches for loop control flow, got {}, IR:\n{}",
        br_count,
        ir
    );
}
