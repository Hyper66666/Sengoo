//! Type checking and MIR lowering for `?` and `try {}`.

use crate::compile_to_ir;
use crate::typeck::typeck;
use crate::Parser;

#[test]
fn conditional_borrowed_tail_keeps_anonymous_owner_until_consumer() {
    let ir = compile_to_ir(
        r#"
struct String { handle: i64 }
impl Drop for String { def drop(&mut self) { print(self.handle); } }
extern "C" { fn view(handle: i64) -> &str; fn inspect(text: &str) -> i64; }
impl String { def as_str(&self) -> &str { view(self.handle) } }
def make() -> String { String { handle: 1 } }
def direct(flag: bool) -> i64 { inspect(if flag { make().as_str() } else { "literal" }) }
def nested(flag: bool) -> i64 { inspect({ if flag { "literal" } else { { make().as_str() } } }) }
def main() -> i64 { 0 }
"#,
    )
    .expect("borrowed expression consumed before anonymous owner cleanup");
    for name in ["direct", "nested"] {
        let marker = format!("; Function: {name}\n");
        let body = ir
            .split(&marker)
            .nth(1)
            .unwrap()
            .split("\n}\n")
            .next()
            .unwrap();
        assert!(
            body.find("@inspect(").unwrap() < body.find("@String_Drop_drop(").unwrap(),
            "anonymous owner must survive the branch join and consumer: {body}"
        );
        assert_eq!(
            body.matches("@String_Drop_drop(").count(),
            1,
            "one conditional cleanup: {body}"
        );
    }
}

#[test]
fn conditional_temporary_borrow_cannot_escape_function() {
    let error = compile_to_ir(
        r#"
struct String { handle: i64 }
extern "C" { fn view(handle: i64) -> &str; }
impl String { def as_str(&self) -> &str { view(self.handle) } }
def make() -> String { String { handle: 1 } }
def escape(flag: bool) -> &str { if flag { make().as_str() } else { "literal" } }
def main() -> i64 { 0 }
"#,
    )
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("borrow-escapes-owner"),
        "escaping temporary must remain rejected: {error}"
    );
}

#[test]
fn fresh_explicit_reference_arguments_keep_their_owner_through_call_and_failure() {
    let ir = compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
struct Owner { marker: i64 }
impl Drop for Owner { def drop(&mut self) { print(self.marker); } }
struct Inspector { marker: i64 }
impl Inspector { def observe(&self, value: &Owner) -> i64 { value.marker } }
def make() -> Owner { Owner { marker: 42 } }
def inspect(value: &Owner, next: i64) -> i64 { value.marker + next }
def fail() -> Result<i64, i64> { Result { is_ok: false, value: 0, error: 9 } }
def direct() -> i64 { inspect(&make(), 0) }
def method() -> i64 { let viewer = Inspector { marker: 1 }; viewer.observe(&make()) }
def named() -> i64 { let owner = make(); inspect(&owner, 0) }
def early() -> Result<i64, i64> {
    let value = inspect(&make(), fail()?);
    Result { is_ok: true, value: value, error: 0 }
}
def main() -> i64 { 0 }
"#,
    )
    .expect("fresh explicitly borrowed owner should compile");
    for (name, expected, call) in [
        ("direct", 1, "@inspect("),
        ("method", 1, "@Inspector_observe("),
        ("named", 1, "@inspect("),
        ("early", 2, "@inspect("),
    ] {
        let marker = format!("; Function: {name}\n");
        let body = ir
            .split(&marker)
            .nth(1)
            .unwrap()
            .split("\n}\n")
            .next()
            .unwrap();
        assert_eq!(
            body.matches("@Owner_Drop_drop(").count(),
            expected,
            "cleanup paths for {name}: {body}"
        );
        if name != "early" {
            assert!(
                body.find(call).unwrap() < body.find("@Owner_Drop_drop(").unwrap(),
                "borrowed use must precede cleanup for {name}: {body}"
            );
        }
    }
}

