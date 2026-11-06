use crate::compile_to_ir;

#[test]
fn mutable_argument_can_be_reborrowed_by_shared_parameter() {
    let ir = compile_to_ir(r#"
struct Item { active: bool }
def read(value: &Item) -> bool { value.active }
def finish(value: &mut Item) -> bool { read(value) }
def main() -> i64 {
    let mut value = Item { active: true };
    if finish(&mut value) { 0 } else { 1 }
}
"#).expect("a shared call may temporarily reborrow mutable input");
    assert!(ir.contains("call i1 @read(%Item*"), "{ir}");
}

#[test]
fn shared_argument_cannot_be_coerced_to_mutable_parameter() {
    let error = compile_to_ir(r#"
struct Item { active: bool }
def write(value: &mut Item) { value.active = false; (); }
def invalid(value: &Item) { write(value); (); }
def main() -> i64 { 0 }
"#).unwrap_err().to_string();
    assert!(error.contains("&mut Item"), "{error}");
}

#[test]
fn shared_reference_field_assignment_has_stable_diagnostic() {
    let error = compile_to_ir(r#"
struct Item { active: bool }
def invalid(value: &Item) { value.active = false; (); }
def main() -> i64 { 0 }
"#).unwrap_err().to_string();
    assert!(error.contains("immutable-reference-assignment"), "{error}");
}

#[test]
fn nested_reference_coercion_cannot_upgrade_mutability() {
    assert!(compile_to_ir(r#"
struct Item { active: bool }
def consume(value: &&mut Item) { (); }
def invalid(value: &&Item) { consume(value); (); }
def main() -> i64 { 0 }
"#).is_err());
}

#[test]
fn explicit_dereference_assignment_and_nested_field_address_compile() {
    let ir = compile_to_ir(r#"
struct Item { active: bool }
struct Holder { item: Item }
def replace(value: &mut Item) { *value = Item { active: false }; (); }
def nested(value: &mut Holder) { replace(&mut (*value).item); (); }
def main() -> i64 {
    let mut value = Holder { item: Item { active: true } };
    nested(&mut value);
    if value.item.active { 1 } else { 0 }
}
"#).expect("explicit dereferences must share the borrowed assignment/address path");
    assert!(ir.contains("call void @replace(%Item*"), "{ir}");
}

#[test]
fn shared_dereference_cannot_replace_owned_storage() {
    assert!(compile_to_ir(r#"
struct Item { active: bool }
def invalid(value: &Item) { *value = Item { active: false }; (); }
def main() -> i64 { 0 }
"#).is_err());
}
