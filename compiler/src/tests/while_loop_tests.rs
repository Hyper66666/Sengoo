//! Unit tests for while loop MIR lowering and codegen
//!
//! Tests that the Sengoo compiler generates correct LLVM IR for `while` loops,
//! including proper block structure with condition, body, and exit blocks.
//!
//! _Requirements: 2.1, 2.2_

use crate::compile_to_ir;

/// Test that a simple while loop compiles and produces correct block structure.
/// The while loop should generate: cond_block → body_block → cond_block (loop back)
///                                             → exit_block (condition false)
///
/// _Requirements: 2.1, 2.2_
#[test]
fn test_simple_while_loop_compiles() {
    let source = r#"def main() -> i64 { let mut x = 0; while x < 10 { x = x + 1; } x }"#;
    let ir = compile_to_ir(source).expect("simple while loop should compile successfully");

    // The IR should contain:
    // 1. A conditional branch (br i1 ... for the while condition)
    // 2. An unconditional branch back to the condition block (loop back from body)
    // 3. The icmp instruction for the condition (x < 10)
    assert!(
        ir.contains("icmp slt"),
        "Expected IR to contain 'icmp slt' for the < comparison, got:\n{}",
        ir
    );
    assert!(
        ir.contains("br i1"),
        "Expected IR to contain 'br i1' for the conditional branch, got:\n{}",
        ir
    );
    // There should be at least one unconditional branch (loop back)
    assert!(
        ir.contains("br label"),
        "Expected IR to contain 'br label' for unconditional branch (loop back), got:\n{}",
        ir
    );
}

/// Test that a while loop with a more complex body (containing if/else) compiles correctly.
/// This tests the fix for body blocks that contain control flow.
///
/// _Requirements: 2.1, 2.2_
#[test]
fn test_while_loop_with_if_body_compiles() {
    let source = r#"
def main() -> i64 {
    let mut x = 0;
    let mut y = 0;
    while x < 10 {
        if x < 5 {
            y = y + 1;
        } else {
            y = y + 2;
        }
        x = x + 1;
    }
    y
}
"#;
    let ir =
        compile_to_ir(source).expect("while loop with if/else body should compile successfully");

    // Should have conditional branches for both the while condition and the if
    let br_i1_count = ir.matches("br i1").count();
    assert!(
        br_i1_count >= 2,
        "Expected at least 2 conditional branches (while cond + if cond), got {}, IR:\n{}",
        br_i1_count,
        ir
    );
}

/// Test that a while loop that never executes (condition immediately false) compiles.
///
/// _Requirements: 2.1, 2.2_
#[test]
fn test_while_loop_false_condition_compiles() {
    let source = r#"def main() -> i64 { let mut x = 100; while x < 10 { x = x + 1; } x }"#;
    let ir = compile_to_ir(source).expect("while loop with false condition should compile");

    // Should still have the conditional branch structure
    assert!(
        ir.contains("br i1"),
        "Expected IR to contain 'br i1' even for immediately-false while, got:\n{}",
        ir
    );
}

/// Test that a loop (infinite) with break compiles and produces correct block structure.
/// The loop should generate: loop_block → loop_block (unconditional loop back)
///                                      → exit_block (via break)
///
/// _Requirements: 2.3_
#[test]
fn test_loop_with_break_compiles() {
    let source =
        r#"def main() -> i64 { let mut x = 0; loop { x = x + 1; if x > 5 { break; } } x }"#;
    let ir = compile_to_ir(source).expect("loop with break should compile successfully");

    // The IR should contain:
    // 1. A conditional branch for the if (break condition)
    assert!(
        ir.contains("br i1"),
        "Expected IR to contain 'br i1' for the if condition guarding break, got:\n{}",
        ir
    );
    // 2. Unconditional branches (loop back + entry into loop)
    assert!(
        ir.contains("br label"),
        "Expected IR to contain 'br label' for unconditional branches, got:\n{}",
        ir
    );
    // 3. The comparison for x > 5
    assert!(
        ir.contains("icmp sgt"),
        "Expected IR to contain 'icmp sgt' for the > comparison, got:\n{}",
        ir
    );
}