#[test]
fn reference_alias_escape_cannot_be_inferred_as_preserving_a_string_owner() {
    let source = r#"
struct String { handle: i64 }
extern "C" { fn unknown_handle(handle: i64) -> i64; }
def escape(value: String) -> i64 { let alias = &value.handle; unknown_handle(*alias) }
def main() -> i64 { let text = String { handle: 7 }; let value = escape(text); print(text.handle); value }
"#;
    let error = compile_to_ir(source).unwrap_err().to_string();
    assert!(
        error.contains("use-after-move"),
        "reference alias cannot hide an escaping identity: {error}"
    );
}

#[test]
fn checked_native_projection_contract_cannot_survive_extern_identity_change() {
    let source = "extern \"C\" { fn sengoo_ffi_buffer_copy_out(handle: i64, output: i64, capacity: i64) -> i64; }\ndef main() -> i64 { 0 }";
    let parsed = Parser::parse(source).unwrap();
    let mut checker = crate::TypeChecker::new();
    checker.check_program(&parsed).unwrap();
    let env = checker.into_env();
    let mut hir = crate::lower_ast(&parsed, &env);
    for item in &mut hir.items {
        if let crate::hir::HIRItem::ExternBlock(block) = item {
            for item in &mut block.items {
                if let crate::hir::HIRExternItem::Function(function) = item {
                    function.name = "unknown_retaining_sink".to_string();
                }
            }
        }
    }
    let error = crate::lower_hir_with_options(
        &hir.items,
        crate::MirLowerOptions::default().with_checked_parameter_borrows(&env),
    )
    .unwrap_err();
    assert!(
        error.contains("ownership::parameter_contract"),
        "unexpected native contract mismatch: {error}"
    );
}

#[test]
fn raw_projection_is_borrowed_only_inside_a_proven_noescape_sink() {
    let source = r#"
struct String { handle: i64 }
impl Drop for String { def drop(&mut self) { print(self.handle); } }
extern "C" { fn sengoo_ffi_buffer_copy_out(handle: i64, output: i64, capacity: i64) -> i64; }
def project(value: String) -> i64 { value.handle }
def observed(value: String) -> i64 { sengoo_ffi_buffer_copy_out(0, project(value), 0) }
def main() -> i64 { let text = String { handle: 7 }; let count = observed(text); print(text.handle); count }
"#;
    let parsed = Parser::parse(source).unwrap();
    let mut checker = crate::TypeChecker::new();
    checker.check_program(&parsed).unwrap();
    assert_eq!(checker.env().borrowed_legacy_params["project"], vec![false]);
    assert_eq!(checker.env().projected_legacy_params["project"], vec![true]);
    assert_eq!(checker.env().borrowed_legacy_params["observed"], vec![true]);
    let ir = compile_to_ir(source).unwrap();
    let body = ir
        .split("; Function: main\n")
        .nth(1)
        .unwrap()
        .split("\n}\n")
        .next()
        .unwrap();
    assert_eq!(body.matches("@String_Drop_drop(").count(), 1);
    let escaped = source.replace("let count = observed(text)", "let count = project(text)");
    assert!(compile_to_ir(&escaped)
        .unwrap_err()
        .to_string()
        .contains("use-after-move"));
    let unknown = source
        .replace(
            "let count = observed(text)",
            "let count = unknown(project(text))",
        )
        .replace(
            "def main()",
            "extern \"C\" { fn unknown(handle: i64) -> i64; }\ndef main()",
        );
    assert!(compile_to_ir(&unknown)
        .unwrap_err()
        .to_string()
        .contains("use-after-move"));
}

