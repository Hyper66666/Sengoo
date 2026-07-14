use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn abi_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../runtime/abi/portable_runtime_abi_v1.json")
}

fn abi_text() -> String {
    let path = abi_path();
    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "expected canonical portable ABI artifact at {}: {error}",
            path.display()
        )
    })
}

fn abi_json() -> Value {
    let path = abi_path();
    serde_json::from_str(&abi_text()).unwrap_or_else(|error| {
        panic!(
            "portable ABI artifact at {} must be valid JSON: {error}",
            path.display()
        )
    })
}

fn object_field<'a>(value: &'a Value, key: &str) -> &'a serde_json::Map<String, Value> {
    value
        .get(key)
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("expected object field `{key}`"))
}

fn array_field<'a>(value: &'a Value, key: &str) -> &'a Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("expected array field `{key}`"))
}

fn u64_field(value: &Value, key: &str) -> u64 {
    value
        .get(key)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("expected integer field `{key}`"))
}

fn str_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("expected string field `{key}`"))
}

fn names_in_array(value: &Value, key: &str) -> BTreeSet<String> {
    array_field(value, key)
        .iter()
        .map(|entry| str_field(entry, "name").to_owned())
        .collect()
}

fn require_named_entries(value: &Value, key: &str, expected_names: &[&str]) {
    let present = names_in_array(value, key);
    for expected in expected_names {
        assert!(
            present.contains(*expected),
            "expected `{key}` to contain `{expected}`, found {present:?}"
        );
    }
}

fn assert_unique_numeric_ids(entries: &[Value], context: &str) {
    let mut seen = BTreeSet::new();
    for entry in entries {
        let id = u64_field(entry, "id");
        assert!(seen.insert(id), "duplicate id {id} in {context}");
    }
}

fn assert_unique_numeric_ordinals(entries: &[Value], ordinal_key: &str, context: &str) {
    let mut seen = BTreeSet::new();
    for entry in entries {
        let ordinal = u64_field(entry, ordinal_key);
        assert!(
            seen.insert(ordinal),
            "duplicate {ordinal_key} {ordinal} in {context}"
        );
    }
}

#[test]
fn canonical_portable_runtime_abi_uses_v1_schema_and_version_surface() {
    let abi = abi_json();

    assert_eq!(
        abi.get("schema").and_then(Value::as_str),
        Some("sengoo.portable_runtime_abi.v1")
    );
    assert_eq!(
        abi.get("portable_runtime_abi_version")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        abi.get("mir_semantic_abi_version")
            .and_then(Value::as_u64),
        Some(1)
    );
}

#[test]
fn canonical_portable_runtime_abi_contains_required_layouts_and_ids() {
    let abi = abi_json();

    require_named_entries(
        &abi,
        "layouts",
        &["owned_string", "generic_vec", "dyn_trait_fat_ref", "async_frame"],
    );
    require_named_entries(
        &abi,
        "ownership_transitions",
        &[
            "move_in",
            "move_out",
            "borrow_shared",
            "borrow_mut",
            "return_owned",
            "drop_owned",
        ],
    );
    require_named_entries(
        &abi,
        "host_calls",
        &[
            "args_read",
            "env_read",
            "stdout_write",
            "stderr_write",
            "time_now_unix_ms",
            "file_open_read",
        ],
    );
    require_named_entries(
        &abi,
        "resource_limit_categories",
        &[
            "args_total_units",
            "env_total_units",
            "stdout_write_units",
            "stderr_write_units",
            "time_query_units",
            "file_io_units",
        ],
    );

    let dyn_dispatch = object_field(&abi, "dyn_dispatch");
    let dyn_drop_slot_ordinal = dyn_dispatch.get("drop_slot_ordinal").and_then(Value::as_u64);
    assert_eq!(dyn_drop_slot_ordinal, Some(0));
    require_named_entries(
        dyn_dispatch.get("method_slots").unwrap_or(&Value::Null),
        "entries",
        &["first_trait_method", "reserved_runtime_hook"],
    );

    let async_lifecycle = object_field(&abi, "async_lifecycle");
    require_named_entries(
        async_lifecycle.get("operations").unwrap_or(&Value::Null),
        "entries",
        &["poll", "wake", "cancel", "drop"],
    );
}

#[test]
fn canonical_portable_runtime_abi_uses_unique_numeric_ids_and_ordinals() {
    let abi = abi_json();

    assert_unique_numeric_ids(array_field(&abi, "layouts"), "layouts");
    assert_unique_numeric_ids(
        array_field(&abi, "ownership_transitions"),
        "ownership_transitions",
    );
    assert_unique_numeric_ids(array_field(&abi, "host_calls"), "host_calls");
    assert_unique_numeric_ids(
        array_field(&abi, "resource_limit_categories"),
        "resource_limit_categories",
    );

    for layout in array_field(&abi, "layouts") {
        let fields = array_field(layout, "fields");
        assert_unique_numeric_ordinals(
            fields,
            "logical_index",
            &format!("layout `{}` fields", str_field(layout, "name")),
        );
    }

    let dyn_dispatch = object_field(&abi, "dyn_dispatch");
    assert_eq!(
        dyn_dispatch.get("drop_slot_ordinal").and_then(Value::as_u64),
        Some(0),
        "dyn drop slot ordinal must remain stable at 0"
    );
    assert_unique_numeric_ordinals(
        array_field(
            dyn_dispatch
                .get("method_slots")
                .unwrap_or_else(|| panic!("expected array field `method_slots`")),
            "entries",
        ),
        "ordinal",
        "dyn_dispatch.method_slots.entries",
    );

    let async_lifecycle = object_field(&abi, "async_lifecycle");
    assert_unique_numeric_ids(
        array_field(
            async_lifecycle
                .get("operations")
                .unwrap_or_else(|| panic!("expected array field `operations`")),
            "entries",
        ),
        "async_lifecycle.operations.entries",
    );
}

#[test]
fn canonical_portable_runtime_abi_rejects_native_and_c_abi_vocabulary() {
    let text = abi_text().to_ascii_lowercase();
    let forbidden_terms = [
        "void*",
        "size_t",
        "uintptr_t",
        "function_pointer",
        "native_address",
        "windows handle",
        "win32 handle",
        "\"handle\"",
        "hmodule",
        "hwnd",
        "hinstance",
        "\"socket\"",
    ];

    for forbidden in forbidden_terms {
        assert!(
            !text.contains(forbidden),
            "portable ABI artifact must not contain forbidden native term `{forbidden}`"
        );
    }
}
