mod common;

use common::source_sgc_command;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn legacy_enumerate_next_borrows_its_iterator_across_repeated_calls() {
    let root = std::env::temp_dir().join(format!(
        "sengoo-enumerate-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("main.sg");
    fs::write(&source, r#"
import std::collections;
import std::string;
extern "C" { fn sengoo_string_live_handle_count() -> i64; }
def exercise() -> i64 {
    let values = vec_new_i64();
    values.push(10); values.push(20);
    let integers = values.iter().enumerate();
    let first = integers.next().unwrap_or(EnumeratedI64 { index: 9, value: 0 });
    let second = integers.next().unwrap_or(EnumeratedI64 { index: 9, value: 0 });
    if first.index != 0 or first.value != 10 or second.index != 1 or second.value != 20 or not integers.next().is_none() { return 1; };

    let flags = vec_new_bool();
    flags.push(false); flags.push(true);
    let booleans = flags.iter().enumerate();
    let first_flag = booleans.next().unwrap_or(EnumeratedBool { index: 9, value: true });
    let second_flag = booleans.next().unwrap_or(EnumeratedBool { index: 9, value: false });
    if first_flag.index != 0 or first_flag.value or second_flag.index != 1 or not second_flag.value or not booleans.next().is_none() { return 2; };

    let counts = hashmap_new_i64_i64();
    counts.insert(1, 10); counts.insert(2, 20);
    let entries = counts.iter().enumerate();
    let first_entry = entries.next().unwrap_or(EnumeratedI64 { index: 9, value: 0 });
    let second_entry = entries.next().unwrap_or(EnumeratedI64 { index: 9, value: 0 });
    if first_entry.index != 0 or second_entry.index != 1 or first_entry.value + second_entry.value != 30 or not entries.next().is_none() { return 3; };

    let switches = hashmap_new_bool_bool();
    switches.insert(false, false); switches.insert(true, true);
    let switch_entries = switches.iter().enumerate();
    let first_switch = switch_entries.next().unwrap_or(EnumeratedBool { index: 9, value: false });
    let second_switch = switch_entries.next().unwrap_or(EnumeratedBool { index: 9, value: false });
    if first_switch.index != 0 or second_switch.index != 1 or first_switch.value == second_switch.value or not switch_entries.next().is_none() { return 4; };

    let ids = hashset_new_i64();
    ids.insert(10); ids.insert(20);
    let id_entries = ids.iter().enumerate();
    let first_id = id_entries.next().unwrap_or(EnumeratedI64 { index: 9, value: 0 });
    let second_id = id_entries.next().unwrap_or(EnumeratedI64 { index: 9, value: 0 });
    if first_id.index != 0 or second_id.index != 1 or first_id.value + second_id.value != 30 or not id_entries.next().is_none() { return 5; };

    let states = hashset_new_bool();
    states.insert(false); states.insert(true);
    let state_entries = states.iter().enumerate();
    let first_state = state_entries.next().unwrap_or(EnumeratedBool { index: 9, value: false });
    let second_state = state_entries.next().unwrap_or(EnumeratedBool { index: 9, value: false });
    if first_state.index != 0 or second_state.index != 1 or first_state.value == second_state.value or not state_entries.next().is_none() { return 6; };

    let names = string_map_i64_new();
    names.insert("alpha", 1); names.insert("beta", 2);
    let keys = names.iter_keys().enumerate();
    let first_key = keys.next().unwrap_or(EnumeratedString { index: 9, value: string_new() });
    let second_key = keys.next().unwrap_or(EnumeratedString { index: 9, value: string_new() });
    if first_key.index != 0 or second_key.index != 1 or first_key.value.len() + second_key.value.len() != 9 or not keys.next().is_none() { return 7; };
    0
}
def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let status = exercise();
    if status != 0 { return status; };
    if sengoo_string_live_handle_count() != before { return 8; };
    println("enumerate-repeated-next: passed");
    0
}
"#).unwrap();
    let output = source_sgc_command()
        .arg("run")
        .arg(&source)
        .arg("--force-rebuild")
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("enumerate-repeated-next: passed"));
    assert!(root.starts_with(std::env::temp_dir()));
    fs::remove_dir_all(root).unwrap();
}