#[test]
fn native_observation_requires_exact_extern_identity_and_signature() {
    for native in [
        "def sengoo_string_eq(left: i64, right: i64) -> i64 { left }",
        "extern \"C\" { fn sengoo_string_eq(left: i32, right: i64) -> i64; }",
    ] {
        let source = format!("struct String {{ handle: i64 }}\n{native}\ndef observed(value: String) -> i64 {{ sengoo_string_eq(value.handle, 0) }}\ndef main() -> i64 {{ 0 }}");
        let parsed = Parser::parse(&source).unwrap();
        let mut checker = crate::TypeChecker::new();
        if native.contains("i32") {
            // Build declaration HIR without type-checking a deliberately
            // incompatible call: it must not inherit the i64 runtime contract.
            let hir = crate::lower_ast(&parsed, checker.env());
            assert!(!crate::mir::infer_legacy_parameter_borrows(&hir.items)["observed"][0]);
        } else {
            checker.check_program(&parsed).unwrap();
            assert!(!checker.env().borrowed_legacy_params["observed"][0]);
        }
    }
}

#[test]
fn ambiguous_function_identity_never_infers_parameter_borrows() {
    let program = Parser::parse("struct String { handle: i64 }\ndef observe(value: String) -> bool { value.handle == 1 }\ndef main() -> i64 { 0 }").unwrap();
    let mut checker = crate::TypeChecker::new();
    checker.check_program(&program).unwrap();
    let mut hir = crate::lower_ast(&program, checker.env());
    let duplicate = hir.items.iter().find(|item| matches!(item, crate::hir::HIRItem::Function(function) if function.name == "observe")).unwrap().clone();
    hir.items.push(duplicate);
    let summary = crate::mir::infer_legacy_parameter_borrows(&hir.items);
    assert_eq!(summary.get("observe"), Some(&vec![false]));
}