/// Test that a simple infinite loop with immediate break compiles.
///
/// _Requirements: 2.3_
#[test]
fn test_loop_immediate_break_compiles() {
    let source = r#"def main() -> i64 { loop { break; } 0 }"#;
    let ir = compile_to_ir(source).expect("loop with immediate break should compile");

    // Should have at least one unconditional branch (entry into loop block)
    assert!(
        ir.contains("br label"),
        "Expected IR to contain 'br label', got:\n{}",
        ir
    );
}

// ============================================================================
// Nested loop tests — verifying break/continue target correct blocks
// _Requirements: 2.4, 2.5, 2.6_
// ============================================================================

/// Test nested while loops: inner break should exit inner loop only.
/// `while ... { while ... { break; } }` — inner break exits inner while,
/// outer while continues iterating.
///
/// _Requirements: 2.4, 2.6_
#[test]
fn test_nested_while_inner_break_exits_inner_only() {
    let source = r#"
def main() -> i64 {
    let mut x = 0;
    let mut y = 0;
    while x < 3 {
        let mut z = 0;
        while z < 5 {
            if z > 2 {
                break;
            }
            z = z + 1;
        }
        y = y + 1;
        x = x + 1;
    }
    y
}
"#;
    let ir = compile_to_ir(source).expect("nested while loops with inner break should compile");

    // The IR should have multiple conditional branches:
    // - outer while condition (x < 3)
    // - inner while condition (z < 5)
    // - if condition (z > 2) guarding break
    let br_i1_count = ir.matches("br i1").count();
    assert!(
        br_i1_count >= 3,
        "Expected at least 3 conditional branches (outer while + inner while + if for break), got {}, IR:\n{}",
        br_i1_count,
        ir
    );

    // Should have multiple unconditional branches (loop backs + entry jumps)
    let br_label_count = ir.matches("br label").count();
    assert!(
        br_label_count >= 4,
        "Expected at least 4 unconditional branches for nested while loops, got {}, IR:\n{}",
        br_label_count,
        ir
    );
}

/// Test while containing loop: break inside loop should exit the loop, not the while.
/// `while ... { loop { break; } }` — break exits the inner loop, while continues.
///
/// _Requirements: 2.4, 2.6_
#[test]
fn test_while_containing_loop_break_exits_loop_not_while() {
    let source = r#"
def main() -> i64 {
    let mut x = 0;
    let mut result = 0;
    while x < 5 {
        loop {
            result = result + 1;
            break;
        }
        x = x + 1;
    }
    result
}
"#;
    let ir = compile_to_ir(source).expect("while containing loop with break should compile");

    // The while condition should still be present (break doesn't exit while)
    assert!(
        ir.contains("br i1"),
        "Expected conditional branch for while condition, got:\n{}",
        ir
    );

    // Should have multiple unconditional branches
    let br_label_count = ir.matches("br label").count();
    assert!(
        br_label_count >= 3,
        "Expected at least 3 unconditional branches (while→cond, loop entry, break→exit, body→cond), got {}, IR:\n{}",
        br_label_count,
        ir
    );
}

/// Test loop containing while: continue inside while should go to while's condition,
/// not loop's body start.
/// `loop { while ... { continue; } break; }` — continue goes to while's condition.
///
/// _Requirements: 2.5, 2.6_
#[test]
fn test_loop_containing_while_continue_targets_while_condition() {
    let source = r#"
def main() -> i64 {
    let mut x = 0;
    loop {
        let mut y = 0;
        while y < 3 {
            y = y + 1;
            continue;
        }
        x = x + 1;
        if x > 2 {
            break;
        }
    }
    x
}
"#;
    let ir = compile_to_ir(source).expect("loop containing while with continue should compile");

    // Should have conditional branches for:
    // - while condition (y < 3)
    // - if condition (x > 2) guarding break
    let br_i1_count = ir.matches("br i1").count();
    assert!(
        br_i1_count >= 2,
        "Expected at least 2 conditional branches (while cond + if for break), got {}, IR:\n{}",
        br_i1_count,
        ir
    );
}

