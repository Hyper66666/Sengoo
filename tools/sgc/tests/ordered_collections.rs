use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempSource {
    root: PathBuf,
    path: PathBuf,
}

impl TempSource {
    fn new(name: &str, source: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "sengoo-ordered-collections-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create ordered collection test directory");
        let path = root.join("main.sg");
        fs::write(&path, source).expect("write ordered collection test source");
        Self { root, path }
    }
}

impl Drop for TempSource {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn run_native(name: &str, source: &str) {
    let source = TempSource::new(name, source);
    let output = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .arg("run")
        .arg(&source.path)
        .arg("--force-rebuild")
        .output()
        .expect("sgc should launch");

    assert_eq!(
        output.status.code(),
        Some(0),
        "ordered collection native program failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn raw_vec_abi_preserves_alignment_and_exact_move_drop_counts() {
    let Some(clang) = which::which("clang").ok() else {
        eprintln!("skipping RawVec ABI probe: clang not found");
        return;
    };
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("sengoo-raw-vec-abi-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&root).expect("create RawVec ABI probe directory");
    let source_path = root.join("probe.c");
    let executable = root.join(if cfg!(windows) { "probe.exe" } else { "probe" });
    fs::write(
        &source_path,
        r#"
#include "runtime_shared.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

long long sengoo_ptr_to_handle(void* ptr) { return (long long)(intptr_t)ptr; }
void* sengoo_handle_to_ptr(long long handle) { return (void*)(intptr_t)handle; }
long long sengoo_opaque_handle_new(void* ptr) { return sengoo_ptr_to_handle(ptr); }
void* sengoo_opaque_handle_get(long long handle) { return sengoo_handle_to_ptr(handle); }
void* sengoo_opaque_handle_take(long long handle) { return sengoo_handle_to_ptr(handle); }
long long sengoo_copy_bytes_to_managed_buffer(long long handle, const char* bytes, size_t len) {
    (void)handle; (void)bytes; (void)len; return SENGOO_STATUS_UNSUPPORTED;
}
long long sengoo_string_clone_status(long long handle) { return handle; }
long long sengoo_string_free_status(long long handle) { (void)handle; return 0; }
long long sengoo_string_from_bytes_copy(long long bytes, long long len) {
    (void)bytes; (void)len; return 0;
}

#include "runtime_collections.c"

typedef struct { long long value; unsigned char padding[56]; } ProbeValue;
static long long moves = 0;
static long long drops = 0;

static void move_probe(void* destination, void* source) {
    memcpy(destination, source, sizeof(ProbeValue));
    ((ProbeValue*)source)->value = -1;
    moves += 1;
}

static void drop_probe(void* value) {
    ((ProbeValue*)value)->value = -2;
    drops += 1;
}

static uint64_t hash_probe(const void* value) {
    return (uint64_t)((const ProbeValue*)value)->value;
}

static long long eq_probe(const void* left, const void* right) {
    return ((const ProbeValue*)left)->value == ((const ProbeValue*)right)->value;
}

int main(void) {
    SengooTypeDescriptor descriptor = {
        SENGOO_COLLECTIONS_ABI_VERSION, 0, sizeof(ProbeValue), 64,
        move_probe, drop_probe, NULL, NULL, NULL, NULL
    };
    long long handle = sengoo_raw_vec_new(&descriptor);
    if (!handle) return 1;
    for (long long index = 0; index < 10; ++index) {
        ProbeValue value = { index, {0} };
        if (sengoo_raw_vec_push(handle, &value) != SENGOO_STATUS_OK || value.value != -1) return 2;
    }
    if (sengoo_raw_vec_len(handle) != 10) return 3;
    for (long long index = 0; index < 10; ++index) {
        ProbeValue* value = (ProbeValue*)sengoo_raw_vec_get(handle, index);
        if (!value || ((uintptr_t)value % 64) != 0 || value->value != index) return 4;
    }
    ProbeValue inserted = { 99, {0} };
    if (sengoo_raw_vec_insert(handle, 3, &inserted) != SENGOO_STATUS_OK) return 5;
    ProbeValue replacement = { 77, {0} };
    if (sengoo_raw_vec_set(handle, 0, &replacement) != SENGOO_STATUS_OK) return 6;
    ProbeValue rejected_set = { 88, {0} };
    if (sengoo_raw_vec_set(handle, 99, &rejected_set) != SENGOO_STATUS_INVALID_ARGUMENT || rejected_set.value != -2) return 12;
    ProbeValue rejected_insert = { 89, {0} };
    if (sengoo_raw_vec_insert(handle, -1, &rejected_insert) != SENGOO_STATUS_INVALID_ARGUMENT || rejected_insert.value != -2) return 13;
    ProbeValue removed = {0, {0}};
    ProbeValue popped = {0, {0}};
    if (sengoo_raw_vec_remove(handle, 3, &removed) != SENGOO_STATUS_OK || removed.value != 99) return 7;
    if (sengoo_raw_vec_pop(handle, &popped) != SENGOO_STATUS_OK || popped.value != 9) return 8;
    if (moves != 40 || drops != 3) return 9;
    if (sengoo_raw_vec_clear(handle) != SENGOO_STATUS_OK || drops != 12) return 10;
    drop_probe(&removed);
    drop_probe(&popped);
    if (drops != 14 || sengoo_raw_vec_free(handle) != SENGOO_STATUS_OK) return 11;

    moves = 0; drops = 0;
    long long map = sengoo_raw_hashmap_new_parts(
        sizeof(ProbeValue), 64, move_probe, drop_probe, hash_probe, eq_probe,
        sizeof(ProbeValue), 64, move_probe, drop_probe
    );
    if (!map) return 14;
    ProbeValue key1 = { 1, {0} }, value1 = { 10, {0} };
    ProbeValue key2 = { 2, {0} }, value2 = { 20, {0} };
    if (sengoo_raw_hashmap_insert(map, &key1, &value1) != SENGOO_STATUS_OK) return 15;
    if (sengoo_raw_hashmap_insert(map, &key2, &value2) != SENGOO_STATUS_OK) return 16;
    ProbeValue replacement_key = { 1, {0} }, map_replacement = { 11, {0} };
    if (sengoo_raw_hashmap_insert(map, &replacement_key, &map_replacement) != SENGOO_STATUS_OK) return 17;
    ProbeValue lookup = { 1, {0} };
    ProbeValue* found = (ProbeValue*)sengoo_raw_hashmap_get(map, &lookup);
    if (!found || found->value != 11 || sengoo_raw_hashmap_len(map) != 2) return 18;
    ProbeValue remove_key = { 2, {0} }, removed_value = {0, {0}};
    if (sengoo_raw_hashmap_remove(map, &remove_key, &removed_value) != SENGOO_STATUS_OK
        || removed_value.value != 20) return 19;
    if (moves != 6 || drops != 3) return 20;
    if (sengoo_raw_hashmap_clear(map) != SENGOO_STATUS_OK || drops != 5) return 21;
    drop_probe(&removed_value);
    if (drops != 6 || sengoo_raw_hashmap_free(map) != SENGOO_STATUS_OK) return 22;
    return 0;
}
"#,
    )
    .expect("write RawVec ABI probe");

    let stdlib = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("stdlib");
    let compile = Command::new(clang)
        .arg("-std=c11")
        .arg("-I")
        .arg(&stdlib)
        .arg(&source_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("clang should compile RawVec ABI probe");
    assert!(
        compile.status.success(),
        "RawVec ABI probe failed to compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );
    let output = Command::new(&executable)
        .output()
        .expect("RawVec ABI probe should run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "RawVec ABI probe exited with {:?}",
        output.status.code()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn generic_collections_runtime_reports_frozen_abi_v1() {
    run_native(
        "abi-version",
        r#"
extern "C" {
    fn sengoo_collections_abi_version() -> i64;
}

def main() -> i64 {
    if sengoo_collections_abi_version() == 1 { 0 } else { 1 }
}
"#,
    );
}

#[test]
fn generic_vec_i64_constructor_executes_through_raw_vec_runtime() {
    run_native(
        "generic-vec-i64",
        r#"
import std::collections;

def main() -> i64 {
    let values: Vec<i64> = vec_new();
    let first = values.push(20);
    let second = values.push(22);
    if first && second && values.len() == 2 && values.get(0).unwrap_or(0) + values.get(1).unwrap_or(0) == 42 {
        0
    } else {
        1
    }
}
"#,
    );
}

#[test]
fn generic_vec_struct_drops_owned_elements_at_scope_exit() {
    run_native(
        "generic-vec-struct-drop",
        r#"
import std::collections;

extern "C" {
    fn sengoo_string_live_handle_count() -> i64;
}

struct Payload {
    text: String,
}

def scoped(before: i64) -> i64 {
    let values: Vec<Payload> = vec_new();
    let first = string_from_str("first").unwrap_or(String { handle: 0 });
    let replacement = string_from_str("replacement").unwrap_or(String { handle: 0 });
    let inserted = string_from_str("inserted").unwrap_or(String { handle: 0 });
    let rejected = string_from_str("rejected").unwrap_or(String { handle: 0 });
    let pushed = values.push(Payload { text: first });
    let replaced = values.set(0, Payload { text: replacement });
    let inserted_ok = values.insert(0, Payload { text: inserted });
    let rejected_ok = values.set(99, Payload { text: rejected });
    let borrowed_ok = {
        let borrowed = values.get(0);
        let iter = values.iter();
        let iter_item = iter.next();
        (*borrowed).text.handle != 0 && iter_item.is_some()
    };
    let removed = values.remove(1).unwrap_or(Payload { text: String { handle: 0 } });
    let popped = values.pop().unwrap_or(Payload { text: String { handle: 0 } });
    let removed_ok = removed.text.handle != 0;
    let popped_ok = popped.text.handle != 0;
    let cleared = values.clear();
    let after_clear = sengoo_string_live_handle_count();
    if pushed == false {
        1
    } else if replaced == false {
        2
    } else if inserted_ok == false {
        6
    } else if rejected_ok {
        7
    } else if borrowed_ok == false {
        8
    } else if removed_ok == false {
        9
    } else if popped_ok == false {
        10
    } else if cleared == false {
        3
    } else if after_clear != before {
        4
    } else if values.len() != 0 {
        5
    } else {
        0
    }
}

def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let result = scoped(before);
    let after = sengoo_string_live_handle_count();
    if result == 0 && before == after { 0 } else { result + 10 }
}
"#,
    );
}

#[test]
fn generic_vec_into_iter_drops_unconsumed_owned_elements() {
    run_native(
        "generic-vec-into-iter-drop",
        r#"
import std::collections;

extern "C" {
    fn sengoo_string_live_handle_count() -> i64;
}

struct Payload { text: String }

def scoped() -> i64 {
    let values: Vec<Payload> = vec_new();
    values.push(Payload { text: string_from_str("first").unwrap_or(String { handle: 0 }) });
    values.push(Payload { text: string_from_str("second").unwrap_or(String { handle: 0 }) });
    values.push(Payload { text: string_from_str("third").unwrap_or(String { handle: 0 }) });
    let iter = values.into_iter();
    let first = iter.next().unwrap_or(Payload { text: String { handle: 0 } });
    if first.text.handle != 0 { 0 } else { 1 }
}

def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let result = scoped();
    let after = sengoo_string_live_handle_count();
    if result == 0 && before == after { 0 } else { result + 10 }
}
"#,
    );
}

#[test]
fn generic_vecdeque_struct_drops_remaining_owned_elements() {
    run_native(
        "generic-vecdeque-drop",
        r#"
import std::collections;

extern "C" { fn sengoo_string_live_handle_count() -> i64; }

struct Payload { text: String }

def scoped() -> i64 {
    let deque: VecDeque<Payload> = vecdeque_new();
    deque.push_back(Payload { text: string_from_str("back").unwrap_or(String { handle: 0 }) });
    deque.push_front(Payload { text: string_from_str("front").unwrap_or(String { handle: 0 }) });
    let borrowed_ok = {
        let front = deque.front();
        let back = deque.back();
        (*front).text.handle != 0 && (*back).text.handle != 0
    };
    let removed = deque.pop_front().unwrap_or(Payload { text: String { handle: 0 } });
    if borrowed_ok && removed.text.handle != 0 && deque.len() == 1 { 0 } else { 1 }
}

def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let result = scoped();
    let after = sengoo_string_live_handle_count();
    if result == 0 && before == after { 0 } else { result + 10 }
}
"#,
    );
}

#[test]
fn generic_hashmap_struct_key_and_owned_value_drop_exactly_once() {
    run_native(
        "generic-hashmap-drop",
        r#"
import std::collections;

extern "C" { fn sengoo_string_live_handle_count() -> i64; }

#[derive(Hash, PartialEq, Eq)]
struct Key { id: i64 }

struct Payload { text: String }

def scoped() -> i64 {
    let map: HashMap<Key, Payload> = hashmap_new();
    let first = map.insert(
        Key { id: 7 },
        Payload { text: string_from_str("first").unwrap_or(String { handle: 0 }) }
    );
    let replaced = map.insert(
        Key { id: 7 },
        Payload { text: string_from_str("replacement").unwrap_or(String { handle: 0 }) }
    );
    let second = map.insert(
        Key { id: 8 },
        Payload { text: string_from_str("second").unwrap_or(String { handle: 0 }) }
    );
    let lookup = Key { id: 7 };
    let borrowed_ok = {
        let value = map.get(&lookup);
        (*value).text.handle != 0
    };
    let removed = map.remove(&lookup).unwrap_or(Payload { text: String { handle: 0 } });
    if first && replaced && second && borrowed_ok && removed.text.handle != 0 && map.len() == 1 {
        0
    } else {
        1
    }
}

def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let result = scoped();
    let after = sengoo_string_live_handle_count();
    if result == 0 && before == after { 0 } else { result + 10 }
}
"#,
    );
}

#[test]
fn generic_hashset_owned_key_uses_hash_eq_and_exact_drop() {
    run_native(
        "generic-hashset-drop",
        r#"
import std::collections;

extern "C" { fn sengoo_string_live_handle_count() -> i64; }

struct Key { id: i64, text: String }

impl Hash for Key {
    def hash_into(&self, h: &mut Hasher) {
        h.write_i64(self.id);
    }
}

impl PartialEq for Key {
    def eq(&self, other: &Key) -> bool {
        self.id == other.id
    }
}

impl Eq for Key {}

def make_key(id: i64, value: &str) -> Key {
    Key { id: id, text: string_from_str(value).unwrap_or(String { handle: 0 }) }
}

def scoped() -> i64 {
    let probe_left = make_key(1, "alpha");
    let probe_right = make_key(1, "different-owned-text");
    let probe_third = make_key(2, "beta");
    let hash_ok = probe_left.hash() == probe_right.hash();
    let distinct_hash = probe_left.hash() != probe_third.hash();
    let eq_ok = probe_left.eq(&probe_right);
    let set: HashSet<Key> = hashset_new();
    let first = set.insert(make_key(1, "alpha"));
    let duplicate = set.insert(make_key(1, "replacement-key"));
    let second = set.insert(make_key(2, "beta"));
    let lookup = make_key(1, "lookup");
    let contains = set.contains(&lookup);
    let removed = set.remove(&lookup);
    if hash_ok == false { 8 }
    else if distinct_hash == false { 10 }
    else if eq_ok == false { 9 }
    else if first == false { 1 }
    else if duplicate == false { 2 }
    else if second == false { 3 }
    else if contains == false { 4 }
    else if removed == false { 5 }
    else if set.len() != 1 { 6 }
    else { 0 }
}

def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let result = scoped();
    let after = sengoo_string_live_handle_count();
    if result == 0 && before == after { 0 } else { result + 10 }
}
"#,
    );
}

#[test]
fn generic_btree_owned_keys_iterate_in_ord_order_and_drop_exactly_once() {
    run_native(
        "generic-btree-order-drop",
        r#"
import std::collections;

extern "C" { fn sengoo_string_live_handle_count() -> i64; }

struct Key { id: i64, text: String }

impl PartialEq for Key {
    def eq(&self, other: &Key) -> bool { self.id == other.id }
}
impl Eq for Key {}
impl PartialOrd for Key {}
impl Ord for Key {
    def compare(&self, other: &Key) -> i64 {
        if self.id < other.id { -1 } else if self.id > other.id { 1 } else { 0 }
    }
}

struct Payload { text: String }

def key(id: i64, text: &str) -> Key {
    Key { id: id, text: string_from_str(text).unwrap_or(String { handle: 0 }) }
}

def payload(text: &str) -> Payload {
    Payload { text: string_from_str(text).unwrap_or(String { handle: 0 }) }
}

def scoped() -> i64 {
    let map: BTreeMap<Key, Payload> = btreemap_new();
    map.insert(key(3, "three"), payload("v3"));
    map.insert(key(1, "one"), payload("v1"));
    map.insert(key(2, "two"), payload("v2"));
    map.insert(key(2, "two-replacement-key"), payload("v2-replacement"));
    let fallback = key(0, "fallback");
    let ordered = {
        let keys = map.iter_keys();
        let first = keys.next().unwrap_or(&fallback);
        let second = keys.next().unwrap_or(&fallback);
        let third = keys.next().unwrap_or(&fallback);
        (*first).id == 1 && (*second).id == 2 && (*third).id == 3
    };
    let lookup = key(2, "lookup");
    let removed = map.remove(&lookup).unwrap_or(payload("missing"));

    let set: BTreeSet<Key> = btreeset_new();
    set.insert(key(9, "nine"));
    set.insert(key(4, "four"));
    let set_ordered = {
        let keys = set.iter();
        let first = keys.next().unwrap_or(&fallback);
        let second = keys.next().unwrap_or(&fallback);
        (*first).id == 4 && (*second).id == 9
    };
    if ordered && removed.text.handle != 0 && map.len() == 2 && set_ordered && set.len() == 2 {
        0
    } else {
        1
    }
}

def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let result = scoped();
    let after = sengoo_string_live_handle_count();
    if result == 0 && before == after { 0 } else { result + 10 }
}
"#,
    );
}

#[test]
fn ordered_i64_maps_replace_remove_clear_and_iterate_in_key_order() {
    run_native(
        "map",
        r#"
import std::collections;

def main() -> i64 {
    let counts = btreemap_new_i64_i64();
    let inserted_high = counts.insert(30, 3);
    let inserted_low = counts.insert(-4, 4);
    let inserted_mid = counts.insert(10, 1);
    let replaced_mid = counts.insert(10, 99);
    let keys = counts.iter_keys().collect();
    let removed_low = counts.remove(-4);

    let flags = btreemap_new_i64_bool();
    let flag_inserted = flags.insert(5, true);
    let flag_replaced = flags.insert(5, false);
    let flag_value = flags.get(5).unwrap_or(true);
    let flags_cleared = flags.clear();

    let ok = inserted_high
        && inserted_low
        && inserted_mid
        && replaced_mid
        && counts.get(10).unwrap_or(0) == 99
        && keys.len() == 3
        && keys.get(0).unwrap_or(0) == -4
        && keys.get(1).unwrap_or(0) == 10
        && keys.get(2).unwrap_or(0) == 30
        && removed_low
        && !counts.contains(-4)
        && counts.len() == 2
        && flag_inserted
        && flag_replaced
        && !flag_value
        && flags_cleared
        && flags.is_empty();
    if ok { 0 } else { 1 }
}
"#,
    );
}

#[test]
fn ordered_i64_set_deduplicates_and_iterates_in_key_order() {
    run_native(
        "set",
        r#"
import std::collections;

def main() -> i64 {
    let ids = btreeset_new_i64();
    let inserted_high = ids.insert(8);
    let inserted_low = ids.insert(-8);
    let inserted_mid = ids.insert(0);
    let replaced_existing = ids.insert(8);
    let keys = ids.iter().collect();
    let removed_mid = ids.remove(0);

    let ok = inserted_high
        && inserted_low
        && inserted_mid
        && replaced_existing
        && ids.len() == 2
        && keys.len() == 3
        && keys.get(0).unwrap_or(0) == -8
        && keys.get(1).unwrap_or(1) == 0
        && keys.get(2).unwrap_or(0) == 8
        && removed_mid
        && !ids.contains(0);
    if ok { 0 } else { 1 }
}
"#,
    );
}