#[test]
fn same_named_methods_use_their_receiver_identity() {
    let ir = compile_to_ir(r#"
struct String { handle: i64 }
impl Drop for String { def drop(&mut self) { print(self.handle); } }
struct First { marker: i64 }
struct Second { marker: i64 }
impl First { def observe(&self, value: String) -> bool { value.handle == 7 } }
impl Second { def observe(&self, value: String) -> String { value } }
def borrowed() -> bool { let receiver = First { marker: 1 }; let text = String { handle: 7 }; receiver.observe(text) }
def transferred() -> String { let receiver = Second { marker: 2 }; let text = String { handle: 7 }; receiver.observe(text) }
def main() -> i64 { 0 }
"#).unwrap();
    for (name, expected, target) in [
        ("borrowed", 1, "First_observe"),
        ("transferred", 0, "Second_observe"),
    ] {
        let marker = format!("; Function: {name}\n");
        let body = ir
            .split(&marker)
            .nth(1)
            .unwrap()
            .split("\n}\n")
            .next()
            .unwrap();
        assert!(body.contains(&format!("@{target}(")));
        assert_eq!(
            body.matches("@String_Drop_drop(").count(),
            expected,
            "wrong receiver summary: {body}"
        );
    }
}

#[test]
fn checked_parameter_borrow_cannot_be_silently_tightened_in_mir() {
    let source = r#"
struct String { handle: i64 }
def observe(value: String) -> bool { value.handle == 1 }
def main() -> i64 { 0 }
"#;
    let program = Parser::parse(source).unwrap();
    let mut checker = crate::TypeChecker::new();
    checker.check_program(&program).unwrap();
    let env = checker.into_env();
    assert_eq!(env.borrowed_legacy_params.get("observe"), Some(&vec![true]));
    let mut hir = crate::lower_ast(&program, &env);
    for item in &mut hir.items {
        if let crate::hir::HIRItem::Function(function) = item {
            if function.name == "observe" {
                function.body.expr = Some(Box::new(crate::hir::HIRExpr::Var {
                    name: function.params[0].name.clone(),
                    symbol: function.params[0].symbol,
                }));
            }
        }
    }
    let error = crate::lower_hir_with_options(
        &hir.items,
        crate::MirLowerOptions::default().with_checked_parameter_borrows(&env),
    )
    .unwrap_err();
    assert!(
        error.contains("ownership::parameter_contract"),
        "unexpected error: {error}"
    );
}

#[test]
fn legacy_summary_keeps_unknown_recursive_stored_and_returned_values_consuming() {
    let source = r#"
struct String { handle: i64 }
struct Holder { text: String }
impl Drop for String { def drop(&mut self) { print(self.handle); } }
impl String { def free(self) -> bool { true } }
def recursive(value: String) -> bool { if value.handle == 0 { return true; }; recursive(value) }
def unknown(value: String, callback: fn(String) -> bool) -> bool { callback(value) }
def identity(value: String) -> String { value }
def stored(value: String) -> Holder { Holder { text: value } }
def released(value: String) -> bool { value.free(); true }
def raw_return(value: String) -> i64 { value.handle }
def raw_reconstruct(value: String) -> String { String { handle: value.handle } }
def raw_aggregate(value: String) -> Holder { Holder { text: String { handle: value.handle } } }
extern "C" { fn unknown_handle(handle: i64) -> i64; }
def raw_external(value: String) -> i64 { unknown_handle(value.handle) }
def raw_alias(value: String) -> i64 { let alias = value.handle; unknown_handle(alias) }
def raw_reference_alias(value: String) -> i64 { let alias = &value.handle; unknown_handle(*alias) }
def main() -> i64 { 0 }
"#;
    let program = Parser::parse(source).unwrap();
    let mut checker = crate::TypeChecker::new();
    checker.check_program(&program).unwrap();
    for name in [
        "recursive",
        "unknown",
        "identity",
        "stored",
        "released",
        "raw_return",
        "raw_reconstruct",
        "raw_aggregate",
        "raw_external",
        "raw_alias",
        "raw_reference_alias",
    ] {
        assert!(
            !checker.env().borrowed_legacy_params[name][0],
            "{name} cannot infer a borrow"
        );
    }
}

#[test]
fn canonical_string_transfer_still_rejects_use_after_move() {
    let source = r#"
struct String { handle: i64 }
def consume(value: String) -> String { value }
def main() -> i64 { let value = String { handle: 7 }; let moved = consume(value); print(value.handle); 0 }
"#;
    let error = compile_to_ir(source).unwrap_err().to_string();
    assert!(
        error.contains("use-after-move"),
        "real transfer must still move the String: {error}"
    );
}

#[test]
fn discarded_loop_tail_question_value_gets_cleanup() {
    let ir = compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
struct Owner { marker: i64 }
impl Drop for Owner { def drop(&mut self) { print(self.marker); } }
def make() -> Result<Owner, i64> { Result { is_ok: true, value: Owner { marker: 9 }, error: 0 } }
def tail() -> Result<bool, i64> {
    let mut index = 0;
    loop { if index == 5 { break; }; index = index + 1; make()?; };
    Result { is_ok: true, value: true, error: 0 }
}
def main() -> i64 { 0 }
"#,
    )
    .expect("discarded loop-tail owner should compile");
    let body = ir
        .split("; Function: tail\n")
        .nth(1)
        .unwrap()
        .split("\n}\n")
        .next()
        .unwrap();
    assert!(
        body.contains("@Owner_Drop_drop("),
        "loop tail owner must have a real cleanup: {body}"
    );
}

#[test]
fn observed_legacy_arguments_keep_named_and_fresh_owners() {
    let ir = compile_to_ir(
        r#"
struct String { handle: i64 }
impl Drop for String { def drop(&mut self) { print(self.handle); } }
impl String { def eq(self, other: String) -> bool { self.handle == other.handle } }
def make() -> String { String { handle: 7 } }
def observe(value: String) -> bool { value.handle == 7 }
def forward(value: String) -> bool { observe(value) }
def named() -> bool { let a = make(); let b = make(); a.eq(b) }
def fresh() -> bool { make().eq(make()) }
def alias() -> bool { let a = make(); a.eq(a) }
def wrapper() -> bool { let a = make(); forward(a) }
def by_reference(receiver: &String) -> bool { let a = make(); receiver.eq(a) }
def consume(value: String) -> String { value }
def transfer() -> String { let a = make(); consume(a) }
def main() -> i64 { 0 }
"#,
    )
    .expect("legacy observed arguments should compile");
    for (name, expected) in [
        ("named", 2),
        ("fresh", 2),
        ("alias", 1),
        ("wrapper", 1),
        ("by_reference", 1),
        ("transfer", 0),
    ] {
        let marker = format!("; Function: {name}\n");
        let body = ir
            .split(&marker)
            .nth(1)
            .unwrap()
            .split("\n}\n")
            .next()
            .unwrap();
        assert_eq!(
            body.matches("@String_Drop_drop(").count(),
            expected,
            "ownership for {name}: {body}"
        );
    }
}

