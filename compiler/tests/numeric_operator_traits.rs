use sengoo_compiler::{compile_to_ir, Parser, TypeChecker};

#[test]
fn user_defined_arithmetic_operators_dispatch_to_trait_impls() {
    let source = r#"
trait Add<Rhs, Output> { def add(self, rhs: Rhs) -> Output {} }
trait Sub<Rhs, Output> { def sub(self, rhs: Rhs) -> Output {} }
trait Mul<Rhs, Output> { def mul(self, rhs: Rhs) -> Output {} }
trait Div<Rhs, Output> { def div(self, rhs: Rhs) -> Output {} }
trait Rem<Rhs, Output> { def rem(self, rhs: Rhs) -> Output {} }
trait Neg<Output> { def neg(self) -> Output {} }

struct Scalar { value: i64 }

impl Add<Scalar, Scalar> for Scalar {
    def add(self, rhs: Scalar) -> Scalar { Scalar { value: self.value + rhs.value } }
}

impl Sub<Scalar, Scalar> for Scalar {
    def sub(self, rhs: Scalar) -> Scalar { Scalar { value: self.value - rhs.value } }
}
impl Mul<Scalar, Scalar> for Scalar {
    def mul(self, rhs: Scalar) -> Scalar { Scalar { value: self.value * rhs.value } }
}
impl Div<Scalar, Scalar> for Scalar {
    def div(self, rhs: Scalar) -> Scalar { Scalar { value: self.value / rhs.value } }
}
impl Rem<Scalar, Scalar> for Scalar {
    def rem(self, rhs: Scalar) -> Scalar { Scalar { value: self.value % rhs.value } }
}
impl Neg<Scalar> for Scalar {
    def neg(self) -> Scalar { Scalar { value: -self.value } }
}

def main() -> i64 {
    let a = Scalar { value: 20 };
    let b = Scalar { value: 6 };
    let added = a + b;
    let subbed = added - Scalar { value: 4 };
    let multiplied = subbed * Scalar { value: 2 };
    let divided = multiplied / Scalar { value: 4 };
    let remainder = divided % Scalar { value: 3 };
    let negated = -remainder;
    negated.value
}
"#;

    let ir = compile_to_ir(source).expect("operator traits should lower to static impl calls");
    for expected in [
        "@Scalar_Add_Scalar_Scalar_add",
        "@Scalar_Sub_Scalar_Scalar_sub",
        "@Scalar_Mul_Scalar_Scalar_mul",
        "@Scalar_Div_Scalar_Scalar_div",
        "@Scalar_Rem_Scalar_Scalar_rem",
        "@Scalar_Neg_Scalar_neg",
    ] {
        assert!(ir.contains(expected), "missing {expected} in IR:\n{ir}");
    }
}

#[test]
fn operator_output_and_generic_bound_are_preserved_through_dispatch() {
    let source = r#"
trait Add<Rhs, Output> { def add(self, rhs: Rhs) -> Output {} }
struct Scalar { value: i64 }
impl Add<Scalar, Scalar> for Scalar {
    def add(self, rhs: Scalar) -> Scalar { Scalar { value: self.value + rhs.value } }
}
struct Measure { value: i64 }
impl Add<Measure, i64> for Measure {
    def add(self, rhs: Measure) -> i64 { self.value + rhs.value }
}
def add_generic<T>(left: T, right: T) -> T where T: Add<T, T> {
    left + right
}
def main() -> i64 {
    let summed = add_generic(Scalar { value: 20 }, Scalar { value: 22 });
    let measured = Measure { value: 9 } + Measure { value: 4 };
    let primitive = add_generic(20, 22);
    summed.value + measured + primitive
}
"#;

    let ir = compile_to_ir(source).expect("generic bounds and non-Self Output should dispatch");
    assert!(ir.contains("@Scalar_Add_Scalar_Scalar_add"), "{ir}");
    assert!(ir.contains("@Measure_Add_Measure_i64_add"), "{ir}");
    assert!(ir.contains("; Function: add_generic_Scalar"), "{ir}");
    assert!(ir.contains("; Function: add_generic_i64"), "{ir}");
    assert!(!ir.contains("i64_Add_i64_i64_add"), "{ir}");
}

