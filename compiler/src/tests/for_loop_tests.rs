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
    let mut sum = 0;
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
    let mut acc = 0;
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

fn compile_with_collections(program: &str) -> String {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_root = manifest_dir
        .parent()
        .unwrap_or(manifest_dir)
        .join("tools")
        .join("stdlib");
    let modules = [
        "option.sg",
        "result.sg",
        "ffi.sg",
        "string.sg",
        "collections.sg",
    ];
    let prelude = modules
        .iter()
        .map(|module| {
            std::fs::read_to_string(stdlib_root.join(module))
                .unwrap_or_else(|err| panic!("failed to read {module}: {err}"))
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    compile_to_ir(&format!("{prelude}\n\n{program}"))
        .unwrap_or_else(|err| panic!("collection for-loop program should compile: {err}"))
}

#[test]
fn array_for_loop_does_not_lower_through_iterator_next() {
    let ir = compile_to_ir(
        r#"
def main() -> i64 {
    let mut total = 0;
    for value in [1, 2, 3] {
        total = total + value;
    }
    total
}
"#,
    )
    .expect("array for-loop should compile");
    assert!(
        ir.contains("icmp slt") && ir.contains("getelementptr"),
        "array for-loop must keep direct lowering, got:\n{ir}"
    );
    assert!(
        !ir.contains("_next") && !ir.contains("Iterator_next"),
        "array for-loop must not introduce iterator next calls, got:\n{ir}"
    );
}

#[test]
fn for_loop_iterates_vec_and_lazy_adapters() {
    let ir = compile_with_collections(
        r#"
def main() -> i64 {
    let values: Vec<i64> = vec_new();
    values.push(1);
    values.push(2);
    values.push(3);
    let mut total = 0;
    for value in values {
        total = total + value;
    }
    let mapped = values.into_iter().map(|item| item + 1).take(2);
    for value in mapped {
        total = total + value;
    }
    let deque: VecDeque<i64> = vecdeque_new();
    deque.push_back(4);
    for value in deque {
        total = total + *value;
    }
    let set: HashSet<i64> = hashset_new();
    set.insert(5);
    for value in set {
        total = total + value;
    }
    let ordered: BTreeSet<i64> = btreeset_new();
    ordered.insert(6);
    for value in ordered {
        total = total + value;
    }
    total
}
"#,
    );
    assert!(
        ir.contains("RawVecIter")
            || ir.contains("VecIter")
            || ir.contains("_next")
            || ir.contains("iter"),
        "expected iterator-protocol lowering for collection for-loops:\n{ir}"
    );
}

#[test]
fn for_loop_over_vec_matches_none_and_some() {
    let ir = compile_with_collections(
        r#"
def main() -> i64 {
    let values: Vec<i64> = vec_new();
    values.push(1);
    values.push(2);
    values.push(3);
    let mut total = 0;
    for value in values {
        total = total + value;
    }
    total
}
"#,
    );
    let Some(next_at) = ir.find("Iterator_next").or_else(|| ir.find("_next(")) else {
        panic!("expected iterator next call in for-over-vec IR:\n{ir}");
    };
    let switch_region = ir[next_at..]
        .split("\ndefine ")
        .next()
        .unwrap_or(&ir[next_at..]);
    assert!(
        switch_region.contains("i64 0, label") && switch_region.contains("i64 1, label"),
        "for-over-vec must match both Option::None and Option::Some:\n{switch_region}"
    );
}

#[test]
fn for_loop_iterates_map_entries_keys_and_values() {
    let ir = compile_with_collections(
        r#"
def main() -> i64 {
    let mut map: HashMap<i64, i64> = hashmap_new();
    map.insert(1, 10);
    map.insert(2, 20);
    let mut total = 0;
    for entry in map {
        total = total + entry.key + entry.value;
    }
    for key in map.keys() {
        total = total + *key;
    }
    for value in map.values() {
        total = total + value;
    }
    total
}
"#,
    );
    assert!(
        ir.contains("entries") || ir.contains("MapEntryIter") || ir.contains("keys"),
        "expected map entry/key/value for-loop lowering:\n{ir}"
    );
}

#[test]
fn for_loop_rejects_mutation_while_iterating_a_vec() {
    let source = {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let stdlib_root = manifest_dir
            .parent()
            .unwrap_or(manifest_dir)
            .join("tools")
            .join("stdlib");
        let modules = [
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "collections.sg",
        ];
        let prelude = modules
            .iter()
            .map(|module| {
                std::fs::read_to_string(stdlib_root.join(module))
                    .unwrap_or_else(|err| panic!("failed to read {module}: {err}"))
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        format!(
            "{prelude}\n\n{}",
            r#"
def main() -> i64 {
    let values: Vec<i64> = vec_new();
    values.push(1);
    for value in values {
        values.push(value);
    }
    0
}
"#
        )
    };
    let error = compile_to_ir(&source).expect_err("mutating a vec while iterating must fail");
    let message = error.to_string();
    assert!(
        message.contains("borrow") || message.contains("cannot move borrowed"),
        "expected borrow diagnostic, got: {message}"
    );
}
