//! Tests for `&dyn Trait` dynamic dispatch codegen (G3).
//!
//! A `&Concrete -> &dyn Trait` coercion builds a `{ data, vtable }` fat pointer;
//! a method call on a `&dyn Trait` receiver loads the function pointer from the
//! trait's vtable slot and issues an indirect call. Each `(trait, concrete)`
//! pair emits a vtable global plus a by-pointer dispatch shim that forwards to
//! the monomorphic implementation.

use crate::codegen::JITCodegen;
use crate::{compile_to_ir, compile_to_mir, Parser, TypeChecker};

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

const DYN_DROP_PROGRAM: &str = r#"
trait Shape {
    def area(&self) -> i64 {
        0
    }
}

struct Square {
    side: i64,
}

impl Drop for Square {
    def drop(&mut self) {
    }
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

const DYN_MUT_PROGRAM: &str = r#"
trait CounterTrait {
    def inc(&mut self) -> i64 {
        0
    }
}

struct Counter {
    value: i64,
}

impl CounterTrait for Counter {
    def inc(&mut self) -> i64 {
        self.value = self.value + 1;
        self.value
    }
}

def bump(counter: &mut dyn CounterTrait) -> i64 {
    counter.inc()
}

def main() -> i64 {
    let mut counter = Counter { value: 41 };
    bump(&mut counter)
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
        ir.contains("@__vtable$Shape$Square = internal constant [4 x i64]"),
        "expected a vtable global for (Shape, Square), got:\n{ir}"
    );
    assert!(
        ir.contains("@__dynshim$Shape$Square$0$__drop_s8_a8 to i64")
            && ir.contains("i64 8, i64 8")
            && ir.contains("@__dynshim$Shape$Square$3$area to i64"),
        "expected the vtable slot to reference the dispatch shim, got:\n{ir}"
    );
}

#[test]
fn emits_dispatch_shim_forwarding_to_impl() {
    let ir = compile(DYN_PROGRAM);
    assert!(
        ir.contains("define i64 @__dynshim$Shape$Square$3$area(i8* "),
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
        ir.contains("bitcast [4 x i64]* @__vtable$Shape$Square to i8*"),
        "expected the vtable address to be loaded for the fat pointer, got:\n{ir}"
    );
    assert!(
        ir.contains("insertvalue %__dyn_Shape"),
        "expected the fat pointer to be constructed via insertvalue, got:\n{ir}"
    );
}

#[test]
fn dyn_vtable_emits_noop_drop_thunk_for_non_drop_type() {
    let ir = compile(DYN_PROGRAM);
    assert!(
        ir.contains("define void @__dynshim$Shape$Square$0$__drop_s8_a8(i8* "),
        "expected an erased drop thunk in the vtable, got:\n{ir}"
    );
    assert!(
        !ir.contains("@Square_Drop_drop"),
        "non-Drop concrete dyn value should not call a concrete Drop impl, got:\n{ir}"
    );
}

#[test]
fn dyn_vtable_drop_thunk_calls_concrete_drop_impl() {
    let ir = compile(DYN_DROP_PROGRAM);
    assert!(
        ir.contains("define void @__dynshim$Shape$Square$0$__drop_s8_a8(i8* ")
            && ir.contains("call void @Square_Drop_drop("),
        "expected erased dyn drop thunk to call concrete Drop impl, got:\n{ir}"
    );
}

const OWNED_DYN_DROP_PROGRAM: &str = r#"
trait Speak {
    def speak(&self) -> i64 {
        0
    }
}

struct Guard {
    id: i64,
}

impl Drop for Guard {
    def drop(&mut self) {
    }
}

impl Speak for Guard {
    def speak(&self) -> i64 {
        self.id
    }
}

def main() -> i64 {
    let g = Guard { id: 7 };
    let s: dyn Speak = g;
    s.speak()
}
"#;

const OWNED_DYN_EARLY_DROP_PROGRAM: &str = r#"
trait Speak {
    def speak(&self) -> i64 {
        0
    }
}

struct Guard {
    id: i64,
}

impl Drop for Guard {
    def drop(&mut self) {
    }
}

impl Speak for Guard {
    def speak(&self) -> i64 {
        self.id
    }
}

def main() -> i64 {
    let g = Guard { id: 7 };
    let s: dyn Speak = g;
    s.drop();
    0
}
"#;