#[test]
fn temporary_owned_question_receiver_gets_lexical_cleanup() {
    let ir = compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
struct Owner { marker: i64 }
impl Drop for Owner { def drop(&mut self) { print(self.marker); } }
impl Owner { def peek(&self) -> i64 { self.marker } }
def make() -> Result<Owner, i64> { Result { is_ok: true, value: Owner { marker: 42 }, error: 0 } }
def later() -> Result<i64, i64> { Result { is_ok: false, value: 0, error: 9 } }
def chain() -> Result<i64, i64> {
    let marker = make()?.peek();
    let next = later()?;
    Result { is_ok: true, value: marker + next, error: 0 }
}
def main() -> i64 { 0 }
"#,
    )
    .expect("temporary owned receiver chain should compile");
    let body = ir
        .split("; Function: chain\n")
        .nth(1)
        .unwrap()
        .split("\n}\n")
        .next()
        .unwrap();
    assert_eq!(
        body.matches("@Owner_Drop_drop(").count(),
        2,
        "the retained temporary must clean up on success and later propagation: {body}"
    );
    let borrowed = body.find("@Owner_peek(").expect("borrowed call");
    let first_drop = body.find("@Owner_Drop_drop(").expect("owner cleanup");
    assert!(
        borrowed < first_drop,
        "receiver cannot be destroyed before its borrowed use: {body}"
    );
}

#[test]
fn consumed_temporary_receiver_does_not_leave_caller_cleanup() {
    let ir = compile_to_ir(
        r#"
struct Owner { marker: i64 }
impl Drop for Owner { def drop(&mut self) { print(self.marker); } }
impl Owner { def consume(self) -> i64 { self.marker } }
def make() -> Owner { Owner { marker: 42 } }
def chain() -> i64 { make().consume() }
def main() -> i64 { 0 }
"#,
    )
    .expect("owning method transfer should compile");
    let body = ir
        .split("; Function: chain\n")
        .nth(1)
        .unwrap()
        .split("\n}\n")
        .next()
        .unwrap();
    assert!(
        !body.contains("@Owner_Drop_drop("),
        "the consuming callee owns the temporary; caller cannot destroy it again: {body}"
    );
}

#[test]
fn borrowed_close_and_free_names_preserve_caller_ownership() {
    let ir = compile_to_ir(
        r#"
struct Owner { marker: i64 }
impl Drop for Owner { def drop(&mut self) { print(self.marker); } }
impl Owner {
    def close(&mut self) { self.marker = self.marker + 1; }
    def free(&mut self) { self.marker = self.marker + 1; }
}
def named() -> i64 {
    let mut owner = Owner { marker: 40 };
    owner.close(); owner.free(); owner.marker
}
def make() -> Owner { Owner { marker: 42 } }
def temporary() { make().close(); }
def main() -> i64 { 0 }
"#,
    )
    .expect("borrowed method names cannot consume the owner");
    for name in ["named", "temporary"] {
        let marker = format!("; Function: {name}\n");
        let body = ir
            .split(&marker)
            .nth(1)
            .unwrap()
            .split("\n}\n")
            .next()
            .unwrap();
        assert_eq!(
            body.matches("@Owner_Drop_drop(").count(),
            1,
            "a borrowed close/free call leaves its owner alive until lexical cleanup: {body}"
        );
    }
}

