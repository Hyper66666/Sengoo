//! Tests for `&dyn Trait` dynamic dispatch codegen (G3).
//!
//! A `&Concrete -> &dyn Trait` coercion builds a `{ data, vtable }` fat pointer;
//! a method call on a `&dyn Trait` receiver loads the function pointer from the
//! trait's vtable slot and issues an indirect call. Each `(trait, concrete)`
//! pair emits a vtable global plus a by-pointer dispatch shim that forwards to
//! the monomorphic implementation.

use crate::compile_to_ir;

const DYN_PROGRAM: &str = r#"
trait Shape {
    def area(&self) -> i64 {
        0
    }
}

struct Square {
    side: i64,
}

impl Shape for Square {
    def area(&self) -> i64 {
        self.side * self.side
    }
}

def describe(shape: &dyn Shape) -> i64 {
    shape.area()
}

def main() -> i64 {
    let sq = Square { side: 5 };
    describe(&sq)
}
"#;

fn compile(program: &str) -> String {
    compile_to_ir(program).unwrap_or_else(|err| panic!("dyn program should compile: {err:?}"))
}

#[test]
fn emits_fat_pointer_struct_type() {
    let ir = compile(DYN_PROGRAM);
    assert!(
        ir.contains("%__dyn_Shape = type { i8*, i8* }"),
        "expected the dyn fat-pointer struct type, got:\n{ir}"
    );
}

#[test]
fn emits_vtable_global_for_concrete_pair() {
    let ir = compile(DYN_PROGRAM);
    assert!(
        ir.contains("@__vtable$Shape$Square = internal constant [1 x i64]"),
        "expected a vtable global for (Shape, Square), got:\n{ir}"
    );
    assert!(
        ir.contains("@__dynshim$Shape$Square$0$area to i64"),
        "expected the vtable slot to reference the dispatch shim, got:\n{ir}"
    );
}

#[test]
fn emits_dispatch_shim_forwarding_to_impl() {
    let ir = compile(DYN_PROGRAM);
    assert!(
        ir.contains("define i64 @__dynshim$Shape$Square$0$area(i8* "),
        "expected a by-pointer dispatch shim, got:\n{ir}"
    );
    assert!(
        ir.contains("call i64 @Square_Shape_area("),
        "expected the shim to forward to the monomorphic impl, got:\n{ir}"
    );
}

#[test]
fn coercion_builds_fat_pointer_at_call_site() {
    let ir = compile(DYN_PROGRAM);
    // The vtable address is bitcast into the fat pointer's second field.
    assert!(
        ir.contains("bitcast [1 x i64]* @__vtable$Shape$Square to i8*"),
        "expected the vtable address to be loaded for the fat pointer, got:\n{ir}"
    );
    assert!(
        ir.contains("insertvalue %__dyn_Shape"),
        "expected the fat pointer to be constructed via insertvalue, got:\n{ir}"
    );
}

#[test]
fn dyn_receiver_dispatches_through_vtable() {
    let ir = compile(DYN_PROGRAM);
    // describe() extracts the vtable, loads the slot, and calls indirectly.
    assert!(
        ir.contains("extractvalue %__dyn_Shape"),
        "expected the receiver fat pointer to be decomposed, got:\n{ir}"
    );
    assert!(
        ir.contains("inttoptr i64") && ir.contains("to i64 (i8*)*"),
        "expected an indirect call through a materialized function pointer, got:\n{ir}"
    );
}