#[test]
fn primitive_arithmetic_keeps_intrinsic_fast_path() {
    let ir = compile_to_ir(
        "def combine(left: i64, right: i64) -> i64 { (left + right) * right }\n\
         def main() -> i64 { combine(20, 2) }",
    )
    .expect("primitive arithmetic should compile");
    assert!(ir.contains("add i64") && ir.contains("mul i64"), "{ir}");
    assert!(!ir.contains("i64_Add") && !ir.contains("i64_Mul"), "{ir}");
}

#[test]
fn generic_operator_call_requires_the_exact_rhs_and_output_bound() {
    let source = r#"
trait Add<Rhs, Output> { def add(self, rhs: Rhs) -> Output {} }
struct Measure { value: i64 }
impl Add<Measure, i64> for Measure {
    def add(self, rhs: Measure) -> i64 { self.value + rhs.value }
}
def add_generic<T>(left: T, right: T) -> T where T: Add<T, T> {
    left + right
}
def main() -> Measure {
    add_generic(Measure { value: 1 }, Measure { value: 2 })
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let error = checker
        .check_program(&program)
        .expect_err("Add<Measure, i64> must not satisfy Add<Measure, Measure>")
        .to_string();
    assert!(
        error.contains("[unsatisfied-trait-bound]") && error.contains("Add<Measure,Measure>"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn arithmetic_operator_without_matching_impl_has_stable_diagnostic() {
    let source = r#"
trait Add<Rhs, Output> { def add(self, rhs: Rhs) -> Output {} }
struct Scalar { value: i64 }
def main() -> Scalar {
    Scalar { value: 1 } + Scalar { value: 2 }
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let error = checker
        .check_program(&program)
        .expect_err("missing Add impl should be rejected")
        .to_string();
    assert!(
        error.contains("[operator-trait-missing]")
            && error.contains("Add")
            && error.contains("Scalar"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn arithmetic_operator_with_multiple_outputs_has_stable_ambiguity_diagnostic() {
    let source = r#"
trait Add<Rhs, Output> { def add(self, rhs: Rhs) -> Output {} }
struct Scalar { value: i64 }
impl Add<Scalar, Scalar> for Scalar {
    def add(self, rhs: Scalar) -> Scalar { self }
}
impl Add<Scalar, i64> for Scalar {
    def add(self, rhs: Scalar) -> i64 { self.value + rhs.value }
}
def main() -> Scalar {
    Scalar { value: 1 } + Scalar { value: 2 }
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let error = checker
        .check_program(&program)
        .expect_err("ambiguous Add output should be rejected")
        .to_string();
    assert!(
        error.contains("[operator-trait-ambiguous]")
            && error.contains("Add")
            && error.contains("Scalar"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn operator_impl_return_must_match_declared_output() {
    let source = r#"
trait Add<Rhs, Output> { def add(self, rhs: Rhs) -> Output {} }
struct Scalar { value: i64 }
impl Add<Scalar, Scalar> for Scalar {
    def add(self, rhs: Scalar) -> i64 { self.value + rhs.value }
}

def main() -> i64 { 0 }
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let error = checker
        .check_program(&program)
        .expect_err("operator Output mismatch should be rejected")
        .to_string();
    assert!(
        error.contains("[operator-trait-output-mismatch]")
            && error.contains("Scalar")
            && error.contains("i64"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn operator_trait_declaration_must_return_its_output_parameter() {
    let source = r#"
trait Add<Rhs, Output> { def add(self, rhs: Rhs) -> i64 {} }
def main() -> i64 { 0 }
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let error = checker
        .check_program(&program)
        .expect_err("malformed operator trait contract should be rejected")
        .to_string();
    assert!(
        error.contains("[operator-trait-contract]") && error.contains("Output"),
        "unexpected diagnostic: {error}"
    );
}