#[test]
fn named_result_question_transfers_its_owned_payload_before_cleanup() {
    let ir = compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
struct Owner { marker: i64 }
impl Drop for Owner { def drop(&mut self) { print(self.marker); } }
def forward(input: Result<Owner, i64>) -> Result<Owner, i64> {
    let pending = input;
    let owner = pending?;
    Result { is_ok: true, value: owner, error: 0 }
}


def main() -> i64 { 0 }
"#,
    )
    .expect("named result propagation should compile");
    let forward = ir
        .split("; Function: forward\n")
        .nth(1)
        .unwrap()
        .split("\n}\n")
        .next()
        .unwrap();
    assert!(!forward.contains("@Result_Owner_i64_Drop_drop("),
        "both success extraction and failure propagation transfer the named container; it must not close the transferred owner:\n{forward}");
}

#[test]
fn named_result_question_rejects_reusing_the_consumed_owner() {
    let error = compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
struct Owner { marker: i64 }
impl Drop for Owner { def drop(&mut self) {} }
def consume(input: Result<Owner, i64>) -> Result<i64, i64> {
    let owner = input?;
    Result { is_ok: true, value: input.value.marker + owner.marker, error: 0 }
}
def main() -> i64 { 0 }
"#,
    )
    .expect_err("owning named question-mark operands must be consumed");
    assert!(format!("{error:?}").contains("use-after-move"), "{error:?}");
}

#[test]
fn discarded_owned_producers_are_dropped_at_the_statement_boundary() {
    let ir = compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
struct Owner { marker: i64 }
impl Drop for Owner { def drop(&mut self) { print(self.marker); } }
def make() -> Result<Owner, i64> { Result { is_ok: true, value: Owner { marker: 42 }, error: 0 } }
def later() -> Result<i64, i64> { Result { is_ok: false, value: 0, error: 9 } }
def discarded() -> Result<i64, i64> {
    make()?;
    let value = later()?;
    Result { is_ok: true, value: value, error: 0 }
}
def main() -> i64 { 0 }
"#,
    )
    .expect("discarded owning producer should compile");
    let body = ir
        .split("; Function: discarded\n")
        .nth(1)
        .unwrap()
        .split("\n}\n")
        .next()
        .unwrap();
    assert_eq!(
        body.matches("@Owner_Drop_drop(").count(),
        1,
        "the unused owner is released once before a later failing request: {body}"
    );
    assert!(
        body.find("@Owner_Drop_drop(").unwrap() < body.find("@later(").unwrap(),
        "discarded values must not retain capacity until function exit: {body}"
    );
}

fn typeck_ok(source: &str) {
    let mut parser = Parser::new(source);
    let program = parser.parse_program().expect("parse");
    typeck(&program).expect("typeck");
}

fn typeck_err(source: &str) {
    let mut parser = Parser::new(source);
    let program = parser.parse_program().expect("parse");
    assert!(typeck(&program).is_err(), "expected typeck failure");
}

#[test]
fn result_question_in_result_function_typechecks() {
    typeck_ok(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def ok_i64(v: i64) -> Result<i64, i64> {
    Result { is_ok: true, value: v, error: 0 }
}
def step() -> Result<i64, i64> {
    ok_i64(1)
}
def main() -> Result<i64, i64> {
    let x = step()?;
    ok_i64(x + 1)
}
"#,
    );
}

#[test]
fn option_question_in_option_function_typechecks() {
    typeck_ok(
        r#"
struct Option<T> { is_some: bool, value: T }
def some_i64(v: i64) -> Option<i64> {
    Option { is_some: true, value: v }
}
def main() -> Option<i64> {
    let x = some_i64(3)?;
    some_i64(x + 1)
}
"#,
    );
}

#[test]
fn question_rejects_result_in_option_context() {
    typeck_err(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
struct Option<T> { is_some: bool, value: T }
def bad() -> Result<i64, i64> {
    Result { is_ok: true, value: 0, error: 0 }
}
def main() -> Option<i64> {
    let sink = bad()?;
    sink
}
"#,
    );
}

#[test]
fn question_rejects_plain_i64_main() {
    typeck_err(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def ok() -> Result<i64, i64> {
    Result { is_ok: true, value: 1, error: 0 }
}
def main() -> i64 {
    let sink = ok()?;
    sink
}
"#,
    );
}