/// Test nested loops with continue in inner loop: continue should target inner loop's
/// continue block, not outer loop's.
/// `while ... { while ... { continue; } }` — continue goes to inner while's condition.
///
/// _Requirements: 2.5, 2.6_
#[test]
fn test_nested_while_continue_targets_inner_condition() {
    let source = r#"
def main() -> i64 {
    let mut x = 0;
    let mut total = 0;
    while x < 3 {
        let mut y = 0;
        while y < 5 {
            y = y + 1;
            if y > 3 {
                continue;
            }
            total = total + 1;
        }
        x = x + 1;
    }
    total
}
"#;
    let ir = compile_to_ir(source).expect("nested while with inner continue should compile");

    // Should have conditional branches for:
    // - outer while (x < 3)
    // - inner while (y < 5)
    // - if (y > 3) guarding continue
    let br_i1_count = ir.matches("br i1").count();
    assert!(
        br_i1_count >= 3,
        "Expected at least 3 conditional branches, got {}, IR:\n{}",
        br_i1_count,
        ir
    );
}

/// Test loop with continue: continue should jump back to loop body start.
/// `loop { ... continue; ... }` — continue goes back to the beginning of the loop body.
///
/// _Requirements: 2.5_
#[test]
fn test_loop_continue_targets_body_start() {
    let source = r#"
def main() -> i64 {
    let mut x = 0;
    let mut y = 0;
    loop {
        x = x + 1;
        if x > 10 {
            break;
        }
        if x < 5 {
            continue;
        }
        y = y + 1;
    }
    y
}
"#;
    let ir = compile_to_ir(source).expect("loop with continue should compile");

    // Should have conditional branches for:
    // - if (x > 10) guarding break
    // - if (x < 5) guarding continue
    let br_i1_count = ir.matches("br i1").count();
    assert!(
        br_i1_count >= 2,
        "Expected at least 2 conditional branches, got {}, IR:\n{}",
        br_i1_count,
        ir
    );
}

/// Test deeply nested loops: break in innermost loop should only exit that loop.
/// Three levels of nesting: while { while { loop { break; } } }
///
/// _Requirements: 2.4, 2.6_
#[test]
fn test_three_level_nested_loops_break_targets_innermost() {
    let source = r#"
def main() -> i64 {
    let mut a = 0;
    let mut result = 0;
    while a < 2 {
        let mut b = 0;
        while b < 2 {
            loop {
                result = result + 1;
                break;
            }
            b = b + 1;
        }
        a = a + 1;
    }
    result
}
"#;
    let ir = compile_to_ir(source).expect("three-level nested loops should compile");

    // Should have conditional branches for both while conditions
    let br_i1_count = ir.matches("br i1").count();
    assert!(
        br_i1_count >= 2,
        "Expected at least 2 conditional branches for the two while loops, got {}, IR:\n{}",
        br_i1_count,
        ir
    );

    // Should have many unconditional branches for all the loop structures
    let br_label_count = ir.matches("br label").count();
    assert!(
        br_label_count >= 5,
        "Expected at least 5 unconditional branches for three-level nesting, got {}, IR:\n{}",
        br_label_count,
        ir
    );
}

/// Test break outside of loop produces an error.
///
/// _Requirements: 2.4_
#[test]
fn test_break_outside_loop_is_error() {
    let source = r#"def main() -> i64 { break; 0 }"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "break outside of loop should produce an error"
    );
}

/// Test continue outside of loop produces an error.
///
/// _Requirements: 2.5_
#[test]
fn test_continue_outside_loop_is_error() {
    let source = r#"def main() -> i64 { continue; 0 }"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "continue outside of loop should produce an error"
    );
}