#[test]
fn owned_dyn_value_drops_through_vtable_helper_at_scope_exit() {
    let ir = compile(OWNED_DYN_DROP_PROGRAM);
    assert!(
        ir.contains("define void @__dyn_Speak_Drop_drop(%__dyn_Speak"),
        "expected the per-trait owned dyn drop helper, got:\n{ir}"
    );
    assert!(
        ir.contains("call void @__dyn_Speak_Drop_drop("),
        "expected main to drop the owned dyn value via the helper, got:\n{ir}"
    );
    assert!(
        ir.contains("@__vtable$Speak$Guard = internal constant [4 x i64]")
            && ir.contains("call void @Guard_Drop_drop("),
        "expected the vtable drop thunk to reach the concrete Drop impl, got:\n{ir}"
    );
}

#[test]
fn owned_dyn_drop_helper_guards_null_drop_slot() {
    let ir = compile(OWNED_DYN_DROP_PROGRAM);
    // The helper loads slot 0, compares against zero, and only calls through
    // the function pointer when a drop thunk is present.
    assert!(
        ir.contains("to void (i8*)*"),
        "expected an indirect void call through the drop slot pointer, got:\n{ir}"
    );
}

#[test]
fn owned_dyn_explicit_early_drop_lowered_via_helper() {
    let ir = compile(OWNED_DYN_EARLY_DROP_PROGRAM);
    let helper_calls = ir.matches("call void @__dyn_Speak_Drop_drop(").count();
    assert_eq!(
        helper_calls, 1,
        "explicit s.drop() should drop exactly once (no scope-exit double drop), got:\n{ir}"
    );
}

#[test]
fn jit_codegen_lowers_owned_dyn_drop() {
    let mir = compile_to_mir(OWNED_DYN_DROP_PROGRAM).expect("owned dyn program should lower");
    let mut jit = JITCodegen::new();
    let ir = jit
        .generate(&mir)
        .expect("JIT codegen should support owned dyn drop MIR");
    assert!(
        ir.contains("@__dyn_Speak_Drop_drop"),
        "JIT IR should contain the owned dyn drop helper, got:\n{ir}"
    );
    assert!(
        !ir.contains("; unhandled instruction"),
        "JIT IR should lower every owned dyn drop instruction, got:\n{ir}"
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

#[test]
fn dyn_mut_receiver_dispatches_through_vtable() {
    let ir = compile(DYN_MUT_PROGRAM);
    assert!(
        ir.contains("@__vtable$CounterTrait$Counter = internal constant [4 x i64]")
            && ir.contains("define i64 @__dynshim$CounterTrait$Counter$3$inc(i8* ")
            && ir.contains("call i64 @Counter_CounterTrait_inc(")
            && !ir.contains("%Counter**"),
        "expected &mut dyn receiver to dispatch through the vtable, got:\n{ir}"
    );
}

#[test]
fn jit_codegen_lowers_dyn_dispatch_call_indirect() {
    let mir = compile_to_mir(DYN_PROGRAM).expect("dyn program should lower to MIR");
    let mut jit = JITCodegen::new();
    let ir = jit
        .generate(&mir)
        .expect("JIT codegen should support dyn dispatch MIR");

    assert!(
        ir.contains("@__vtable$Shape$Square = internal constant"),
        "JIT IR should emit the dyn vtable global, got:\n{ir}"
    );
    assert!(
        ir.contains("inttoptr i64") && ir.contains("to i64 (i8*)*"),
        "JIT IR should materialize and call the vtable function pointer, got:\n{ir}"
    );
    assert!(
        !ir.contains("; unhandled instruction"),
        "JIT IR should lower every dyn dispatch instruction, got:\n{ir}"
    );
}

#[test]
fn dyn_multi_trait_reports_stable_diagnostic() {
    let source = r#"
trait Read {}
trait Write {}

def stream(x: dyn Read + Write) -> i64 {
    0
}
"#;
    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("multi-trait dyn should be rejected");
    let crate::error::CompileError::TypeckError(typeck) = err else {
        panic!("expected TypeckError, got {err:?}");
    };
    assert_eq!(typeck.stable_code(), Some("dyn-multi-trait-unsupported"));
}

#[test]
fn box_dyn_trait_reports_stable_diagnostic() {
    let source = r#"
trait Show {}

struct Box<T> {
    value: T,
}

def takes(x: Box<dyn Show>) -> i64 {
    0
}
"#;
    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("Box<dyn Trait> should be rejected");
    let crate::error::CompileError::TypeckError(typeck) = err else {
        panic!("expected TypeckError, got {err:?}");
    };
    assert_eq!(typeck.stable_code(), Some("dyn-box-unsupported"));
}