#[test]
fn question_rejects_mismatched_result_error_type() {
    typeck_err(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def err_other() -> Result<i64, bool> {
    Result { is_ok: false, value: 0, error: true }
}
def main() -> Result<i64, i64> {
    let sink = err_other()?;
    Result { is_ok: true, value: sink, error: 0 }
}
"#,
    );
}

#[test]
fn try_block_allows_question_in_plain_main() {
    typeck_ok(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def ok_i64(v: i64) -> Result<i64, i64> {
    Result { is_ok: true, value: v, error: 0 }
}
def main() -> i64 {
    let r = try {
        let x = ok_i64(5)?;
        ok_i64(x + 1)
    };
    if r.is_ok { r.value } else { 0 }
}
"#,
    );
}

#[test]
fn nested_result_question_typechecks() {
    typeck_ok(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def ok_i64(v: i64) -> Result<i64, i64> {
    Result { is_ok: true, value: v, error: 0 }
}
def outer() -> Result<i64, i64> {
    let a = ok_i64(1)?;
    let b = ok_i64(a)?;
    ok_i64(b + 1)
}
def main() -> i64 { 0 }
"#,
    );
}

#[test]
fn try_block_scalar_success_wraps_inferred_result_container() {
    compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def fail() -> Result<i64, i64> {
    Result { is_ok: false, value: 0, error: 9 }
}
def main() -> i64 {
    let r = try {
        let x = fail()?;
        x + 1
    };
    if r.is_ok { r.value } else { 0 }
}
"#,
    )
    .expect("try block should wrap scalar success in Result");
}

#[test]
fn try_block_option_propagation_wraps_scalar_success() {
    compile_to_ir(
        r#"
struct Option<T> { is_some: bool, value: T }
def opt_none() -> Option<i64> {
    Option { is_some: false, value: 0 }
}
def main() -> i64 {
    let r = try {
        let x = opt_none()?;
        x + 1
    };
    if r.is_some { r.value } else { 0 }
}
"#,
    )
    .expect("try block should wrap scalar success in Option");
}

#[test]
fn try_block_lowers_without_double_wrapping_result() {
    compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def ok_i64(v: i64) -> Result<i64, i64> {
    Result { is_ok: true, value: v, error: 0 }
}
def main() -> i64 {
    let r = try {
        let x = ok_i64(5)?;
        ok_i64(x + 1)
    };
    if r.is_ok { r.value } else { 0 }
}
"#,
    )
    .expect("try block should lower to valid IR");
}

#[test]
fn result_question_lowers_with_branch_and_return() {
    let ir = compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def ok_i64(v: i64) -> Result<i64, i64> {
    Result { is_ok: true, value: v, error: 0 }
}
def fail() -> Result<i64, i64> {
    Result { is_ok: false, value: 0, error: 7 }
}
def use_it() -> Result<i64, i64> {
    let x = fail()?;
    ok_i64(x)
}
def main() -> i64 { 0 }
"#,
    )
    .expect("compile");
    assert!(
        ir.contains("br i1") && ir.contains("extractvalue"),
        "expected branch/unpack for ?, got:\n{ir}"
    );
}

#[test]
fn result_question_rebuilds_failure_for_a_different_success_type() {
    let ir = compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
struct Token { value: i64 }

def fail_token() -> Result<Token, i64> {
    Result {
        is_ok: false,
        value: Token { value: 0 },
        error: 9,
    }
}

def bridge() -> Result<i64, i64> {
    let token = fail_token()?;
    Result { is_ok: true, value: token.value, error: 0 }
}

def main() -> i64 { 0 }
"#,
    )
    .expect("? should rebuild the error container for the caller's success type");

    let bridge = ir
        .split("; Function: bridge")
        .nth(1)
        .expect("bridge should be emitted");
    assert!(
        bridge.contains("ret %Result_i64_i64"),
        "bridge must return its declared Result<i64, i64> on every path:\n{bridge}"
    );
    assert!(
        !bridge.contains("ret %Result_Token_i64"),
        "the failure path must not return the operand container unchanged:\n{bridge}"
    );
}
