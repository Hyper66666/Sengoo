//! Regression tests for streaming LLVM IR emission parity.

use crate::{compile_to_mir, Codegen};

fn codegen_legacy_and_stream(source: &str) -> (String, String) {
    let mir_fns = compile_to_mir(source).expect("source should compile to MIR");

    let mut legacy_codegen = Codegen::new();
    let legacy_ir = legacy_codegen
        .codegen(&mir_fns)
        .expect("legacy codegen should succeed");

    let mut stream_codegen = Codegen::new();
    let mut stream_buf = Vec::new();
    stream_codegen
        .codegen_to_writer(&mir_fns, &mut stream_buf)
        .expect("stream codegen should succeed");
    let stream_ir = String::from_utf8(stream_buf).expect("stream IR should be valid utf-8");

    (legacy_ir, stream_ir)
}

#[test]
fn stream_codegen_matches_legacy_for_simple_program() {
    let source = r#"
def main() -> i64 {
    let x = 10;
    let y = 32;
    x + y
}
"#;
    let (legacy, stream) = codegen_legacy_and_stream(source);
    assert_eq!(legacy, stream);
}

#[test]
fn stream_codegen_matches_legacy_for_control_flow_and_calls() {
    let source = r#"
def abs(x: i64) -> i64 {
    if x < 0 { 0 - x } else { x }
}

def sum_to(n: i64) -> i64 {
    let i = 0;
    let acc = 0;
    while i < n {
        acc = acc + i;
        i = i + 1;
    }
    acc
}

def main() -> i64 {
    abs(sum_to(16) - 42)
}
"#;
    let (legacy, stream) = codegen_legacy_and_stream(source);
    assert_eq!(legacy, stream);
}

#[test]
fn stream_codegen_matches_legacy_for_struct_and_method_style_code() {
    let source = r#"
struct Point { x: i64, y: i64 }

def norm2(p: Point) -> i64 {
    p.x * p.x + p.y * p.y
}

def main() -> i64 {
    let p = Point { x: 3, y: 4 };
    norm2(p)
}
"#;
    let (legacy, stream) = codegen_legacy_and_stream(source);
    assert_eq!(legacy, stream);
}
